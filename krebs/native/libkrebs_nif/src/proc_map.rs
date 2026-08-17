use libkrebs::error::KrebsError;
use libkrebs::mem::{MemAddress, ProcMap};

use crate::errors::Enc;
use crate::process::ProcessRef;

// ── Wire types ────────────────────────────────────────────────────────────────

/// Encodes as `%Krebs.Region{base: addr, range_start: addr, range_end: addr, perms: str, mem_type: str, state: str}`
#[derive(NifStruct)]
#[module = "Krebs.Region"]
pub struct RegionMap {
    pub base: MemAddress,
    pub range_start: MemAddress,
    pub range_end: MemAddress,
    pub perms: String,
    pub mem_type: String,
    pub state: String,
}

/// Encodes as `%Krebs.Module{name: str, range_start: addr, range_end: addr}`
#[derive(NifStruct)]
#[module = "Krebs.Module"]
pub struct ModuleMap {
    pub name: String,
    pub range_start: MemAddress,
    pub range_end: MemAddress,
}

// ── NIFs ──────────────────────────────────────────────────────────────────────

#[nif(schedule = "DirtyIo")]
pub fn regions(proc: ProcessRef) -> Result<Vec<RegionMap>, Enc<KrebsError>> {
    proc.resource
        .inner
        .lock()
        .unwrap()
        .get_regions()
        .map(|rs| {
            rs.into_iter()
                .map(|r| RegionMap {
                    base: r.base,
                    range_start: r.range.start_addr,
                    range_end: r.range.end_addr,
                    perms: format!("{}", r.perms),
                    mem_type: format!("{}", r.mem_type),
                    state: format!("{}", r.state),
                })
                .collect()
        })
        .map_err(|e| Enc(KrebsError::Mem(e)))
}

#[nif(schedule = "DirtyIo")]
pub fn modules(proc: ProcessRef) -> Result<Vec<ModuleMap>, Enc<KrebsError>> {
    proc.resource
        .inner
        .lock()
        .unwrap()
        .get_modules()
        .map(|ms| {
            ms.into_iter()
                .map(|m| ModuleMap {
                    name: m.name,
                    range_start: m.range.start_addr,
                    range_end: m.range.end_addr,
                })
                .collect()
        })
        .map_err(|e| Enc(KrebsError::Mem(e)))
}
