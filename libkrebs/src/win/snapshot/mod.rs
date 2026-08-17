//! Rust wrappers for the [Windows Tool Help Library](https://docs.microsoft.com/en-us/windows/win32/toolhelp/tool-help-library).
//!
//! Not very reliable. I'd advise against using it.

mod toolhelp_snapshot;
mod toolhelp_snapshot_info_types;

use std::sync::Arc;

use super::error::WinResult;

pub use toolhelp_snapshot::ProcessSnapshotType;
use toolhelp_snapshot::{ChildSnapshot, MotherSnapshot};
use toolhelp_snapshot_info_types::*;
pub use toolhelp_snapshot_info_types::{
    HeapEntry, HeapListEntry, ModuleEntry, ProcessEntry, ThreadEntry,
};

use winapi::shared::minwindef::*;

/// Represents a snapshot of the whole system.
///
/// This is required to take a snapshot of an individual process.
pub struct SystemSnapshotInfo {
    snapshot: Arc<MotherSnapshot>,

    processes: ProcList,
    threads: ThreadList,
}

/// Represents a snapshot of an individual process.
///
/// Requires a system snapshot to be taken first. Internally
/// contains an [`Arc`] pointing to the "mother" snapshot.
#[allow(dead_code)]
pub struct ProcessSnapshotInfo {
    snapshot: ChildSnapshot,

    heap_lists: HeapListList,
    modules: ModuleList,
}

fn list_and_map<T: WinList, U: From<T::Entry>>(list: &T) -> WinResult<Vec<U>> {
    Ok(list.get_all()?.into_iter().map(U::from).collect())
}

impl SystemSnapshotInfo {
    /// Creates a new system snapshot.
    ///
    /// # Errors
    ///
    /// Propagates errors from the underlying Windows function.
    pub fn new() -> WinResult<SystemSnapshotInfo> {
        Ok(Self::from_snapshot(MotherSnapshot::new()?))
    }

    fn from_snapshot(snapshot: MotherSnapshot) -> SystemSnapshotInfo {
        SystemSnapshotInfo {
            processes: ProcList::new(&snapshot),
            threads: ThreadList::new(&snapshot),

            snapshot: Arc::new(snapshot),
        }
    }

    /// Creates a new process snapshot for the given process ID.
    ///
    /// # Errors
    ///
    /// Propagates errors from the underlying Windows function.
    pub fn get_proc_snapshot(
        &self,
        pid: DWORD,
        child_type: ProcessSnapshotType,
    ) -> WinResult<ProcessSnapshotInfo> {
        let child = ChildSnapshot::create_child(self.snapshot.clone(), pid, child_type)?;

        Ok(ProcessSnapshotInfo {
            heap_lists: HeapListList::new(&child),
            modules: ModuleList::new(&child),

            snapshot: child,
        })
    }

    /// Returns a vector of all heap chunks in the list.
    ///
    /// # Errors
    ///
    /// Propagates errors from the underlying Windows function.
    pub fn get_heaps(&self, list: &HeapListEntry) -> WinResult<Vec<HeapEntry>> {
        let heap_list = HeapList::from(list);

        list_and_map(&heap_list)
    }

    /// Returns a vector of all running processes.
    ///
    /// # Errors
    ///
    /// Propagates errors from the underlying Windows function.
    pub fn get_processes(&self) -> WinResult<Vec<ProcessEntry>> {
        list_and_map(&self.processes)
    }

    /// Returns a vector of all running threads.
    ///
    /// # Errors
    ///
    /// Propagates errors from the underlying Windows function.
    pub fn get_threads(&self) -> WinResult<Vec<ThreadEntry>> {
        list_and_map(&self.threads)
    }
}

impl ProcessSnapshotInfo {
    /// Returns a vector of all of the process's heap lists.
    ///
    /// # Errors
    ///
    /// Propagates errors from the underlying Windows function.
    pub fn get_heap_lists(&self) -> WinResult<Vec<HeapListEntry>> {
        list_and_map(&self.heap_lists)
    }

    /// Returns a vector of all of the process's loaded modules.
    /// Only returns modules that contain code.
    ///
    /// # Errors
    ///
    /// Propagates errors from the underlying Windows function.
    pub fn get_modules(&self) -> WinResult<Vec<ModuleEntry>> {
        list_and_map(&self.modules)
    }
}
