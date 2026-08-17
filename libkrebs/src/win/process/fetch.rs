use std::fmt;
use std::mem;
use std::ptr::null;

use winapi::shared::minwindef::*;
use winapi::shared::windef::HWND;
use winapi::um::handleapi::{CloseHandle, INVALID_HANDLE_VALUE};
use winapi::um::tlhelp32::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};
use winapi::um::winuser::{
    EnumWindows, FindWindowW, GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId,
    IsWindowVisible,
};

use super::super::error::*;
use super::super::misc::{str_to_wstr, WSTR};

// -----------------------------------------------------------------------
// WindowInfo
// -----------------------------------------------------------------------

/// A visible top-level window.
#[derive(Clone, Debug)]
pub struct WindowInfo {
    pub hwnd: HWND,
    pub title: String,
}

unsafe impl Send for WindowInfo {}
unsafe impl Sync for WindowInfo {}

// -----------------------------------------------------------------------
// ProcessInfo
// -----------------------------------------------------------------------

/// Everything knowable about a running process without opening a handle.
#[derive(Clone, Debug)]
pub struct ProcessInfo {
    pub pid: DWORD,
    /// Executable base name (e.g. `"notepad.exe"`), populated via Toolhelp32.
    pub exe_name: Option<String>,
    /// All visible top-level windows that belong to this process.
    pub windows: Vec<WindowInfo>,
}

unsafe impl Send for ProcessInfo {}
unsafe impl Sync for ProcessInfo {}

impl ProcessInfo {
    /// Collects all available info for `pid`: executable name (Toolhelp32)
    /// and all visible windows (EnumWindows).
    pub fn from_pid(pid: DWORD) -> Self {
        ProcessInfo {
            pid,
            exe_name: exe_name_for_pid(pid),
            windows: windows_for_pid(pid),
        }
    }

    /// Finds the first top-level window whose title matches `title` exactly,
    /// then returns full info for its owning process.
    ///
    /// Returns `Ok(None)` when no such window exists.
    pub fn from_window_title(title: &str) -> WinResult<Option<Self>> {
        let wstr: WSTR = str_to_wstr(title);
        let hwnd = unsafe { FindWindowW(null(), wstr.as_ptr()) };

        if hwnd.is_null() {
            return last_result_or(None);
        }

        let pid = pid_from_hwnd(hwnd)?;
        Ok(Some(Self::from_pid(pid)))
    }
}

impl fmt::Display for ProcessInfo {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let name = self.exe_name.as_deref().unwrap_or("(unknown)");
        write!(f, "[Process \"{}\" (PID {})]", name, self.pid)
    }
}

// -----------------------------------------------------------------------
// Helpers
// -----------------------------------------------------------------------

/// Returns the executable base name for `target_pid`, or `None` on failure.
fn exe_name_for_pid(target_pid: DWORD) -> Option<String> {
    let snap = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    if snap == INVALID_HANDLE_VALUE {
        return None;
    }

    let mut entry: PROCESSENTRY32W = unsafe { mem::zeroed() };
    entry.dwSize = mem::size_of::<PROCESSENTRY32W>() as u32;

    let result = unsafe {
        if Process32FirstW(snap, &mut entry) == 0 {
            None
        } else {
            loop {
                if entry.th32ProcessID == target_pid {
                    let nul = entry
                        .szExeFile
                        .iter()
                        .position(|&c| c == 0)
                        .unwrap_or(entry.szExeFile.len());
                    break Some(String::from_utf16_lossy(&entry.szExeFile[..nul]));
                }
                if Process32NextW(snap, &mut entry) == 0 {
                    break None;
                }
            }
        }
    };

    unsafe { CloseHandle(snap) };
    result
}

/// Returns all visible top-level windows belonging to `target_pid`.
fn windows_for_pid(target_pid: DWORD) -> Vec<WindowInfo> {
    unsafe extern "system" fn callback(hwnd: HWND, lparam: LPARAM) -> BOOL {
        if IsWindowVisible(hwnd) == 0 {
            return 1;
        }

        let mut pid: DWORD = 0;
        GetWindowThreadProcessId(hwnd, &mut pid);

        let (target, out) = &mut *(lparam as *mut (DWORD, Vec<WindowInfo>));
        if pid != *target {
            return 1;
        }

        let len = GetWindowTextLengthW(hwnd);
        if len > 0 {
            let mut buf: Vec<u16> = vec![0u16; len as usize + 1];
            let actual = GetWindowTextW(hwnd, buf.as_mut_ptr(), len + 1);
            if actual > 0 {
                out.push(WindowInfo {
                    hwnd,
                    title: String::from_utf16_lossy(&buf[..actual as usize]),
                });
            }
        }
        1
    }

    let mut state: (DWORD, Vec<WindowInfo>) = (target_pid, Vec::new());
    unsafe { EnumWindows(Some(callback), &mut state as *mut _ as LPARAM) };
    state.1
}

/// Gets the owning PID of a window handle.
pub fn pid_from_hwnd(hwnd: HWND) -> WinResult<DWORD> {
    let mut pid: DWORD = 0;
    unsafe { GetWindowThreadProcessId(hwnd, &mut pid) };
    if pid == 0 { Err(last_error()) } else { Ok(pid) }
}
