use std::collections::HashMap;
use std::fmt;
use std::mem;
use std::ptr::null_mut;

use winapi::ctypes::*;
use winapi::shared::minwindef::*;
use winapi::shared::windef::HWND;
use winapi::um::winnt::*;

use winapi::um::errhandlingapi::GetLastError;
use winapi::um::handleapi::CloseHandle;
use winapi::um::memoryapi::{ReadProcessMemory, VirtualQueryEx, WriteProcessMemory};
use winapi::um::processthreadsapi::OpenProcess;
use winapi::um::psapi::{EnumProcessModules, GetModuleBaseNameW, GetModuleInformation, MODULEINFO};
use winapi::um::sysinfoapi::{GetSystemInfo, SYSTEM_INFO};

use super::super as win;
use {win::error::*, win::perms::*, win::misc::{WSTR, wstr_to_str}};

use super::fetch::ProcessInfo;
use super::module::ModuleInfo;
use super::region::MemBlock;

use crate::error::{MemError, WinMemError};
use crate::mem::{
    AccessLevel, AddressRange, MemAddress, Module, ProcMap, Process, ProcessId, Reader, Region,
    Writer,
};

const NO_HANDLE: HANDLE = null_mut::<c_void>();

/// A process opened with [`OpenProcess`](https://docs.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-openprocess).
#[derive(Clone)]
pub struct OpenedProcess {
    pub proc_info: ProcessInfo,
    access_handle: HANDLE,
    pub access_perms: ProcessPerms,
    pub open: bool,
}

unsafe impl Send for OpenedProcess {}
unsafe impl Sync for OpenedProcess {}

impl OpenedProcess {
    /// Opens the process described by `proc_info` with the given permissions
    /// (defaulting to `PROCESS_ALL_ACCESS`).
    pub fn from_proc_info(
        proc_info: ProcessInfo,
        access_perms: Option<ProcessPerms>,
    ) -> WinResult<Option<OpenedProcess>> {
        let pid = proc_info.pid;
        let access_perms = access_perms.unwrap_or(ProcessPerms::PROCESS_ALL_ACCESS);

        let access_handle: HANDLE = unsafe { OpenProcess(access_perms.bits(), 0, pid) };
        if access_handle != NO_HANDLE {
            Ok(Some(OpenedProcess {
                proc_info,
                access_handle,
                access_perms,
                open: true,
            }))
        } else {
            match last_result() {
                Err(WinMemError::AccessDenied) => Err(WinMemError::ProcessAccessDenied {
                    pid: pid as ProcessId,
                    access_mask: access_perms.bits(),
                }),
                r => r,
            }
        }
    }

    pub fn force_close(&mut self) -> Result<(), WinMemError> {
        if self.open {
            if unsafe { CloseHandle(self.access_handle) } != 0 {
                self.open = false;
                Ok(())
            } else {
                Err(WinMemError::ProcessCloseFailed {
                    pid: self.pid(),
                    code: last_error_code(),
                })
            }
        } else {
            Err(WinMemError::ProcessAlreadyClosed { pid: self.pid() })
        }
    }

    /// Wrapper for [`VirtualQueryEx`](https://docs.microsoft.com/en-us/windows/win32/api/memoryapi/nf-memoryapi-virtualqueryex).
    pub fn virtual_query(&self, base_addr: MemAddress) -> WinResult<MemBlock> {
        assert!(
            self.access_perms.contains(ProcessPerms::QUERY_INFORMATION),
            "Process handle must have QUERY_INFORMATION"
        );
        let mut info: MEMORY_BASIC_INFORMATION = MEMORY_BASIC_INFORMATION::default();
        let n = unsafe {
            VirtualQueryEx(
                self.access_handle,
                base_addr as LPCVOID,
                &mut info,
                mem::size_of::<MEMORY_BASIC_INFORMATION>(),
            )
        };

        if n == 0 || info.RegionSize == 0 {
            Err(WinMemError::VirtualQueryFailed { addr: base_addr, code: last_error_code() })
        } else {
            Ok(MemBlock::from(info))
        }
    }

    /// The highest address an application can use, per [`GetSystemInfo`].
    ///
    /// This is the correct upper bound for walking the address space — it is
    /// architecture-aware (≈2 GB on 32-bit, ≈128 TB on 64-bit) and avoids the
    /// `ERROR_INVALID_PARAMETER` that `VirtualQueryEx` returns past the real
    /// limit. Preferred over the static `WIN_MAX_USERSPACE_ADDR` estimate.
    ///
    /// [`GetSystemInfo`]: https://learn.microsoft.com/windows/win32/api/sysinfoapi/nf-sysinfoapi-getsysteminfo
    pub fn max_application_address(&self) -> MemAddress {
        let mut info: SYSTEM_INFO = unsafe { mem::zeroed() };
        unsafe { GetSystemInfo(&mut info) };
        info.lpMaximumApplicationAddress as MemAddress
    }

    /// Wrapper for [`EnumProcessModules`](https://docs.microsoft.com/en-us/windows/win32/api/psapi/nf-psapi-enumprocessmodules).
    pub fn get_module_handles(&self) -> WinResult<Vec<HINSTANCE>> {
        const MOD_SIZE: usize = mem::size_of::<HINSTANCE>();

        let mut bytes_needed: u32 = 0;
        if unsafe { EnumProcessModules(self.access_handle, null_mut(), 0, &mut bytes_needed) } == 0
            || bytes_needed == 0
        {
            return Err(WinMemError::ModuleEnumerationFailed {
                code: unsafe { GetLastError() },
            });
        }

        let mut handles: Vec<HINSTANCE> = vec![null_mut(); bytes_needed as usize / MOD_SIZE];
        let mut mem_needed: DWORD = 0;

        if unsafe {
            EnumProcessModules(
                self.access_handle,
                handles.as_mut_ptr() as *mut HINSTANCE,
                bytes_needed,
                &mut mem_needed,
            )
        } == 0
        {
            return Err(WinMemError::ModuleEnumerationFailed {
                code: unsafe { GetLastError() },
            });
        }

        Ok(handles.into_iter().filter(|h| !h.is_null()).collect())
    }

    /// Returns the base name of `mod_handle` inside this process.
    pub fn get_module_name(&self, mod_handle: HMODULE) -> WinResult<String> {
        let mut buf: WSTR = vec![0; MAX_PATH];
        let len = unsafe {
            GetModuleBaseNameW(
                self.access_handle,
                mod_handle,
                buf.as_mut_ptr(),
                (MAX_PATH - 1) as u32,
            )
        };

        if len == 0 {
            return Err(last_error());
        }

        let end = buf.iter().position(|&c| c == 0).unwrap_or(MAX_PATH - 1);
        Ok(wstr_to_str(&buf[..end]))
    }
}

impl Reader for OpenedProcess {
    #[allow(unused_unsafe)]
    unsafe fn read_n_bytes(
        &mut self,
        base_addr: MemAddress,
        buffer: &mut [u8],
        size: usize,
    ) -> Result<usize, MemError> {
        let mut n: usize = 0;
        let ok = unsafe {
            ReadProcessMemory(
                self.access_handle,
                base_addr as LPCVOID,
                buffer.as_mut_ptr() as LPVOID,
                size,
                &mut n,
            )
        };
        if ok == 0 {
            Err(WinMemError::ReadFailed {
                addr: base_addr,
                size,
                code: unsafe { GetLastError() },
            }
            .into())
        } else {
            Ok(n)
        }
    }
}

impl Writer for OpenedProcess {
    #[allow(unused_unsafe)]
    unsafe fn write_n_bytes(
        &mut self,
        base_addr: MemAddress,
        bytes: &[u8],
        size: usize,
    ) -> Result<usize, MemError> {
        let mut n: usize = 0;
        let ok = unsafe {
            WriteProcessMemory(
                self.access_handle,
                base_addr as LPVOID,
                bytes.as_ptr() as LPCVOID,
                size,
                &mut n,
            )
        };
        if ok == 0 {
            Err(WinMemError::WriteFailed {
                addr: base_addr,
                size,
                code: unsafe { GetLastError() },
            }
            .into())
        } else {
            Ok(n)
        }
    }
}

impl ProcMap for OpenedProcess {
    fn get_regions(&self) -> Result<Vec<Region>, MemError> {
        let mut regions: Vec<MemBlock> = vec![];
        let max = self.max_application_address();
        let mut addr: MemAddress = 0;

        while addr <= max {
            // VirtualQueryEx fails with ERROR_INVALID_PARAMETER once `addr`
            // climbs past the last valid user-space region; that's the normal
            // termination signal, not an error. The `max` guard from
            // GetSystemInfo is the common case; this is the belt-and-suspenders.
            let block = match self.virtual_query(addr) {
                Ok(block) => block,
                Err(_) => break,
            };
            // `end_addr` is inclusive; saturating_add avoids wrapping at the top.
            addr = block.range.end_addr.saturating_add(1);
            regions.push(block);
        }

        Ok(regions.into_iter().map(MemBlock::into).collect())
    }

    fn get_modules(&self) -> Result<Vec<Module>, MemError> {
        const MOD_INFO_SIZE: usize = mem::size_of::<MODULEINFO>();
        let handles = self.get_module_handles().map_err(MemError::Windows)?;

        let mut infos = vec![MODULEINFO::default(); handles.len()];
        let results: Vec<BOOL> = handles
            .iter()
            .zip(infos.iter_mut())
            .map(|(h, info)| unsafe {
                GetModuleInformation(
                    self.access_handle,
                    *h as HINSTANCE,
                    info,
                    MOD_INFO_SIZE as u32,
                )
            })
            .collect();

        if results.iter().any(|&r| r == 0) {
            return Err(MemError::Windows(WinMemError::ModuleEnumerationFailed {
                code: unsafe { GetLastError() },
            }));
        }

        Ok(handles
            .into_iter()
            .zip(infos)
            .filter_map(|(h, info)| {
                let name = self.get_module_name(h).ok()?;
                Some(
                    ModuleInfo {
                        name,
                        range: AddressRange::new(
                            info.lpBaseOfDll as MemAddress,
                            info.lpBaseOfDll as MemAddress + info.SizeOfImage as MemAddress,
                        ),
                        entry_point: info.EntryPoint as MemAddress,
                        handle: h,
                    }
                    .into(),
                )
            })
            .collect())
    }
}

impl Process for OpenedProcess {
    fn attach(pid: ProcessId, access: AccessLevel) -> Result<Self, MemError> {
        let perms = match access {
            AccessLevel::Read => ProcessPerms::VM_READ | ProcessPerms::QUERY_INFORMATION,
            AccessLevel::ReadWrite => ProcessPerms::PROCESS_ALL_ACCESS,
        };
        OpenedProcess::from_proc_info(ProcessInfo::from_pid(pid as u32), Some(perms))
            .map_err(MemError::Windows)?
            .ok_or_else(|| {
                MemError::Windows(WinMemError::ProcessAccessDenied {
                    pid,
                    access_mask: perms.bits(),
                })
            })
    }

    fn close(&mut self) -> Result<(), MemError> {
        Ok(self.force_close()?)
    }

    /// Enumerates all running processes via a Toolhelp32 snapshot.
    fn list_processes() -> HashMap<ProcessId, String> {
        use winapi::um::handleapi::INVALID_HANDLE_VALUE;
        use winapi::um::tlhelp32::{
            CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
            TH32CS_SNAPPROCESS,
        };

        let mut map = HashMap::new();
        let snap = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
        if snap == INVALID_HANDLE_VALUE {
            return map;
        }

        let mut entry: PROCESSENTRY32W = unsafe { mem::zeroed() };
        entry.dwSize = mem::size_of::<PROCESSENTRY32W>() as u32;

        unsafe {
            if Process32FirstW(snap, &mut entry) != 0 {
                loop {
                    let nul = entry
                        .szExeFile
                        .iter()
                        .position(|&c| c == 0)
                        .unwrap_or(entry.szExeFile.len());
                    map.insert(
                        entry.th32ProcessID as ProcessId,
                        String::from_utf16_lossy(&entry.szExeFile[..nul]),
                    );
                    if Process32NextW(snap, &mut entry) == 0 {
                        break;
                    }
                }
            }
            CloseHandle(snap);
        }

        map
    }

    fn search_processes(exe_name: &str) -> Vec<ProcessId> {
        Self::list_processes()
            .into_iter()
            .filter(|(_, name)| name.contains(exe_name))
            .map(|(pid, _)| pid)
            .collect()
    }

    /// Enumerates all visible top-level windows via [`EnumWindows`](https://docs.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-enumwindows).
    /// Returns a map of PID → list of window titles.
    fn list_windows() -> HashMap<ProcessId, Vec<String>> {
        use winapi::um::winuser::{
            EnumWindows, GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId,
            IsWindowVisible,
        };

        unsafe extern "system" fn callback(hwnd: HWND, lparam: LPARAM) -> BOOL {
            if IsWindowVisible(hwnd) == 0 {
                return 1;
            }
            let len = GetWindowTextLengthW(hwnd);
            if len == 0 {
                return 1;
            }
            let mut buf: Vec<u16> = vec![0u16; len as usize + 1];
            let actual = GetWindowTextW(hwnd, buf.as_mut_ptr(), len + 1);
            if actual == 0 {
                return 1;
            }
            let title = String::from_utf16_lossy(&buf[..actual as usize]);

            let mut pid: DWORD = 0;
            GetWindowThreadProcessId(hwnd, &mut pid);
            if pid == 0 {
                return 1;
            }

            let map = &mut *(lparam as *mut HashMap<ProcessId, Vec<String>>);
            map.entry(pid as ProcessId).or_default().push(title);
            1
        }

        let mut map: HashMap<ProcessId, Vec<String>> = HashMap::new();
        unsafe { EnumWindows(Some(callback), &mut map as *mut _ as LPARAM) };
        map
    }

    fn search_windows(window_title: &str) -> Vec<ProcessId> {
        Self::list_windows()
            .into_iter()
            .filter(|(_, titles)| titles.iter().any(|t| t.contains(window_title)))
            .map(|(pid, _)| pid)
            .collect()
    }

    fn pid(&self) -> ProcessId {
        self.proc_info.pid as ProcessId
    }

    fn access_level(&self) -> AccessLevel {
        if self.access_perms.contains(ProcessPerms::VM_WRITE) {
            AccessLevel::ReadWrite
        } else {
            AccessLevel::Read
        }
    }

    /// The basename of the executable.  Uses the name already collected in
    /// [`ProcessInfo`] when available, falling back to `GetModuleBaseNameW`.
    fn executable_name(&self) -> String {
        self.proc_info
            .exe_name
            .clone()
            .or_else(|| self.get_module_name(null_mut()).ok())
            .unwrap_or_else(|| format!("(PID {})", self.proc_info.pid))
    }

    fn window_names(&self) -> Vec<String> {
        Self::list_windows().remove(&self.pid()).unwrap_or_default()
    }
}

impl Drop for OpenedProcess {
    fn drop(&mut self) {
        self.force_close().ok();
    }
}

impl fmt::Display for OpenedProcess {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "[{} {} (Handle {:#08p}, Access {})]",
            if self.open { "Open" } else { "Closed" },
            self.proc_info,
            self.access_handle,
            self.access_perms,
        )
    }
}
