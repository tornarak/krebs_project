//! Rust representation of Visual C++ std::vector.
//!
//! # Safety note
//!
//! Creating an object with `from_addr` DOES NOT confirm that said
//! memory location actually contains said object, as this task
//! is impossible. It only rules out the bad states of said object.
//!
//! It is possible, when scanning for one of these objects, to
//! pick up a totally irrelevant sequence of bytes that simply
//! bears an uncanny resemblance to an instance of the object,
//! as if an [evil demon](https://en.wikipedia.org/wiki/Evil_demon) had created
//! it in order to trick you! Likewise, the scanner could also miss a valid
//! object that failed the validations, either due to my own incompetence
//! or because it has been modified by the host process to the point
//! where it cannot be recognized.

use std::mem::size_of;

use crate::error::vcpp::VectorError;
use crate::mem::{MemAddress, Reader};
use crate::Verifiable;

/// An std::vector (VC++).
///
/// Consists of the address of the first element,
/// the address of the last element,
/// and the address of the end of the underlying array.
///
/// In debug mode, this is preceded by 4 bytes of unknown data.
#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct CppVector<T: Copy + Clone> {
    // Probably a vftable or iterator
    //	idfk : 			u32,
    /// The address of the first element.
    pub first_addr: u32,
    /// The address of the last element.
    pub last_addr: u32,
    /// The address of the end of the array.
    pub arr_end: u32,

    phantom: std::marker::PhantomData<*const T>,
}

impl<T: Copy + Clone + Send + Sync> CppVector<T> {
    /// Returns the size of the backing array in bytes.
    pub fn array_size(&self) -> usize {
        (self.arr_end - self.first_addr) as usize
    }

    /// Returns the size of the vector in bytes.
    pub fn byte_size(&self) -> usize {
        (self.last_addr - self.first_addr) as usize
    }

    /// Returns the size of the vector in elements.
    pub fn len(&self) -> usize {
        self.byte_size() / size_of::<T>()
    }

    /// Converts the vector to a Rust `Vec<T>`.
    ///
    /// # Errors
    ///
    /// Fails if the read fails.
    pub fn to_vec<U: Reader>(&self, reader: &mut U) -> Result<Vec<T>, VectorError> {
        let mut element_buf: Vec<u8> = vec![0x00; self.byte_size()];

        reader.read_to_buffer(self.first_addr as MemAddress, &mut element_buf)?;

        Ok(Vec::from(unsafe {
            std::slice::from_raw_parts(element_buf.as_ptr() as *const T, self.len())
        }))
    }
}

unsafe impl<T: Copy + Clone + Send + Sync> Send for CppVector<T> {}
unsafe impl<T: Copy + Clone + Send + Sync> Sync for CppVector<T> {}

impl<T: Copy + Clone + Send + Sync> Verifiable for CppVector<T> {
    type Error = VectorError;

    fn verify_shallow(&self) -> Result<(), VectorError> {
        if self.first_addr >= self.last_addr {
            Err(VectorError::InvalidRange { first_addr: self.first_addr, last_addr: self.last_addr })
        //		} else if self.arr_end < self.last_addr {
        //			Err(...)
        } else {
            let diff: u32 = self.last_addr - self.first_addr;
            let element_size: u32 = std::mem::size_of::<T>() as u32;
            if diff % element_size != 0 {
                Err(VectorError::Misaligned { diff, element_size })
            } else {
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    // These test bytes are taken from actual C++ objects that I inspected in memory
    // Obviously I can't generate them on the fly
    use super::*;
    use crate::util::from_bytes;
    use crate::Verifiable;

    #[test]
    fn vecs() {
        let vec0 = [
            0x90, 0x1e, 0xde, 0x0, 0xa0, 0x1e, 0xde, 0x0, 0xa0, 0x1e, 0xde, 0x0,
        ];
        let vec1 = [
            0xf8, 0xd6, 0xdd, 0x0, 0xc0, 0xd7, 0xdd, 0x0, 0xc0, 0xd7, 0xdd, 0x0,
        ];

        let cpp_vec0: CppVector<u32> = from_bytes(&vec0);
        let cpp_vec1: CppVector<char> = from_bytes(&vec1);

        cpp_vec0.verify_shallow().unwrap();
        cpp_vec1.verify_shallow().unwrap();
    }

    #[test]
    #[should_panic]
    fn bad_vecs() {
        let bad_vec = [
            0x90, 0x1e, 0xde, 0x0, 0xa0, 0x1e, 0xce, 0x0, 0xa0, 0x1e, 0xde, 0x0,
        ];

        let cpp_vec0: CppVector<u32> = from_bytes(&bad_vec);

        cpp_vec0.verify_shallow().unwrap();
    }
}
