use std::sync::{Arc, Mutex};

use rustler::{Atom, ResourceArc};

use libkrebs::error::{KrebsError, MemError};
use libkrebs::mem::{AccessLevel, Process, ProcessId};
use libkrebs::NativeProcess;

use crate::atoms;
use crate::errors::Enc;

// ── Resource ──────────────────────────────────────────────────────────────────

pub struct ProcessResource {
    pub inner: Arc<Mutex<NativeProcess>>,
}

// ── NifStruct ─────────────────────────────────────────────────────────────────

#[derive(NifStruct)]
#[module = "Krebs.ProcessRef"]
pub struct ProcessRef {
    pub resource: ResourceArc<ProcessResource>,
}

impl ProcessRef {
    fn guard(&self) -> std::sync::MutexGuard<'_, NativeProcess> {
        self.resource.inner.lock().unwrap()
    }
}

// ── access-level helpers ──────────────────────────────────────────────────────

fn atom_to_access(atom: Atom) -> Option<AccessLevel> {
    if atom == atoms::read() {
        Some(AccessLevel::Read)
    } else if atom == atoms::read_write() {
        Some(AccessLevel::ReadWrite)
    } else {
        None
    }
}

fn access_to_atom(level: AccessLevel) -> Atom {
    match level {
        AccessLevel::Read      => atoms::read(),
        AccessLevel::ReadWrite => atoms::read_write(),
    }
}

// ── Static NIFs (no open handle needed) ──────────────────────────────────────

#[nif]
pub fn list_processes() -> Vec<(usize, String)> {
    NativeProcess::list_processes().into_iter().collect()
}

#[nif]
pub fn search_processes(name: String) -> Vec<usize> {
    NativeProcess::search_processes(&name)
}

#[nif]
pub fn list_windows() -> Vec<(usize, Vec<String>)> {
    NativeProcess::list_windows().into_iter().collect()
}

#[nif]
pub fn search_windows(title: String) -> Vec<usize> {
    NativeProcess::search_windows(&title)
}

// ── Attachment ────────────────────────────────────────────────────────────────

#[nif(schedule = "DirtyIo")]
pub fn attach(pid: usize, access: Atom) -> Result<ProcessRef, Enc<KrebsError>> {
    let level = atom_to_access(access).ok_or_else(|| {
        // invalid atom — treat as invalid handle (compiles on all platforms)
        Enc(KrebsError::Mem(MemError::Windows(
            libkrebs::error::WinMemError::InvalidHandle,
        )))
    })?;

    log::info!("attach: pid={} access={:?}", pid, level);

    let proc = NativeProcess::attach(pid as ProcessId, level)
        .map_err(|e| Enc(KrebsError::Mem(e)))?;

    Ok(ProcessRef {
        resource: ResourceArc::new(ProcessResource {
            inner: Arc::new(Mutex::new(proc)),
        }),
    })
}

// ── Accessors ─────────────────────────────────────────────────────────────────

#[nif]
pub fn process_pid(proc: ProcessRef) -> usize {
    proc.guard().pid()
}

#[nif]
pub fn executable_name(proc: ProcessRef) -> String {
    proc.guard().executable_name()
}

#[nif]
pub fn window_names(proc: ProcessRef) -> Vec<String> {
    proc.guard().window_names()
}

#[nif]
pub fn access_level(proc: ProcessRef) -> Atom {
    access_to_atom(proc.guard().access_level())
}

// ── Close ─────────────────────────────────────────────────────────────────────

#[nif]
pub fn close(proc: ProcessRef) -> Result<(), Enc<KrebsError>> {
    log::info!("close: pid={}", proc.resource.inner.lock().unwrap().pid());
    proc.resource
        .inner
        .lock()
        .unwrap()
        .close()
        .map_err(|e| Enc(KrebsError::Mem(e)))
}
