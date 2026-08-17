//! Wrappers around the most important functions for
//! dealing with Windows processes.

use crate::mem::MemAddress;

/// The upper bound of Windows user-space memory.
/// For more info, see [MSDN](https://docs.microsoft.com/en-us/windows-hardware/drivers/gettingstarted/virtual-address-spaces).
/// Differs by architecture: 2 GB on 32-bit, 128 TB on 64-bit.
#[cfg(target_pointer_width = "64")]
pub const WIN_MAX_USERSPACE_ADDR: MemAddress = 0x7FFF_FFFF_FFFF;
#[cfg(target_pointer_width = "32")]
pub const WIN_MAX_USERSPACE_ADDR: MemAddress = 0x7FFF_FFFF;

mod fetch;
mod module;
mod opened_process;
pub mod region;

pub use fetch::*;
pub use module::*;
pub use opened_process::*;
pub use region::*;
