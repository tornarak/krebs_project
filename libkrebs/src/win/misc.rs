//! Various useful functions for working with the Windows API.
#[allow(dead_code)]
/* IMPORTS */
// std imports
use std::iter;

use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;

// types
use winapi::um::winnt::*;

// functions
use winapi::um::sysinfoapi::{GetSystemInfo, SYSTEM_INFO};
use winapi::um::winuser::GetAsyncKeyState;

/* Wide Strings */

// A Rust wrapper for `wchar_t *` (technically `wchar_t[]`).
pub type WSTR = Vec<WCHAR>;

/// Creates a null-terminated wide string from a string slice.
pub fn str_to_wstr(from: &str) -> WSTR {
    return OsStr::new(from) // OsStr
        .encode_wide() // To WSTR
        .chain(iter::once(0)) // Null Terminator
        .collect(); // To Vector
}

/// Converts a [`WSTR`] to a [`String`].
pub fn wstr_to_str(from: &[WCHAR]) -> String {
    String::from_utf16_lossy(from)
}

/// Converts a [`WSTR`] to a [`String`], truncating any null bytes at the end.
///
/// This is useful when working with various Windows methods that use a fixed-length buffer.
pub fn wstr_to_str_truncated(from: &[WCHAR]) -> String {
    String::from_utf16_lossy(match from.iter().position(|x| *x == 0x00) {
        Some(pos) => &from[0..pos],
        None => from,
    })
}

/// Effectively a wrapper for [GetSystemInfo](https://docs.microsoft.com/en-us/windows/win32/api/sysinfoapi/nf-sysinfoapi-getsysteminfo).
///
/// # Errors
///
/// Returns an error if the underlying windows function fails.
pub fn get_system_info() -> Result<SYSTEM_INFO, String> {
    let mut info = SYSTEM_INFO::default();

    unsafe {
        GetSystemInfo(&mut info);
    }

    if info.dwPageSize == 0 {
        Err(String::from("Failed to get system info"))
    } else {
        Ok(info)
    }
}

/// [This function](https://docs.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getasynckeystate) is like an old friend to me.
///
/// Developing my first keylogger, trying to spy on my parents...
/// Such sweet memories.
pub fn get_async_key_state(key: i32) -> bool {
    let val: i16 = unsafe { GetAsyncKeyState(key) };

    (val & 0x4000) != 0
}
