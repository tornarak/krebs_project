// std imports
use std::sync::Arc;
use std::time::SystemTime;

use log::debug;

// winapi imports
use winapi::shared::minwindef::*;
use winapi::um::winnt::*;

use winapi::um::handleapi::CloseHandle;
use winapi::um::tlhelp32::*;

use super::super::error::{last_result, WinResult};

pub struct MotherSnapshot {
    pub handle: HANDLE,

    #[allow(unused)]
    pub timestamp: SystemTime,
}

pub enum ProcessSnapshotType {
    Heaps,
    Modules,
}

#[allow(unused)]
pub struct ChildSnapshot {
    pub mother: Arc<MotherSnapshot>,
    pub handle: HANDLE,
    pub child_type: ProcessSnapshotType,

    pub pid: DWORD,
    pub timestamp: SystemTime,
}

impl MotherSnapshot {
    pub fn new() -> WinResult<MotherSnapshot> {
        debug!("Creating Mother Snapshot");

        let handle = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPALL, 0) };

        if handle as u64 == (0xFFFFFFFFFFFFFFFF) {
            last_result()
        } else {
            Ok(MotherSnapshot {
                handle,

                timestamp: SystemTime::now(),
            })
        }
    }
}

impl Drop for MotherSnapshot {
    fn drop(&mut self) {
        unsafe { CloseHandle(self.handle) };
        debug!(
            "Closed Handle {:16X} for Mother Snapshot",
            self.handle as u32
        );
    }
}

impl ChildSnapshot {
    pub fn create_child(
        mother: Arc<MotherSnapshot>,
        pid: DWORD,
        child_type: ProcessSnapshotType,
    ) -> WinResult<ChildSnapshot> {
        debug!("Creating Snapshot w/ PID {}", pid);

        let handle = unsafe {
            CreateToolhelp32Snapshot(
                match child_type {
                    ProcessSnapshotType::Heaps => TH32CS_SNAPHEAPLIST,
                    ProcessSnapshotType::Modules => TH32CS_SNAPMODULE,
                },
                pid,
            )
        };

        if handle as u64 == (0xFFFFFFFFFFFFFFFF) {
            last_result()
        } else {
            Ok(ChildSnapshot {
                mother,
                handle,
                child_type,

                pid,
                timestamp: SystemTime::now(),
            })
        }
    }
}

impl Drop for ChildSnapshot {
    fn drop(&mut self) {
        unsafe { CloseHandle(self.handle) };
        debug!(
            "Closed Handle {:16X} for Snapshot of process with PID {}",
            self.handle as u32, self.pid
        );
    }
}
