//! A library that provides a utility similar to [Cheat Engine](https://cheatengine.org/aboutce.php).
//!
//! With libkrebs, you can read from a foreign process's memory with [`win::process`], scan a
//! continuous buffer with [`mem_buffer`], and create complex scan signatures (patterns) with
//! [`pattern_scan`]. The library also contains signatures for common C++ data structures ([`vcpp`]).
//!
//! For a comprehensive example of using libkrebs to scan a running process's memory, see the
//! C++ companion fixture at `libkrebs/examples/cpp_fixture/` and the `cpp_scan` example.

#![feature(portable_simd)]
#![feature(associated_type_defaults)]

#[macro_use]
extern crate bitflags;
#[cfg(unix)]
extern crate libc;
extern crate maplit;
#[cfg(windows)]
extern crate winapi;

pub mod cpp_std;
pub mod error;
pub mod mem;
pub mod pattern_scan;
pub mod scanner;
#[cfg(unix)]
pub mod unix;
pub mod util;
#[cfg(windows)]
pub mod win;

mod boros;
pub use boros::Boros;
mod verifiable;
pub use verifiable::Verifiable;

/// The platform's native opened-process handle type.
#[cfg(windows)]
pub type NativeProcess = win::OpenedProcess;
#[cfg(unix)]
pub type NativeProcess = unix::ProcFs;
