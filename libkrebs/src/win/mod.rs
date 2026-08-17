//! Bindings and abstractions around the Windows API.

mod error;
pub mod misc;
pub mod perms;
pub mod process;
pub mod snapshot;

pub use error::*;
pub use process::{OpenedProcess, ProcessInfo};
