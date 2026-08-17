//! # Terminology
//!
//! The terminology here is a bit different from normal usage.
//!
//! ## Heap
//!
//! By "heap", I actually mean "all writable memory after the executable".
//!
//! ## Module
//!
//! By "module", I actually mean "all writable memory inside of the executable".

use std::cell::RefCell;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use rayon::prelude::*;

use crate::error::{MemError, ScannerError};
use crate::mem::{Buffer, MemAddress, Reader, Region, ScanTime};
use crate::pattern_scan::*;
use crate::NativeProcess;

#[cfg(windows)]
use crate::win::ProcessInfo;

use super::memory_layout::MemoryLayout;

pub trait MatchPredicate<U: Sync + Send>: Sync + Send + Fn(ScanMatch) -> Option<U> {}
impl<T: Sync + Send + Fn(ScanMatch) -> Option<U>, U: Sync + Send> MatchPredicate<U> for T {}

/// A wrapper around a [`NativeProcess`] that handles scanning,
/// memory layout, and buffer allocation.
pub struct Scanner {
    o_proc: Arc<Mutex<NativeProcess>>,
    mem_layout: MemoryLayout,
    read_size: usize,
}

impl Scanner {
    /// Creates a new [`Scanner`], attaching to the process with the given PID.
    ///
    /// `read_size` is the size of the buffers that will be used during a scan.
    /// The optimal size I found during testing was between 512 KB and 2 MB.
    ///
    /// # Errors
    ///
    /// Returns an error if the process cannot be opened, or the memory layout
    /// cannot be determined.
    pub fn new(pid: crate::mem::ProcessId, read_size: usize) -> Result<Self, MemError> {
        use crate::mem::{AccessLevel, Process};
        Self::from_proc(NativeProcess::attach(pid, AccessLevel::Read)?, read_size)
    }

    /// Creates a new [`Scanner`] from an already-opened process handle.
    pub fn from_proc(mut o_proc: NativeProcess, read_size: usize) -> Result<Self, MemError> {
        let mem_layout = MemoryLayout::new(&mut o_proc)?;

        Ok(Self {
            o_proc: Arc::new(Mutex::new(o_proc)),
            mem_layout,
            read_size,
        })
    }

    /// Creates a new [`Scanner`] sharing an existing `Arc<Mutex<NativeProcess>>`.
    ///
    /// The `Arc` is cloned, so both `ProcessResource` and `ScannerResource` share
    /// the same underlying handle.
    pub fn from_arc(o_proc: Arc<Mutex<NativeProcess>>, read_size: usize) -> Result<Self, MemError> {
        let mem_layout = {
            let mut p = o_proc.lock().unwrap();
            MemoryLayout::new(&mut *p)?
        };
        Ok(Self {
            o_proc,
            mem_layout,
            read_size,
        })
    }

    #[cfg(windows)]
    fn process(&self) -> std::sync::MutexGuard<'_, NativeProcess> {
        self.o_proc.lock().unwrap()
    }

    /// Returns the internal process's [`ProcessInfo`].
    #[cfg(windows)]
    pub fn process_info(&self) -> ProcessInfo {
        self.process().proc_info.clone()
    }

    /// Returns the access permissions that the process was opened with.
    #[cfg(windows)]
    pub fn perms(&self) -> crate::win::perms::ProcessPerms {
        self.process().access_perms
    }

    /// Forces the process to close.
    ///
    /// This is normally handled automatically by RAII.
    #[cfg(windows)]
    pub fn close(&self) -> Result<(), crate::error::MemError> {
        use crate::mem::Process;

        let mut guard = self.o_proc.lock().unwrap();
        guard.close()
    }

    /// Refreshes the scanner's internal memory layout.
    ///
    /// Any long-term scanner must run this periodically.
    /// A scanner with a stale layout will repeatedly error on old pages that
    /// have been deallocated, and miss new pages that have been recently allocated.
    pub fn refresh_layout(&mut self) -> Result<(), MemError> {
        let new_mem_layout = {
            let mut mutex_guard = self.o_proc.lock().unwrap();
            MemoryLayout::new(&mut *mutex_guard)?
        };

        self.mem_layout = new_mem_layout;

        Ok(())
    }

    fn scan_blocks(
        &self,
        blocks: &[Region],
        siggy: &Arc<dyn ScanPattern>,
    ) -> Vec<ChunkScanResult> {
        thread_local!(static BUF: RefCell<Option<Buffer<NativeProcess>>> = RefCell::new(None));

        let total_scan_duration = Arc::new(Mutex::new(ScanTime::default()));

        let scan_res = blocks
            .par_iter()
            .map(|mem_block| {
                BUF.with(|buf_cell| {
                    let mut mem_buffer = buf_cell.borrow_mut();
                    if mem_buffer.is_none() {
                        *mem_buffer = Some(Buffer::new(self.o_proc.clone(), self.read_size))
                    };

                    let (result, scan_time) = mem_buffer.as_mut().unwrap().full_scan(
                        siggy.as_ref(),
                        mem_block.range,
                        false,
                    );

                    {
                        let total_scan_time: &mut ScanTime =
                            &mut total_scan_duration.lock().unwrap();
                        *total_scan_time += scan_time;
                    }

                    result
                })
            })
            .collect();

        log::info!("scan_heap: {}", total_scan_duration.lock().unwrap());

        scan_res
    }

    fn scan_blocks_until<F, U>(
        &self,
        blocks: &[Region],
        siggy: &Arc<dyn ScanPattern>,
        func: F,
    ) -> Option<U>
    where
        F: MatchPredicate<U>,
        U: Sync + Send,
    {
        thread_local!(static BUF: RefCell<Option<Buffer<NativeProcess>>> = RefCell::new(None));

        let total_scan_duration = Arc::new(Mutex::new(ScanTime::default()));

        let scan_res = blocks
            .par_iter()
            .map(|mem_block| {
                BUF.with(|buf_cell| {
                    let mut mem_buffer = buf_cell.borrow_mut();
                    if mem_buffer.is_none() {
                        *mem_buffer = Some(Buffer::new(self.o_proc.clone(), self.read_size))
                    };

                    let (result, scan_time) = mem_buffer.as_mut().unwrap().full_scan(
                        siggy.as_ref(),
                        mem_block.range,
                        false,
                    );

                    {
                        let total_scan_time: &mut ScanTime =
                            &mut total_scan_duration.lock().unwrap();
                        *total_scan_time += scan_time;
                    }

                    result
                })
            })
            .map(|chunk_scan| chunk_scan.matches)
            .flatten()
            .find_map_any(func);

        log::info!("scan_heap_until: {}", total_scan_duration.lock().unwrap());

        scan_res
    }

    fn get_block_siblings(&self, heap: &Region) -> Vec<Region> {
        self.mem_layout
            .writable_heap
            .iter()
            .filter(|x| **x != *heap && x.base == heap.base)
            .map(|x| x.clone())
            .collect()
    }

    /// Returns a tuple of `(Region, Vec<Region>)`. The first entry is the [`Region`]
    /// that is the immediate "parent" of the given address. The second entry is a vector
    /// of [`Region`]s with the same allocation base and permissions.
    pub fn get_addr_segment(&self, addr: MemAddress) -> Option<(Region, Vec<Region>)> {
        let opt_immediate_parent = self
            .mem_layout
            .writable_heap
            .iter()
            .find(|x| x.range.contains_addr(addr));
        match opt_immediate_parent {
            None => None,
            Some(ref_immediate_parent) => {
                let immediate_parent: Region = ref_immediate_parent.clone();

                Some((
                    immediate_parent,
                    self.get_block_siblings(ref_immediate_parent),
                ))
            }
        }
    }

    /// Returns `true` if the given address is in the heap.
    pub fn is_in_heap(&self, addr: MemAddress) -> bool {
        self.mem_layout
            .writable_heap
            .iter()
            .any(|block| block.range.contains_addr(addr))
    }

    /// Returns `Ok(())` if the address is in the heap, or an error otherwise.
    pub fn heap_ptr(&self, desc: &str, addr: MemAddress) -> Result<(), ScannerError> {
        if self.is_in_heap(addr) {
            Ok(())
        } else {
            Err(ScannerError::NotInHeap {
                addr,
                desc: desc.to_owned(),
            })
        }
    }

    /// Returns `true` if the given address is in the module.
    pub fn is_in_module(&self, addr: MemAddress) -> bool {
        self.mem_layout.module_range.contains_addr(addr)
    }

    /// Returns `Ok(())` if the address is in the executable image, or an error otherwise.
    pub fn module_ptr(&self, desc: &str, addr: MemAddress) -> Result<(), ScannerError> {
        if self.is_in_module(addr) {
            Ok(())
        } else {
            Err(ScannerError::NotInModule {
                addr,
                desc: desc.to_owned(),
            })
        }
    }

    /// Scans the heap for the given pattern.
    pub fn scan_heap(&self, siggy: &Arc<dyn ScanPattern>) -> Vec<ChunkScanResult> {
        self.scan_blocks(&self.mem_layout.writable_heap, siggy)
            .into_iter()
            .filter(ChunkScanResult::has_matches)
            .collect()
    }

    /// Scans the heap for the given pattern, stopping at the first match for
    /// which the given function returns [`Some`].
    pub fn scan_heap_until<F, U>(&self, siggy: &Arc<dyn ScanPattern>, func: F) -> Option<U>
    where
        F: MatchPredicate<U>,
        U: Sync + Send,
    {
        self.scan_blocks_until(&self.mem_layout.writable_heap, siggy, func)
    }

    /// Like [`scan_heap`], but scans the in-memory module data instead.
    pub fn scan_module(&self, siggy: &Arc<dyn ScanPattern>) -> Vec<ChunkScanResult> {
        let start = SystemTime::now();
        let scan_results = self
            .mem_layout
            .writable_mod
            .par_iter()
            .map(|(block, data)| (block, siggy.scan_buffer(block.range.start_addr, &data)))
            .map(|(block, matches)| ChunkScanResult {
                matches,
                pattern: BytePattern::new(siggy.value().to_vec(), siggy.mask().map(|m| m.to_vec())),
                range: block.range,
            })
            .collect();
        let duration = SystemTime::now().duration_since(start).unwrap();
        log::info!("scan_module: {} ns", duration.as_nanos());
        scan_results
    }

    /// Like [`scan_heap_until`], but scans the in-memory module data instead.
    pub fn scan_module_until<F, U>(&self, siggy: &Arc<dyn ScanPattern>, func: F) -> Option<U>
    where
        F: MatchPredicate<U>,
        U: Sync + Send,
    {
        let start = SystemTime::now();
        let scan_results = self
            .mem_layout
            .writable_mod
            .par_iter()
            .map(|(block, data)| siggy.scan_buffer(block.range.start_addr, &data))
            .flatten()
            .find_map_any(func);
        let duration = SystemTime::now().duration_since(start).unwrap();
        log::info!("scan_module_until: {} ns", duration.as_nanos());
        scan_results
    }
}

impl Reader for Scanner {
    unsafe fn read_n_bytes(
        &mut self,
        base_addr: MemAddress,
        buffer: &mut [u8],
        size: usize,
    ) -> Result<usize, MemError> {
        self.o_proc
            .lock()
            .unwrap()
            .read_n_bytes(base_addr, buffer, size)
    }
}
