//! Trait for objects that read memory from a process,
//! so that libkrebs can be ported to other operating
//! systems.
//!
//! Also contains a catch-all type for 32-bit address
//! ranges.

use std::cmp::{max, min};
use std::fmt;
use std::mem;

use log::debug;

use crate::error::MemError;
use crate::util::{from_bytes, to_bytes};

/* typedefs */

/// Pointer-width process address. `usize` on every platform: on 64-bit
/// Windows this is the full 64-bit user-space range, matching the Unix
/// backend (`ReadProcessMemory` & friends take a pointer-width `LPCVOID`,
/// so the width tracks the architecture libkrebs is compiled for).
pub type MemAddress = usize;

/* Address Data */

/// Represents a block of addresses in another process's memory.
#[derive(Copy, Clone)]
pub struct AddressRange {
    /// The address at which the range starts (inclusive).
    pub start_addr: MemAddress,
    /// The address at which the range ends (inclusive).
    pub end_addr: MemAddress,
    _no_literal: (),
}

impl AddressRange {
    /// Creates an [`AddressRange`] with the given start and end.
    ///
    /// # Panics
    ///
    /// Panics if `start_addr >= end_addr`.
    pub fn new(start_addr: MemAddress, end_addr: MemAddress) -> AddressRange {
        assert!(
            start_addr <= end_addr,
            "start_addr ({:#08X}) is greater than end_addr ({:#08X})",
            start_addr,
            end_addr
        );

        AddressRange {
            start_addr,
            end_addr,
            _no_literal: (),
        }
    }

    /// Returns the size of the address range.
    ///
    /// # Examples
    ///
    /// ```
    /// use libkrebs::mem::AddressRange;
    ///
    /// assert_eq!(
    /// 	AddressRange::new(0x1000, 0x2000).size(),
    /// 	0x1000
    /// );
    /// ```
    pub fn size(&self) -> MemAddress {
        self.end_addr - self.start_addr
    }

    /// Returns true if the range contains the given range.
    ///
    /// # Examples
    ///
    /// ```
    /// use libkrebs::mem::AddressRange;
    ///
    /// assert!(
    /// 	AddressRange::new(0x0000, 0x1000).contains_range(
    /// 		AddressRange::new(0x0300, 0x0400)
    /// 	));
    /// ```
    pub fn contains_range(&self, other: AddressRange) -> bool {
        self.start_addr <= other.start_addr && self.end_addr >= other.end_addr
    }

    /// Returns true if the range contains the given address.
    ///
    /// # Examples
    ///
    /// ```
    /// use libkrebs::mem::AddressRange;
    ///
    /// assert!(
    /// 	AddressRange::new(0x0000, 0x1000).contains_addr(0x0500)
    /// );
    /// ```
    pub fn contains_addr(&self, addr: MemAddress) -> bool {
        self.start_addr <= addr && self.end_addr >= addr
    }

    /// Returns whether the two vectors overlap (inclusive).
    pub fn overlaps(&self, other: AddressRange) -> bool {
        self.contains_range(other)
            || (self.start_addr <= other.end_addr && self.end_addr >= other.end_addr)
            || (self.start_addr <= other.start_addr && self.end_addr >= other.start_addr)
    }

    /// Returns a vector of subranges that cover at most `size`
    /// bytes, ordered sequentially.
    ///
    /// # Examples
    ///
    /// ```
    /// use libkrebs::mem::AddressRange;
    ///
    /// assert_eq!(
    /// 	AddressRange::new(000, 350).chunk_by(100),
    /// 	vec![
    /// 		AddressRange::new(000, 100),
    /// 		AddressRange::new(100, 200),
    /// 		AddressRange::new(200, 300),
    /// 		AddressRange::new(300, 350),
    /// 	]
    /// );
    /// ```
    pub fn chunk_by(&self, size: MemAddress) -> Vec<AddressRange> {
        assert_ne!(size, 0, "Can't divide by zero, my guy");
        (self.start_addr..=self.end_addr)
            .step_by(size as usize)
            .map(|addr| AddressRange::new(addr, std::cmp::min(addr + size, self.end_addr)))
            .collect()
    }

    /// Returns a vector where all of the overlapping ranges in the
    /// provided vector have been merged.
    pub fn merge(ranges: &Vec<AddressRange>) -> Vec<AddressRange> {
        let mut sorted_ranges = ranges.clone();
        sorted_ranges.sort();

        let mut out: Vec<AddressRange> = vec![];

        for next_range in sorted_ranges.iter() {
            let merge_with = out
                .iter()
                .position(|other_range| next_range.overlaps(*other_range));

            match merge_with {
                Some(ind) => {
                    out[ind] = AddressRange::new(
                        min(next_range.start_addr, out[ind].start_addr),
                        max(next_range.end_addr, out[ind].end_addr),
                    )
                }
                None => out.push(*next_range),
            }
        }

        out
    }
}

impl fmt::Display for AddressRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{:08X} - {:08X}]", self.start_addr, self.end_addr)
    }
}

impl fmt::Debug for AddressRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{:08X} - {:08X}]", self.start_addr, self.end_addr)
    }
}

impl Ord for AddressRange {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.start_addr.cmp(&other.start_addr)
    }
}

impl PartialOrd for AddressRange {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Eq for AddressRange {}

impl PartialEq for AddressRange {
    fn eq(&self, other: &Self) -> bool {
        self.start_addr == other.start_addr && self.end_addr == other.end_addr
    }
}

/// An object that handles reading memory from another process.
pub trait Reader {
    /// Reads `size` bytes to `buffer`, starting at the given address.
    ///
    /// Returns the number of bytes read.
    ///
    /// # Safety
    ///
    /// Trivially unsafe, as it does not check the bounds of the given buffer.
    unsafe fn read_n_bytes(
        &mut self,
        base_addr: MemAddress,
        buffer: &mut [u8],
        size: usize,
    ) -> Result<usize, MemError>;

    /// Fills the buffer with bytes, starting at the given address.
    ///
    /// # Errors
    ///
    /// Propagates errors from [`read_n_bytes`](Reader::read_n_bytes).
    fn read_to_buffer(&mut self, base_addr: MemAddress, buffer: &mut [u8]) -> Result<usize, MemError> {
        unsafe { self.read_n_bytes(base_addr, buffer, buffer.len()) }
    }

    /// Reads a value of the given type from the given address.
    ///
    /// # Errors
    ///
    /// Propagates errors from [`read_n_bytes`](Reader::read_n_bytes).
    fn read_to_type<T: Copy>(&mut self, base_addr: MemAddress) -> Result<T, MemError> {
        let size: usize = mem::size_of::<T>();
        let mut buf: Vec<u8> = vec![0; size];

        self.read_to_buffer(base_addr, &mut buf[..])?;

        Ok(from_bytes::<T>(&buf))
    }

    /// Reads the block of memory represented by the given AddressRange to the given buffer.
    ///
    /// # Panics
    ///
    /// Panics if the range and buffer are different sizes.
    ///
    /// # Errors
    ///
    /// Propagates errors from [`read_n_bytes`](Reader::read_n_bytes).
    fn read_range(&mut self, range: AddressRange, buffer: &mut [u8]) -> Result<usize, MemError> {
        assert!(
            buffer.len() == range.size() as usize,
            "Buffer and range are different sizes"
        );

        self.read_to_buffer(range.start_addr, buffer)
    }

    /// Traverses a dereference chain and returns the end address.
    ///
    /// A dereference chain consists of a base address (`start_addr`)
    /// and a list of offsets (`offsets_v`). They are very useful
    /// when traversing class hierarchies.
    ///
    /// ## How Dereference Chains Work
    ///
    /// Start at `base_addr`. Iterate through the offsets, adding
    /// each offset to the address and then dereferencing the result
    /// to get the new address. When only one offset is left, just add it.
    ///
    /// If you want to dereference the pointer without adding an offset,
    /// simply add a zero.
    ///
    /// # Errors
    ///
    /// Propagates errors from [`read_to_type`](Reader::read_to_type).
    ///
    /// # Panics
    ///
    /// Panics if the offset vector has a length of 0, or if a null pointer is found in the chain.
    ///
    /// # Safety
    ///
    /// This function isn't much safer than a naive C++ implementation.
    /// It checks for null pointers, but if you aren't precise with
    /// your initial address and offsets it's easy to wander into
    /// crazy town and get a segfault.
    ///
    /// # Example
    ///
    /// The game AssaultCube (see [assault_cube](https://github.com/tornarak/krebs_project/tree/master/libkrebs/examples/assault_cube))
    /// has a pointer at static address `0x0050F4F4` that points to the player object. This object
    /// contains a gun object at offset `0x374`, which in turn contains the reserve ammo amount
    /// at offset `0x10`.
    ///
    /// The reserve ammo address can thus be located with base address `0x0050F4F4`
    /// and offset list `vec![0x00, 0x374, 0x10]`.
    #[allow(unused_unsafe)]
    unsafe fn deref_chain(
        &mut self,
        start_addr: MemAddress,
        offsets_v: &Vec<MemAddress>,
    ) -> Result<MemAddress, MemError> {
        debug!("Deref Chain {:?}", offsets_v);
        let mut offsets = offsets_v.clone();

        assert_ne!(offsets.len(), 0, "Deref Chain has a length of 0");
        let last_offset: MemAddress = offsets.pop().unwrap();

        if offsets.len() == 0 {
            return Ok(start_addr + last_offset);
        }

        unsafe {
            let next_ptr = |addr, offset| {
                let ptr_and_off = (addr + offset) as MemAddress;
                debug!("{:#08X} + {:#08X} = {:#08X}", addr, offset, ptr_and_off);
                let new_addy: MemAddress = self.read_to_type(ptr_and_off).unwrap();
                assert_ne!(
                    new_addy, 0,
                    "Null Pointer in Chain: *({:#08X} + {:#08X}) == 0",
                    addr, offset
                );
                debug!("[{:#08X} + {:#08X}] -> {:#08X}", addr, offset, new_addy);
                return new_addy;
            };

            Ok(offsets.iter().fold(start_addr, next_ptr) + last_offset)
        }
    }
}

/// An object that handles editing the memory of another process.
pub trait Writer {
    /// Writes the first `size` bytes of `buffer` to the given address.
    ///
    /// Returns the number of bytes written.
    ///
    /// # Safety
    ///
    /// Trivially unsafe, as it does not check the bounds of the given buffer.
    #[allow(unused_unsafe)]
    unsafe fn write_n_bytes(
        &mut self,
        base_addr: MemAddress,
        bytes: &[u8],
        size: usize,
    ) -> Result<usize, MemError>;

    /// Writes the entire buffer to the given address.
    ///
    /// # Errors
    ///
    /// Propagates errors from [`write_n_bytes`](Writer::write_n_bytes).
    fn write_from_buffer(&mut self, base_addr: MemAddress, bytes: &[u8]) -> Result<usize, MemError> {
        unsafe { self.write_n_bytes(base_addr, bytes, bytes.len()) }
    }

    /// Writes a value of the given type to the given address.
    ///
    /// # Errors
    ///
    /// Propagates errors from [`write_n_bytes`](Writer::write_n_bytes).
    fn write_from_type<T: Copy>(&mut self, base_addr: MemAddress, value: T) -> Result<usize, MemError> {
        self.write_from_buffer(base_addr, &to_bytes(value))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn addr_range_constructor() {
        let range = AddressRange::new(0, 0x100);
        assert_eq!(range.size(), 0x100);
    }

    #[test]
    #[should_panic]
    fn addr_range_constructor_failure() {
        AddressRange::new(0x100, 0);
    }

    #[test]
    fn addr_chunks() {
        assert_eq!(
            AddressRange::new(000, 350).chunk_by(100),
            vec![
                AddressRange::new(000, 100),
                AddressRange::new(100, 200),
                AddressRange::new(200, 300),
                AddressRange::new(300, 350),
            ]
        );
    }

    #[test]
    fn addr_merge() {
        let hugging_addrs = vec![
            AddressRange::new(0x000, 0x100),
            AddressRange::new(0x050, 0x150),
            AddressRange::new(0x150, 0x200),
            AddressRange::new(0x250, 0x300),
            AddressRange::new(0x350, 0x400),
            AddressRange::new(0x400, 0x450),
            AddressRange::new(0x450, 0x500),
        ];

        let merged_addrs = AddressRange::merge(&hugging_addrs);

        assert_eq!(
            merged_addrs,
            vec![
                AddressRange::new(0x000, 0x200),
                AddressRange::new(0x250, 0x300),
                AddressRange::new(0x350, 0x500)
            ]
        );
    }

    #[test]
    fn addr_macro() {
        assert_eq!(AddressRange::new(0x000, 0x100), AddressRange::new(0x000, 0x100));
    }
}
