use std::sync::Mutex;

use rustler::{Atom, LocalPid, ResourceArc};

use libkrebs::error::KrebsError;
use libkrebs::mem::MemAddress;
use libkrebs::scanner::Scanner;

use crate::atoms;
use crate::erl_channel::ErlChannel;
use crate::errors::Enc;
use crate::pattern::*;
use crate::process::ProcessRef;

// ── Resource ──────────────────────────────────────────────────────────────────

pub struct ScannerResource {
    pub scanner: Mutex<Scanner>,
}

// ── NifStruct ─────────────────────────────────────────────────────────────────

#[derive(NifStruct)]
#[module = "Krebs.ScannerRef"]
pub struct ScannerRef {
    pub resource: ResourceArc<ScannerResource>,
}

impl ScannerRef {
    fn scanner(&self) -> std::sync::MutexGuard<'_, Scanner> {
        self.resource.scanner.lock().unwrap()
    }
}

// ── NIFs ──────────────────────────────────────────────────────────────────────

#[nif(schedule = "DirtyIo")]
pub fn scanner_new(proc: ProcessRef, read_size: usize) -> Result<ScannerRef, Enc<KrebsError>> {
    log::info!("scanner_new: read_size={}", read_size);
    let arc = proc.resource.inner.clone();
    let scanner = Scanner::from_arc(arc, read_size)
        .map_err(|e| Enc(KrebsError::Mem(e)))?;
    Ok(ScannerRef {
        resource: ResourceArc::new(ScannerResource {
            scanner: Mutex::new(scanner),
        }),
    })
}

#[nif(schedule = "DirtyIo")]
pub fn refresh_layout(scanner: ScannerRef) -> Result<(), Enc<KrebsError>> {
    log::debug!("refresh_layout");
    scanner
        .scanner()
        .refresh_layout()
        .map_err(|e| Enc(KrebsError::Mem(e)))
}

#[nif]
pub fn is_in_memory(scanner: ScannerRef, addr: MemAddress, mem_type: Atom) -> Result<bool, Atom> {
    let guard = scanner.scanner();
    if mem_type == atoms::heap() {
        Ok(guard.is_in_heap(addr))
    } else if mem_type == atoms::module() {
        Ok(guard.is_in_module(addr))
    } else {
        Err(atoms::invalid_mem_type())
    }
}

#[nif(schedule = "DirtyCpu")]
pub fn scan(
    scanner: ScannerRef,
    pattern: PatternStruct,
    recipient: LocalPid,
    mem_type: Atom,
) -> Result<usize, Enc<KrebsError>> {
    let guard = scanner.scanner();
    let inner_pattern = pattern.pattern();

    let mem_type_str = if mem_type == atoms::module() { "module" } else { "heap" };
    log::info!("scan: starting mem_type={}", mem_type_str);

    let count = {
        let chan: ErlChannel<MatchStruct> = ErlChannel::new(recipient);

        let send_closure = |scan_match: libkrebs::pattern_scan::ScanMatch| -> Option<()> {
            if let Err(e) = chan.send(MatchStruct::from(scan_match)) {
                log::warn!("scan: send error: {}", e);
            }
            None
        };

        if mem_type == atoms::module() {
            guard.scan_module_until(inner_pattern, send_closure);
        } else {
            guard.scan_heap_until(inner_pattern, send_closure);
        }

        chan.force_close().unwrap()
    };

    log::info!("scan: done, {} matches", count);
    Ok(count)
}
