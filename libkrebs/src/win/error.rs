use winapi::um::errhandlingapi::GetLastError;

use crate::error::WinMemError;

pub type WinResult<T> = Result<T, WinMemError>;

/// Returns the current thread's last Win32 error as a [`WinMemError`].
pub fn last_error_code() -> u32 {
    unsafe { GetLastError() }
}

/// Returns the current thread's last Win32 error as a [`WinMemError`].
pub fn last_error() -> WinMemError {
    WinMemError::from_code(last_error_code())
}

/// Returns `Err(last_error())`.
pub fn last_result<T>() -> WinResult<T> {
    Err(last_error())
}

/// Returns `Ok(value)` if `GetLastError()` is zero, otherwise `Err(last_error())`.
pub fn last_result_or<T>(value: T) -> WinResult<T> {
    let code = unsafe { GetLastError() };
    if code == 0 {
        Ok(value)
    } else {
        Err(WinMemError::from_code(code))
    }
}
