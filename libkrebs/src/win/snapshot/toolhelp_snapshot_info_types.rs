// std imports
use fmt::Display;
use std::fmt;
use std::mem;

// winapi imports
use winapi::shared::basetsd::*;
use winapi::shared::minwindef::*;
use winapi::um::winnt::*;

use winapi::um::tlhelp32::*;

use crate::mem::AddressRange;
use crate::mem::MemAddress;

use super::super::error::{last_result, WinResult};
use super::super::misc::wstr_to_str_truncated;
use super::toolhelp_snapshot::{ChildSnapshot, MotherSnapshot};

pub trait WinList: Sized {
    type Entry: Copy + Default;
    type PartOf;
    const ENTRY_SIZE: usize = mem::size_of::<Self::Entry>();

    fn new(parent: &Self::PartOf) -> Self;

    fn write_first(&self, blank_entry: &mut Self::Entry) -> BOOL;
    fn write_next(&self, prev_entry: &mut Self::Entry) -> BOOL;

    fn get_first(&self) -> WinResult<Self::Entry> {
        let mut first_entry = Self::Entry::default();

        let first_write_result = self.write_first(&mut first_entry);

        if first_write_result != 0 {
            Ok(first_entry)
        } else {
            last_result()
        }
    }

    fn get_next(&self, prev: Self::Entry) -> Option<Self::Entry> {
        let mut write_to = prev;

        let write_result = self.write_next(&mut write_to);

        if write_result != 0 {
            Some(write_to)
        } else {
            None
        }
    }

    fn get_all(&self) -> WinResult<Vec<Self::Entry>> {
        let mut entries: Vec<Self::Entry> = vec![];

        let first_entry = self.get_first()?;
        entries.push(first_entry);

        let mut prev_heap = first_entry;

        loop {
            match self.get_next(prev_heap) {
                Some(heap) => {
                    entries.push(heap);
                    prev_heap = heap;
                }
                None => {
                    break;
                }
            }
        }

        Ok(entries)
    }
}

pub struct HeapListList {
    snapshot_handle: HANDLE,
}

impl WinList for HeapListList {
    type Entry = HEAPLIST32;
    type PartOf = ChildSnapshot;

    fn new(parent: &Self::PartOf) -> Self {
        Self {
            snapshot_handle: parent.handle,
        }
    }

    fn write_first(&self, blank_entry: &mut Self::Entry) -> BOOL {
        blank_entry.dwSize = Self::ENTRY_SIZE;

        unsafe { Heap32ListFirst(self.snapshot_handle, blank_entry as *mut Self::Entry) }
    }

    fn write_next(&self, prev: &mut Self::Entry) -> BOOL {
        prev.dwSize = Self::ENTRY_SIZE;

        unsafe { Heap32ListNext(self.snapshot_handle, prev as *mut Self::Entry) }
    }
}

pub struct HeapList {
    pid: DWORD,
    heap_id: ULONG_PTR,
}

impl From<&HeapListEntry> for HeapList {
    fn from(parent: &HeapListEntry) -> Self {
        Self {
            pid: parent.pid,
            heap_id: parent.heap_id,
        }
    }
}

impl WinList for HeapList {
    type Entry = HEAPENTRY32;
    type PartOf = HEAPLIST32;

    fn new(parent: &Self::PartOf) -> Self {
        Self {
            pid: parent.th32ProcessID,
            heap_id: parent.th32HeapID,
        }
    }

    fn write_first(&self, blank_entry: &mut Self::Entry) -> BOOL {
        blank_entry.dwSize = Self::ENTRY_SIZE;

        unsafe { Heap32First(blank_entry as *mut Self::Entry, self.pid, self.heap_id) }
    }

    fn write_next(&self, prev: &mut Self::Entry) -> BOOL {
        prev.dwSize = Self::ENTRY_SIZE;

        unsafe { Heap32Next(prev as *mut Self::Entry) }
    }
}

pub struct ModuleList {
    snapshot_handle: HANDLE,
}

impl WinList for ModuleList {
    type Entry = MODULEENTRY32W;
    type PartOf = ChildSnapshot;

    fn new(parent: &Self::PartOf) -> Self {
        Self {
            snapshot_handle: parent.handle,
        }
    }

    fn write_first(&self, blank_entry: &mut Self::Entry) -> BOOL {
        blank_entry.dwSize = Self::ENTRY_SIZE as DWORD;

        unsafe { Module32FirstW(self.snapshot_handle, blank_entry as *mut Self::Entry) }
    }

    fn write_next(&self, prev: &mut Self::Entry) -> BOOL {
        prev.dwSize = Self::ENTRY_SIZE as DWORD;

        unsafe { Module32NextW(self.snapshot_handle, prev as *mut Self::Entry) }
    }
}

pub struct ThreadList {
    snapshot_handle: HANDLE,
}

impl WinList for ThreadList {
    type Entry = THREADENTRY32;
    type PartOf = MotherSnapshot;

    fn new(parent: &Self::PartOf) -> Self {
        Self {
            snapshot_handle: parent.handle,
        }
    }

    fn write_first(&self, blank_entry: &mut Self::Entry) -> BOOL {
        blank_entry.dwSize = Self::ENTRY_SIZE as DWORD;

        unsafe { Thread32First(self.snapshot_handle, blank_entry as *mut Self::Entry) }
    }

    fn write_next(&self, prev: &mut Self::Entry) -> BOOL {
        prev.dwSize = Self::ENTRY_SIZE as DWORD;

        unsafe { Thread32Next(self.snapshot_handle, prev as *mut Self::Entry) }
    }
}

pub struct ProcList {
    snapshot_handle: HANDLE,
}

impl WinList for ProcList {
    type Entry = PROCESSENTRY32W;
    type PartOf = MotherSnapshot;

    fn new(parent: &Self::PartOf) -> Self {
        Self {
            snapshot_handle: parent.handle,
        }
    }

    fn write_first(&self, blank_entry: &mut Self::Entry) -> BOOL {
        blank_entry.dwSize = Self::ENTRY_SIZE as DWORD;

        unsafe { Process32FirstW(self.snapshot_handle, blank_entry as *mut Self::Entry) }
    }

    fn write_next(&self, prev: &mut Self::Entry) -> BOOL {
        prev.dwSize = Self::ENTRY_SIZE as DWORD;

        unsafe { Process32NextW(self.snapshot_handle, prev as *mut Self::Entry) }
    }
}

#[derive(Copy, Clone)]
#[allow(dead_code)]
pub struct HeapListEntry {
    orig: HEAPLIST32,

    pub pid: DWORD,
    pub heap_id: ULONG_PTR,
    // 1 for default heap, 2 for shared
    pub flags: DWORD,
}

impl HeapListEntry {
    pub fn get_heaps(&self) -> WinResult<Vec<HeapEntry>> {
        Ok((HeapList::from(self))
            .get_all()?
            .into_iter()
            .map(HeapEntry::from)
            .collect())
    }
}

impl From<HEAPLIST32> for HeapListEntry {
    fn from(orig: HEAPLIST32) -> Self {
        Self {
            pid: orig.th32ProcessID,
            heap_id: orig.th32HeapID,
            flags: orig.dwFlags,

            orig,
        }
    }
}

impl Display for HeapListEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[{} chunk list for PID {} and Heap ID {:08X}]",
            match self.flags {
                1 => "Default",
                2 => "Shared",
                _ => "Unknown",
            },
            self.pid,
            self.heap_id
        )
    }
}

#[allow(dead_code)]
pub struct HeapEntry {
    orig: HEAPENTRY32,
    handle: HANDLE,

    pub range: AddressRange,
    pub pid: DWORD,
    pub heap_id: ULONG_PTR,
    // fixed, free, movable
    pub flags: DWORD,
}

impl From<HEAPENTRY32> for HeapEntry {
    fn from(orig: HEAPENTRY32) -> Self {
        Self {
            pid: orig.th32ProcessID,
            heap_id: orig.th32HeapID,

            range: AddressRange::new(
                orig.dwAddress as MemAddress,
                (orig.dwAddress as MemAddress).saturating_add(orig.dwBlockSize as MemAddress) ,
            ),
            flags: orig.dwFlags,

            handle: orig.hHandle,
            orig,
        }
    }
}

impl Display for HeapEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[{} heap chunk w/ Heap ID {:08X} for PID {} (range {})]",
            match self.flags {
                1 => "Fixed",
                2 => "Free",
                3 => "Movable",
                _ => "Unknown",
            },
            self.heap_id,
            self.pid,
            self.range
        )
    }
}

#[allow(dead_code)]
pub struct ProcessEntry {
    orig: PROCESSENTRY32W,

    pub pid: DWORD,
    pub num_threads: DWORD,
    pub parent_id: DWORD,
    pub base_priority: LONG,
    pub exe_name: String,
}

impl From<PROCESSENTRY32W> for ProcessEntry {
    fn from(orig: PROCESSENTRY32W) -> Self {
        Self {
            pid: orig.th32ProcessID,
            num_threads: orig.cntThreads,
            parent_id: orig.th32ParentProcessID,
            base_priority: orig.pcPriClassBase,
            exe_name: wstr_to_str_truncated(&orig.szExeFile[..]),

            orig,
        }
    }
}

impl Display for ProcessEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            r#"[Process {} ("{}") w/ {} threads and priority {}]"#,
            self.pid, self.exe_name, self.num_threads, self.base_priority
        )
    }
}

#[allow(dead_code)]
pub struct ModuleEntry {
    orig: MODULEENTRY32W,
    handle: HMODULE,

    pub pid: DWORD,
    pub load_count: DWORD,
    pub range: AddressRange,
    pub name: String,
    pub exe_path: String,
}

impl From<MODULEENTRY32W> for ModuleEntry {
    fn from(orig: MODULEENTRY32W) -> Self {
        Self {
            pid: orig.th32ProcessID,
            load_count: orig.GlblcntUsage,
            range: AddressRange::new(
                orig.modBaseAddr as MemAddress,
                (orig.modBaseAddr as MemAddress).saturating_add(orig.modBaseSize as usize),
            ),
            name: wstr_to_str_truncated(&orig.szModule[..]),
            exe_path: wstr_to_str_truncated(&orig.szExePath[..]),

            handle: orig.hModule,
            orig,
        }
    }
}

impl Display for ModuleEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            r#"[Module "{}" ({}) in process {}, from {}]"#,
            self.name, self.exe_path, self.pid, self.range
        )
    }
}

#[allow(dead_code)]
pub struct ThreadEntry {
    orig: THREADENTRY32,

    pub thread_id: DWORD,
    pub owner_pid: DWORD,
    pub priority: LONG,
}

impl From<THREADENTRY32> for ThreadEntry {
    fn from(orig: THREADENTRY32) -> Self {
        Self {
            thread_id: orig.th32ThreadID,
            owner_pid: orig.th32OwnerProcessID,
            priority: orig.tpBasePri,

            orig,
        }
    }
}

impl Display for ThreadEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            r#"[Thread {} in process {} w/ priority {}]"#,
            self.thread_id, self.owner_pid, self.priority
        )
    }
}
