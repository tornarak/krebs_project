/* IMPORTS */

// std imports

use std::fmt;

// winapi imports

// types
use winapi::shared::minwindef::*;

// types
use crate::mem::{AddressRange, MemAddress, Module};

/// Information about a module inside of a process.
pub struct ModuleInfo {
    pub name: String,
    /// The location of the module in the process's memory.
    pub range: AddressRange,
    /// The module's entry point.
    pub entry_point: MemAddress,
    /// A handle to the module.
    pub handle: HMODULE,
}

unsafe impl Send for ModuleInfo {}
unsafe impl Sync for ModuleInfo {}

impl fmt::Debug for ModuleInfo {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            r#"[Module Image from {} with entry point {:08X}]"#,
            self.range, self.entry_point
        )
    }
}

impl fmt::Display for ModuleInfo {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            r#"[Module Image from {} with entry point {:08X}]"#,
            self.range, self.entry_point
        )
    }
}

impl Into<Module> for ModuleInfo {
    fn into(self) -> Module {
        let ModuleInfo { name, range, .. } = self;

        Module { name, range }
    }
}
