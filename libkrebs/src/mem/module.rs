use super::AddressRange;

/// A loaded module (shared library or executable image) in a process's address space.
pub struct Module {
    /// The module's name, typically its filename (e.g. `"linux_64_client"` or `"game.exe"`).
    pub name: String,
    /// The address range the module occupies in the process.
    pub range: AddressRange,
}
