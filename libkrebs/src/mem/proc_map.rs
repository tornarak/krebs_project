use crate::error::MemError;

use super::{Module, Region};

/// Trait for objects that can enumerate the memory map of a process.
///
/// Implementors include [`unix::ProcFs`](crate::unix::ProcFs) (via `/proc/{pid}/maps`)
/// and [`win::OpenedProcess`](crate::win::OpenedProcess) (via `VirtualQueryEx`).
pub trait ProcMap {
    /// Returns all memory regions visible in the process's address space.
    fn get_regions(&self) -> Result<Vec<Region>, MemError>;

    /// Returns all loaded modules (shared libraries and the executable image).
    fn get_modules(&self) -> Result<Vec<Module>, MemError>;
}
