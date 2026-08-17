use winapi::um::winnt::*;

use super::super::perms::*;
use crate::mem::{AddressRange, MemAddress, Region};

/// The state of a memory block.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum WinMemState {
    /// Committed memory. The good stuff.
    Commit,
    /// Unused memory.
    Free,
    /// Memory that isn't being used, but has been set aside.
    Reserve,
}

/// The type of memory represented by a memory block.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum WinMemType {
    /// Part of a mapped executable file.
    Image,
    /// Part of a mapped non-executable file.
    Mapped,
    /// Exclusive to the given process. Stack, heap, and static memory lurks here.
    Private,
    /// Not in use.
    None,
}

/// Represents a block of another process's memory.
/// Effectively a wrapper for [MEMORY_BASIC_INFORMATION](https://docs.microsoft.com/en-us/windows/win32/api/winnt/ns-winnt-memory_basic_information).
#[derive(Copy, Clone)]
pub struct MemBlock {
    /// The location of the memory block.
    pub range: AddressRange,
    /// The base allocation address of the block.
    /// Heap blocks with the same base address are part
    /// of the same heap.
    pub alloc_base: PVOID,
    /// The protection level of the memory allocation.
    pub alloc_protection: PagePerms,
    /// Whether the memory block is committed, free, or reserved.
    pub mem_state: WinMemState,
    /// The protection level of the memory block.
    pub protection: PagePerms,
    /// Whether the memory block contains a mapped file, private data, or nothing at all.
    pub mem_type: WinMemType,
}

impl From<MEMORY_BASIC_INFORMATION> for MemBlock {
    fn from(mem_info: MEMORY_BASIC_INFORMATION) -> Self {
        let base = mem_info.BaseAddress as MemAddress;
        let (end, overflow) = base.overflowing_add(mem_info.RegionSize as MemAddress);

        let range = AddressRange::new(base, if overflow { MemAddress::MAX } else { end });

        MemBlock {
            range,
            alloc_base: mem_info.AllocationBase,
            alloc_protection: unsafe { PagePerms::from_bits_unchecked(mem_info.AllocationProtect) },
            protection: unsafe { PagePerms::from_bits_unchecked(mem_info.Protect) },
            mem_type: match mem_info.Type {
                MEM_IMAGE => WinMemType::Image,
                MEM_MAPPED => WinMemType::Mapped,
                MEM_PRIVATE => WinMemType::Private,
                _ => WinMemType::None,
            },
            mem_state: match mem_info.State {
                MEM_COMMIT => WinMemState::Commit,
                MEM_FREE => WinMemState::Free,
                MEM_RESERVE => WinMemState::Reserve,
                _ => panic!("Illegal Memory State"),
            },
        }
    }
}

pub fn convert_perms(protection: PagePerms) -> crate::mem::region::Perms {
    use crate::mem::region::Perms;

    if protection.contains(PagePerms::PAGE_GUARD) {
        Perms::GUARD | Perms::NOPE
    } else {
        //      println!("{:?}", protection);

        let mut cleaned_protection = protection;
        cleaned_protection
            .remove(PagePerms::PAGE_GUARD | PagePerms::PAGE_NOCACHE | PagePerms::PAGE_WRITECOMBINE);

        //      println!("{:?}", cleaned_protection);

        match cleaned_protection {
            PagePerms::PAGE_NOACCESS => Perms::NOPE,

            PagePerms::PAGE_READONLY => Perms::READ,
            PagePerms::PAGE_READWRITE => Perms::READ | Perms::WRITE,
            PagePerms::PAGE_WRITECOPY => Perms::READ | Perms::COW,

            PagePerms::PAGE_EXECUTE => Perms::EXECUTE,
            PagePerms::PAGE_EXECUTE_READ => Perms::READ | Perms::EXECUTE,
            PagePerms::PAGE_EXECUTE_READWRITE => Perms::READ | Perms::WRITE | Perms::EXECUTE,
            PagePerms::PAGE_EXECUTE_WRITECOPY => Perms::READ | Perms::EXECUTE | Perms::COW,

            _ => Perms::NONE,
        }
    }
}

impl Into<Region> for MemBlock {
    fn into(self) -> Region {
        use crate::mem::region::{MemType, State};

        let MemBlock {
            alloc_base, range, ..
        } = self;

        let base = alloc_base as MemAddress;

        let mem_type = match self.mem_type {
            WinMemType::None => MemType::NONE,
            WinMemType::Private => MemType::PRIVATE,
            WinMemType::Image => MemType::IMAGE,
            WinMemType::Mapped => MemType::MAPPED,
        };

        let perms = convert_perms(self.protection);

        let state = match self.mem_state {
            WinMemState::Free => State::FREE,
            WinMemState::Commit => State::COMMITTED,
            WinMemState::Reserve => State::RESERVED,
        };

        Region {
            base,
            range,
            mem_type,
            state,
            perms,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::mem::region::Perms;

    use super::super::super::perms::PagePerms;
    use super::*;

    #[test]
    fn win_to_standard_perms() {
        assert_eq!(convert_perms(PagePerms::PAGE_NOACCESS), Perms::NOPE);
        assert_eq!(
            convert_perms(PagePerms::PAGE_EXECUTE_READWRITE | PagePerms::PAGE_GUARD),
            Perms::GUARD | Perms::NOPE
        );
        assert_eq!(
            convert_perms(PagePerms::PAGE_EXECUTE_READWRITE),
            Perms::READ | Perms::WRITE | Perms::EXECUTE
        );
        assert_eq!(
            convert_perms(PagePerms::PAGE_EXECUTE_WRITECOPY | PagePerms::PAGE_WRITECOMBINE),
            Perms::READ | Perms::EXECUTE | Perms::COW
        );
        assert_eq!(
            convert_perms(
                PagePerms::PAGE_EXECUTE
                    | PagePerms::PAGE_NOCACHE
                    | PagePerms::PAGE_NOCACHE
                    | PagePerms::PAGE_WRITECOMBINE
            ),
            Perms::EXECUTE
        );
    }
}
