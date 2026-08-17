//! All error types for libkrebs, collected in one place.
//!
//! All types implement [`Clone`]. No `Arc` wrappers are needed because OS
//! errors are stored as raw codes / [`io::ErrorKind`] values, which are `Copy`.

use std::io;

use thiserror::Error;

use crate::mem::{MemAddress, ProcessId};

// ============================================================
// OS-level errors
// ============================================================

/// Errors originating from Windows API calls.
///
/// Variants that can be constructed from a raw `GetLastError()` code use
/// [`WinMemError::from_code`]. Variants that require additional context
/// (addresses, byte counts) are constructed explicitly at the call site.
#[derive(Error, Debug, Clone)]
pub enum WinMemError {
    #[error("failed to read {size} bytes at {addr:#010X}: Win32 error {code:#010X}")]
    ReadFailed { addr: MemAddress, size: usize, code: u32 },

    #[error("failed to write {size} bytes at {addr:#010X}: Win32 error {code:#010X}")]
    WriteFailed { addr: MemAddress, size: usize, code: u32 },

    #[error("process {pid} could not be opened (access mask: {access_mask:#010X})")]
    ProcessAccessDenied { pid: ProcessId, access_mask: u32 },

    #[error("process {pid} could not be closed")]
    ProcessCloseFailed { pid: ProcessId, code: u32 },

    #[error("process {pid} already closed")]
    ProcessAlreadyClosed { pid: ProcessId },

    #[error("access denied")]
    AccessDenied,

    #[error("memory access denied at {addr:#010X}")]
    MemoryAccessDenied { addr: MemAddress },

    #[error("invalid process handle")]
    InvalidHandle,

    #[error("partial memory copy: transferred {copied} of {expected} bytes")]
    PartialCopy { copied: usize, expected: usize },

    #[error("null pointer")]
    NullPointer,

    #[error("VirtualQueryEx failed at {addr:#010X}: Win32 error {code:#010X}")]
    VirtualQueryFailed { addr: MemAddress, code: u32 },

    #[error("module enumeration failed: Win32 error {code:#010X}")]
    ModuleEnumerationFailed { code: u32 },

    #[error("Win32 error {code:#010X}")]
    Misc { code: u32 },
}

impl WinMemError {
    /// Converts a raw `GetLastError()` code into the appropriate variant.
    pub fn from_code(code: u32) -> Self {
        match code {
            0x05 => WinMemError::AccessDenied,
            0x06 => WinMemError::InvalidHandle,
            _ => WinMemError::Misc { code },
        }
    }
}

/// Errors originating from Unix `/proc` filesystem operations.
///
/// I/O errors are stored as [`io::ErrorKind`] (which is `Copy`) rather than
/// the full `io::Error`, since the kind is all that's meaningful for `/proc`
/// reads — the errno is predictable and the OS message adds nothing.
#[derive(Error, Debug, Clone)]
pub enum UnixMemError {
    #[error("failed to read {size} bytes at {addr:#X}: {kind:?}")]
    ProcMemReadFailed {
        addr: usize,
        size: usize,
        kind: io::ErrorKind,
    },

    #[error("failed to write {size} bytes at {addr:#X}: {kind:?}")]
    ProcMemWriteFailed {
        addr: usize,
        size: usize,
        kind: io::ErrorKind,
    },

    #[error("/proc/{pid}/maps could not be opened: {kind:?}")]
    ProcMapsOpenFailed { pid: i32, kind: io::ErrorKind },

    #[error("/proc/maps row has {field_count} fields (expected 5 or 6): {row:?}")]
    ProcMapsRowParseError { field_count: usize, row: String },

    #[error("failed to parse address range from {input:?}")]
    ProcMapsAddrRangeParseFailed { input: String },

    #[error("failed to parse permissions string {input:?}")]
    ProcMapsPermsParseFailed { input: String },

    #[error("failed to open process {pid}: {kind:?}")]
    ProcessOpenFailed { pid: i32, kind: io::ErrorKind },
}

/// Platform-discriminated memory I/O error.
#[derive(Error, Debug, Clone)]
pub enum MemError {
    #[error(transparent)]
    Windows(#[from] WinMemError),

    #[error(transparent)]
    Unix(#[from] UnixMemError),
}

// Platform-specific From<io::Error> conversions for MemError.
// These are the only #[cfg]-gated items in this file — they are behavioral
// (deciding which OS branch to construct), not structural.
#[cfg(windows)]
impl From<io::Error> for MemError {
    fn from(e: io::Error) -> Self {
        MemError::Windows(WinMemError::from_code(e.raw_os_error().unwrap_or(0) as u32))
    }
}

#[cfg(unix)]
impl From<io::Error> for MemError {
    fn from(e: io::Error) -> Self {
        MemError::Unix(UnixMemError::ProcMemReadFailed {
            addr: 0,
            size: 0,
            kind: e.kind(),
        })
    }
}

// ============================================================
// C++ standard library errors
// ============================================================

/// Errors from MSVC (`vcpp`) C++ standard library types.
pub mod vcpp {
    use thiserror::Error;

    use super::MemError;

    /// Validation and I/O errors for MSVC `std::string`.
    #[derive(Error, Debug, Clone)]
    pub enum StringError {
        // --- CppShortString ---
        #[error(transparent)]
        Short(#[from] ShortStringError),
        // --- CppLongString ---
        #[error(transparent)]
        Long(#[from] LongStringError),
    }

    #[derive(Error, Debug, Clone)]
    pub enum ShortStringError {
        #[error("short string length is 0")]
        ZeroLength,
        #[error("short string alloc_size is {got:#X}, expected 0x0F")]
        BadAllocSize { got: usize },
    }

    #[derive(Error, Debug, Clone)]
    pub enum LongStringError {
        #[error("long string length {length} is less than 16")]
        TooShort { length: usize },
        #[error("long string alloc_size {alloc_size} is below minimum of 15")]
        AllocTooSmall { alloc_size: usize },
        #[error("long string alloc_size {alloc_size:#X} is not a valid MSVC allocation step")]
        InvalidAllocSize { alloc_size: usize },
        #[error("long string c_str_addr is null")]
        NullPtr,
    }

    /// Validation and I/O errors for MSVC `std::vector`.
    #[derive(Error, Debug, Clone)]
    pub enum VectorError {
        #[error("first_addr {first_addr:#010X} is not less than last_addr {last_addr:#010X}")]
        InvalidRange { first_addr: u32, last_addr: u32 },
        #[error("byte diff {diff} is not a multiple of element size {element_size}")]
        Misaligned { diff: u32, element_size: u32 },
        #[error("I/O error reading vector: {0}")]
        Io(#[from] MemError),
    }

    /// Top-level error for all MSVC C++ standard library operations.
    #[derive(Error, Debug, Clone)]
    pub enum Error {
        #[error(transparent)]
        String(#[from] StringError),
        #[error(transparent)]
        Vector(#[from] VectorError),
    }
}

/// Errors from GCC (`gcc`) C++ standard library types.
pub mod gcc {
    use thiserror::Error;

    /// Validation and I/O errors for GCC `std::string`.
    #[derive(Error, Debug, Clone)]
    pub enum StringError {
        // --- CppShortString ---
        #[error(transparent)]
        Short(#[from] ShortStringError),
        // --- CppLongString ---
        #[error(transparent)]
        Long(#[from] LongStringError),
    }

    #[derive(Error, Debug, Clone)]
    pub enum ShortStringError {
        #[error("short string length {length} is out of range (1..=15)")]
        InvalidLength { length: usize },
    }

    #[derive(Error, Debug, Clone)]
    pub enum LongStringError {
        #[error("long string c_str is null")]
        NullPtr,
        #[error("long string capacity {capacity} is less than length {length}")]
        CapacityTooSmall { capacity: usize, length: usize },
    }

    /// Top-level error for all GCC C++ standard library operations.
    #[derive(Error, Debug, Clone)]
    pub enum Error {
        #[error(transparent)]
        String(#[from] StringError),
    }
}

#[derive(Error, Debug, Clone)]
pub enum CommonStringError {
    #[error("buffer length {buf_len} does not match declared string length {str_len}")]
    BufferLengthMismatch { buf_len: usize, str_len: usize },
    #[error("string of length {str_len} has no null terminator at expected index")]
    NoNullTerminator { str_len: usize },
    #[error("string contains embedded null bytes before the terminator")]
    EmbeddedNullBytes,
    // --- I/O ---
    #[error("I/O error reading string: {0}")]
    Io(#[from] MemError),
    // --- Encoding ---
    #[error("string buffer is not valid UTF-8: {source}")]
    InvalidUtf8 {
        #[from]
        source: std::string::FromUtf8Error,
    },
}

// ============================================================
// Top-level errors
// ============================================================

/// Top-level error for all C++ standard library operations, across compilers.
#[derive(Error, Debug, Clone)]
pub enum StdError {
    #[error(transparent)]
    Vcpp(#[from] vcpp::Error),
    #[error(transparent)]
    Gcc(#[from] gcc::Error),
    #[error(transparent)]
    CommonString(#[from] CommonStringError),
}

/// Top-level error for all libkrebs operations.
#[derive(Error, Debug, Clone)]
pub enum KrebsError {
    #[error(transparent)]
    Mem(#[from] MemError),
    #[error(transparent)]
    CppStd(#[from] StdError),
}

/// Errors specific to [`crate::scanner::Scanner`] address validation.
#[derive(Error, Debug, Clone)]
pub enum ScannerError {
    #[error("address {addr:#010X} ({desc}) is not in the heap")]
    NotInHeap { addr: MemAddress, desc: String },

    #[error("address {addr:#010X} ({desc}) is not in the executable image")]
    NotInModule { addr: MemAddress, desc: String },
}
