use std::collections::HashMap;

use super::{ProcMap, Reader, Writer};
use crate::error::MemError;

/// Platform-independent process identifier.
///
/// Intentionally distinct from both `u32` (Windows PID) and `i32`
/// (Unix `pid_t`) so that callers don't accidentally pass the wrong
/// integer type through the API.
pub type ProcessId = usize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessLevel {
    Read,
    ReadWrite,
}

pub trait Process: Sized + Reader + Writer + ProcMap {
    fn attach(pid: ProcessId, access: AccessLevel) -> Result<Self, MemError>;
    fn close(&mut self) -> Result<(), MemError>;

    // --- Process enumeration (PID → exe name) ---
    fn list_processes() -> HashMap<ProcessId, String>;
    fn search_processes(exe_name: &str) -> Vec<ProcessId>;

    // --- Window enumeration (PID → window titles, 0..N per process) ---
    fn list_windows() -> HashMap<ProcessId, Vec<String>>;
    fn search_windows(window_title: &str) -> Vec<ProcessId>;

    // --- Accessors on the attached process ---
    fn pid(&self) -> ProcessId;
    fn access_level(&self) -> AccessLevel;
    fn executable_name(&self) -> String;
    /// All window titles belonging to this process.  Empty on platforms
    /// where window enumeration is not yet implemented.
    fn window_names(&self) -> Vec<String>;
}
