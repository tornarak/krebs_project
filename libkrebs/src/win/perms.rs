use std::fmt;
use std::fmt::Display;

bitflags! {
    // Values come straight from winapi rather than hand-typed hex: the literals
    // had drifted (VM_READ was 0x1 — actually PROCESS_TERMINATE — so handles
    // opened "for reading" lacked PROCESS_VM_READ and EnumProcessModules /
    // ReadProcessMemory failed with ERROR_ACCESS_DENIED).
    pub struct ProcessPerms : u32 {
        const PROCESS_ALL_ACCESS = winapi::um::winnt::PROCESS_ALL_ACCESS as u32;

        const CREATE_PROCESS = winapi::um::winnt::PROCESS_CREATE_PROCESS as u32;
        const CREATE_THREAD = winapi::um::winnt::PROCESS_CREATE_THREAD as u32;
        const DUP_HANDLE = winapi::um::winnt::PROCESS_DUP_HANDLE as u32;
        const QUERY_INFORMATION = winapi::um::winnt::PROCESS_QUERY_INFORMATION as u32;
        const QUERY_LIMITED_INFORMATION = winapi::um::winnt::PROCESS_QUERY_LIMITED_INFORMATION as u32;
        const SET_INFORMATION = winapi::um::winnt::PROCESS_SET_INFORMATION as u32;
        const SET_QUOTA = winapi::um::winnt::PROCESS_SET_QUOTA as u32;
        const SYNCHRONIZE = winapi::um::winnt::SYNCHRONIZE as u32;
        const TERMINATE = winapi::um::winnt::PROCESS_TERMINATE as u32;
        const VM_OPERATION = winapi::um::winnt::PROCESS_VM_OPERATION as u32;
        const VM_READ = winapi::um::winnt::PROCESS_VM_READ as u32;
        const VM_WRITE = winapi::um::winnt::PROCESS_VM_WRITE as u32;
    }
}

impl Display for ProcessPerms {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({:?})", self)
    }
}

bitflags! {
    pub struct PagePerms : u32 {
        const PAGE_NOACCESS = 0x01;

        const PAGE_READONLY = 0x02;
        const PAGE_READWRITE = 0x04;
        const PAGE_WRITECOPY = 0x08;

        const PAGE_EXECUTE = 0x10;
        const PAGE_EXECUTE_READ = 0x20;
        const PAGE_EXECUTE_READWRITE = 0x40;
        const PAGE_EXECUTE_WRITECOPY = 0x80;

        const PAGE_TARGETS_INVALID = 0x40000000;
        const PAGE_TARGETS_NO_UPDATE = 0x40000000;

        const PAGE_GUARD = 0x100;
        const PAGE_NOCACHE = 0x200;
        const PAGE_WRITECOMBINE = 0x400;
    }
}

impl Display for PagePerms {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({:?})", self)
    }
}
