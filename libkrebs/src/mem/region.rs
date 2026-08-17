use std::cmp::{max, min};
use std::fmt;

use super::{AddressRange, MemAddress};

/// The allocation state of a memory region.
#[repr(u8)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum State {
    /// The region is not in use and has no backing storage.
    FREE = 0b00000000,
    /// The region is actively mapped and accessible.
    COMMITTED = 0b00000001,
    /// The region has been reserved in the virtual address space but not yet backed by storage.
    RESERVED = 0b00000010,
}

impl fmt::Display for State {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

bitflags! {
    /// The type of backing storage for a memory region.
    pub struct MemType : u8 {
        /// No type information (e.g. free regions).
        const NONE      = 0b00000000;
        /// Private memory exclusive to this process (stack, heap, static data).
        const PRIVATE   = 0b00000001;
        /// A memory-mapped file (non-executable).
        const MAPPED    = 0b00000010;
        /// Part of a mapped executable image (`.text`, `.data`, etc.).
        const IMAGE     = 0b00000100;
    }
}

impl fmt::Display for MemType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

bitflags! {
    /// Access permissions for a memory region.
    pub struct Perms : u8 {
        /// No permissions (inaccessible).
        const NONE      = 0b00000000;
        /// Read access.
        const READ      = 0b00000001;
        /// Write access.
        const WRITE     = 0b00000010;
        /// Execute access.
        const EXECUTE   = 0b00000100;
        /// Copy-on-write (mapped file or shared region that diverges on write).
        const COW       = 0b00001000;
        /// Guard page — triggers an exception on first access.
        const GUARD     = 0b00010000;
        /// No access (distinct from `NONE`; used when a guard page is active).
        const NOPE      = 0b00100000;
    }
}

impl fmt::Display for Perms {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

/// A contiguous region of memory in a process's address space.
#[derive(Copy, Clone)]
pub struct Region {
    /// The base address of the allocation this region belongs to.
    pub base: MemAddress,
    /// The address range covered by this region.
    pub range: AddressRange,
    /// The type of backing storage.
    pub mem_type: MemType,
    /// Whether the region is committed, reserved, or free.
    pub state: State,
    /// Access permissions.
    pub perms: Perms,
}

impl Region {
    /// Returns `true` if this region can be merged with `other`.
    ///
    /// Two regions can be merged if they overlap and share the same type, state, and permissions.
    pub fn can_merge(&self, other: &Region) -> bool {
        self.range.overlaps(other.range)
            && self.mem_type == other.mem_type
            && self.state == other.state
            && self.perms == other.perms
    }

    /// Merges blocks that can be merged.
    pub fn merge(blocks: &[Region]) -> Vec<Region> {
        let mut sorted_blocks: Vec<Region> = Vec::from(blocks);
        sorted_blocks.sort();

        let mut out: Vec<Region> = vec![];

        for next_block in sorted_blocks.iter() {
            let merge_with = out
                .iter()
                .position(|other_block| next_block.can_merge(other_block));

            match merge_with {
                Some(ind) => {
                    out[ind] = Region {
                        range: AddressRange::new(
                            min(next_block.range.start_addr, out[ind].range.start_addr),
                            max(next_block.range.end_addr, out[ind].range.end_addr),
                        ),
                        ..out[ind]
                    }
                }
                None => out.push(*next_block),
            }
        }

        out
    }
}

impl Eq for Region {}

impl PartialEq for Region {
    fn eq(&self, other: &Self) -> bool {
        self.range == other.range
            && self.mem_type == other.mem_type
            && self.state == other.state
            && self.perms == other.perms
            && self.base == other.base
    }
}

impl Ord for Region {
    fn cmp(&self, other: &Region) -> std::cmp::Ordering {
        self.range.cmp(&other.range)
    }
}

impl PartialOrd for Region {
    fn partial_cmp(&self, other: &Region) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Display for Region {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.state {
            State::COMMITTED => write!(
                f,
                "[Segment of Memory w/ base {:#08X}: {} // Type {} // Permissions {}]",
                self.base,
                self.range,
                self.mem_type.to_string(),
                self.perms.to_string()
            ),
            State::FREE => write!(f, "[Free Space {}]", self.range),
            State::RESERVED => write!(f, "[Reserved Space {}]", self.range),
        }
    }
}

impl fmt::Debug for Region {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_mem_blocks() {
        let blocks = vec![
            Region {
                range: AddressRange::new(0x000, 0x100),
                base: 0,
                state: State::COMMITTED,
                perms: Perms::READ | Perms::WRITE | Perms::EXECUTE,
                mem_type: MemType::PRIVATE,
            },
            Region {
                range: AddressRange::new(0x100, 0x200),
                base: 0,
                state: State::COMMITTED,
                perms: Perms::READ | Perms::WRITE | Perms::EXECUTE,
                mem_type: MemType::PRIVATE,
            },
            Region {
                range: AddressRange::new(0x200, 0x300),
                base: 200,
                state: State::COMMITTED,
                perms: Perms::READ,
                mem_type: MemType::PRIVATE,
            },
            Region {
                range: AddressRange::new(0x300, 0x350),
                base: 300,
                state: State::COMMITTED,
                perms: Perms::WRITE,
                mem_type: MemType::PRIVATE,
            },
        ];

        let sorted_blocks = Region::merge(&blocks);

        assert_eq!(
            sorted_blocks,
            vec![
                Region {
                    range: AddressRange::new(0x000, 0x200),
                    base: 0,
                    state: State::COMMITTED,
                    perms: Perms::READ | Perms::WRITE | Perms::EXECUTE,
                    mem_type: MemType::PRIVATE
                },
                Region {
                    range: AddressRange::new(0x200, 0x300),
                    base: 200,
                    state: State::COMMITTED,
                    perms: Perms::READ,
                    mem_type: MemType::PRIVATE
                },
                Region {
                    range: AddressRange::new(0x300, 0x350),
                    base: 300,
                    state: State::COMMITTED,
                    perms: Perms::WRITE,
                    mem_type: MemType::PRIVATE
                }
            ]
        )
    }
}
