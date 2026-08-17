use std::path::Path;

use crate::error::MemError;
use crate::mem::{region, AddressRange, ProcMap, Process, Reader, Region};

/// A snapshot of a process's relevant memory regions, built once on attach and
/// refreshed periodically by [`Scanner`](crate::scanner::Scanner).
///
/// "Module" refers to the writable pages of the executable image (`.data`, `.bss`, etc.),
/// which are small and read eagerly. "Heap" refers to all other writable private regions,
/// which are scanned lazily on demand.
pub struct MemoryLayout {
    /// The full address range of the executable image.
    pub module_range: AddressRange,
    /// Writable committed pages inside the module, with their contents pre-read.
    pub writable_mod: Vec<(Region, Vec<u8>)>,
    /// All other writable committed regions (heap, stack, anonymous mappings).
    pub writable_heap: Vec<Region>,
}

fn writable(region: &&Region) -> bool {
    region.mem_type == region::MemType::PRIVATE
        && region.state == region::State::COMMITTED
        && region.perms.contains(region::Perms::READ | region::Perms::WRITE)
        // Guard pages must be excluded — accessing them raises an exception.
        && !region.perms.contains(region::Perms::GUARD | region::Perms::NOPE)
}

impl MemoryLayout {
    pub fn new<P: ProcMap + Reader + Process>(o_proc: &mut P) -> Result<MemoryLayout, MemError> {
        let regions = Region::merge(&o_proc.get_regions()?);

        let committed_blocks: Vec<Region> = regions
            .into_iter()
            .filter(|region| region.state == region::State::COMMITTED)
            .collect();

        let modules = o_proc.get_modules()?;

        assert!(modules.len() > 0, "No modules returned for process");

        // Identify the executable by matching the name from /proc/{pid}/exe.
        // Fall back to lowest address if matching fails (e.g., unusual PIE layouts).
        let exe_name = o_proc.executable_name();
        let module_range = modules
            .iter()
            .find(|m| {
                Path::new(&m.name)
                    .file_name()
                    .map_or(false, |n| n.to_string_lossy() == exe_name)
            })
            .map(|m| m.range)
            .unwrap_or_else(|| {
                log::warn!(
                    "could not match executable '{}' in modules, falling back to lowest address",
                    exe_name
                );
                modules.iter().min_by_key(|m| m.range).unwrap().range
            });

        // Only the writable parts of the module are relevant. Everything else is VM'd anyway.
        // When given the choice, scan 3 MB of module instead of 200 MB of heap.
        let writable_mod_blocks: Vec<Region> = committed_blocks
            .iter()
            .filter(writable)
            .map(|x| *x)
            .collect();

        let writable_mod: Vec<(Region, Vec<u8>)> = writable_mod_blocks
            .into_iter()
            .filter_map(|block| {
                let mut buf: Vec<u8> = vec![0x00; block.range.size() as usize];
                o_proc
                    .read_range(block.range, &mut buf)
                    .map(|_bytes_read| (block, buf))
                    .ok()
            })
            .collect();

        // Next we want to search all writable memory.
        // Using VirtualProtect is intrusive and removes relevant information
        // (the protection / partitioning of a given block can actually tell us a lot!)
        let writable_heap: Vec<Region> = committed_blocks
            .iter()
            .filter(writable)
            .map(|x| *x)
            .collect();

        log::debug!(
            "memory_layout: {} heap regions, {} module pages, module={:?}",
            writable_heap.len(),
            writable_mod.len(),
            module_range,
        );

        Ok(MemoryLayout {
            module_range,
            writable_heap,
            writable_mod,
        })
    }
}
