//! Rust representation of GCC `std::string`.
//!
//! See the [`vcpp::string`](crate::cpp_std::vcpp::string) module for a more detailed
//! explanation of the layout and validation strategy.

use std::iter;
use std::mem::size_of;

use crate::error::{
    gcc::{LongStringError, ShortStringError, StringError},
    CommonStringError, StdError,
};
use crate::pattern_scan::{BytePattern, FromBytes, ScanPattern};

use crate::Verifiable;

use crate::mem::{MemAddress, Reader};
use crate::util::to_bytes;

impl Verifiable for CppUnknownString {}

/// An std::string (gcc) with up to 16 characters (including null terminator).
///
/// Consists of a pointer to the string (which is in the struct),
/// the string length, and the string itself.
#[repr(C)]
#[derive(Copy, Clone)]
pub struct CppShortString {
    pub c_str: MemAddress,
    /// The length of the string.
    pub length: usize,
    /// A buffer containing the null-terminated string.
    ///
    /// The bytes after the null terminator are either
    /// 0xCC (debug) or garbage data (release).
    pub string: [u8; 16],
}

impl CppShortString {
    /// Converts the internal buffer into a Rust `String`.
    ///
    /// # Errors
    ///
    /// Returns an error if the internal buffer or string encoding is invalid.
    pub fn to_string(&self) -> Result<String, CommonStringError> {
        let ebin_part_of_string = &self.string[0..(self.length + 1) as usize];
        check_buffer(ebin_part_of_string, self.length as usize)?;

        Ok(
            String::from_utf8_lossy(&ebin_part_of_string[0..ebin_part_of_string.len() - 1])
                .to_string(),
        )
    }
}

impl Verifiable for CppShortString {
    type Error = StdError;

    /// Makes assertions about the string to ensure that it is probably a valid std::string.
    ///
    /// This function fully validates short strings. Long strings require additional validation
    /// with [`validate_long`](CppString::validate_long) to ensure that the pointer contains the right string.
    ///
    /// If successful, returns the size of the string.
    ///
    /// # Errors
    ///
    /// Returns an error if the string is invalid.
    fn verify_shallow(&self) -> Result<(), StdError> {
        if self.length < 1 || self.length > 15 {
            Err(StdError::Gcc(StringError::Short(ShortStringError::InvalidLength {
                length: self.length,
            }).into()))
        } else {
            let buf = &self.string[0..(self.length + 1) as usize];
            check_buffer(buf, self.length as usize)?;

            Ok(())
        }
    }
}

/// An std::string (gcc) with more than 16 characters (including null terminator).
///
/// Consists of a pointer to the string, the string length, the string capacity,
/// and some other data (I dunno what).
#[repr(C)]
#[derive(Copy, Clone)]
pub struct CppLongString {
    /// The address of the C string.
    pub c_str: MemAddress,
    /// The length of the string.
    pub length: usize,
    /// The capacity of the string buffer (not incl. null)
    pub capacity: usize,
    /// idk what this is.
    ///
    /// it changes dramatically when i copy the string
    /// so i think it's a pointer to some sort of refcount structure
    pub unk: [u8; 8],
}

impl CppLongString {
    /// Returns the referenced C string as a `Vec<u8>`.
    ///
    /// # Errors
    ///
    /// Returns an error if the read fails, or if the
    /// C string is invalid.
    pub fn get_c_str<T: Reader>(&self, reader: &mut T) -> Result<Vec<u8>, CommonStringError> {
        let mut str_buf = vec![0x00; (self.length + 1) as usize];

        reader.read_to_buffer(self.c_str, str_buf.as_mut_slice())
            .map_err(CommonStringError::Io)?;

        check_buffer(&str_buf, self.length as usize)?;

        Ok(str_buf)
    }

    /// Returns the referenced C string as a Rust `String`.
    ///
    /// # Errors
    ///
    /// Returns an error if the read fails or if the C string
    /// or encoding are invalid.
    pub fn to_string<T: Reader>(&self, reader: &mut T) -> Result<String, CommonStringError> {
        let c_str = self.get_c_str(reader)?;
        let no_null = Vec::from(&c_str[..c_str.len() - 1]);

        Ok(String::from_utf8(no_null)?)
    }
}

impl Verifiable for CppLongString {
    type Error = StdError;

    /// Makes assertions about the string to ensure that it is probably a valid std::string.
    ///
    /// This function fully validates short strings. Long strings require additional validation
    /// with [`validate_long`](CppString::validate_long) to ensure that the pointer contains the right string.
    ///
    /// If successful, returns the size of the string.
    ///
    /// # Errors
    ///
    /// Errors if the string is invalid.
    fn verify_shallow(&self) -> Result<(), StdError> {
        if self.c_str == 0 {
            Err(StdError::Gcc(StringError::Long(LongStringError::NullPtr).into()))
        } else if self.capacity < self.length {
            Err(StdError::Gcc(StringError::Long(LongStringError::CapacityTooSmall {
                capacity: self.capacity,
                length: self.length,
            }).into()))
        } else {
            Ok(())
        }
    }
}

/// An std::string (gcc) of unknown type.
///
/// Used internally by [`CppString`].
#[repr(C)]
#[derive(Copy, Clone)]
pub struct CppUnknownString {
    pub c_str: MemAddress,
    /// The string length.
    pub length: usize,
    /// The string allocation size.
    _unk2: [u8; 16],
}

pub enum CppStringValue {
    Short(CppShortString),
    Long(CppLongString),
}

impl CppStringValue {
    pub fn from_unk(unk: CppUnknownString) -> Result<Self, StdError> {
        if unk.length < 16 {
            let shorty: CppShortString = unsafe { std::mem::transmute(unk) };
            shorty.verify_shallow()?;
            Ok(CppStringValue::Short(shorty))
        } else {
            let long_boy: CppLongString = unsafe { std::mem::transmute(unk) };
            long_boy.verify_shallow()?;
            Ok(CppStringValue::Long(long_boy))
        }
    }
}

/// Tests whether the buffer contains a C string of the given length.
///
/// `len` is the number of payload bytes, not including the null terminator.
/// The buffer must therefore be exactly `len + 1` bytes.
///
/// # Errors
///
/// Errors if the buffer does not contain a valid C string of the given length.
fn check_buffer(bytes: &[u8], len: usize) -> Result<(), CommonStringError> {
    if bytes.len() != len + 1 {
        Err(CommonStringError::BufferLengthMismatch {
            buf_len: bytes.len(),
            str_len: len,
        })
    } else if bytes[len] != b'\0' {
        Err(CommonStringError::NoNullTerminator { str_len: len })
    } else if bytes[..len].iter().any(|x| *x == b'\0') {
        Err(CommonStringError::EmbeddedNullBytes)
    } else {
        Ok(())
    }
}

/// Creates a scan pattern for the given string.
pub fn str_to_pattern(s: &str) -> BytePattern {
    let len: usize = s.len() as usize;
    let len_as_bytes = to_bytes(len);

    if len < 16 {
        let str_field_pattern = BytePattern::new(
            s.bytes().chain(iter::once(0x00 as u8)).collect(), // Null Terminator
            None,
        ) + BytePattern::ignore((15 - len) as usize);

        debug_assert_eq!(
            str_field_pattern.len(),
            16,
            "string field vector not filled"
        );

        BytePattern::ignore(size_of::<MemAddress>())
            + BytePattern::new(len_as_bytes, None)
            + str_field_pattern
    } else {
        BytePattern::ignore(size_of::<MemAddress>())
            + BytePattern::new(len_as_bytes, None)
            + BytePattern::ignore(16)
    }
}

#[cfg(test)]
mod tests {
    // These test bytes are taken from actual C++ objects that I inspected in memory
    // Obviously I can't generate them on the fly
    use super::*;

    #[test]
    fn short_strings() {
        let str_short1 = [
            0xe0, 0x74, 0x5d, 0xc0, 0xfd, 0x7f, 0x00, 0x00, 0x07, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x74, 0x65, 0x65, 0x20, 0x68, 0x65, 0x65, 0x00, 0xdf, 0x86, 0x41, 0xd5,
            0x53, 0x56, 0x00, 0x00,
        ];
        let str_short2 = [
            0xe0, 0x70, 0x01, 0x2b, 0xff, 0x7f, 0x00, 0x00, 0x07, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x70, 0x65, 0x65, 0x20, 0x70, 0x65, 0x65, 0x00, 0xdf, 0x76, 0x28, 0x25,
            0x05, 0x56, 0x00, 0x00,
        ];
        let str_short3 = [
            0x90, 0xc7, 0xe2, 0x4a, 0xff, 0x7f, 0x00, 0x00, 0x07, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x70, 0x6f, 0x6f, 0x20, 0x70, 0x6f, 0x6f, 0x00, 0xdf, 0x16, 0xbe, 0xae,
            0x7b, 0x55, 0x00, 0x00,
        ];

        CppShortString::from_bytes(&str_short1).unwrap();
        CppShortString::from_bytes(&str_short2).unwrap();
        CppShortString::from_bytes(&str_short3).unwrap();
    }

    #[test]
    #[should_panic]
    fn bad_short_strings() {
        let bad_str_short1 = [
            0xe0, 0x02, 0xb1, 0xf4, 0xfe, 0x7f, 0x00, 0x00, 0x07, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x74, 0x65, 0x65, 0x20, 0x68, 0x65, 0x65, 0x65, 0x00, 0x56, 0xf4, 0xe4,
            0x74, 0x55, 0x00, 0x00,
        ];

        CppShortString::from_bytes(&bad_str_short1).unwrap();
    }

    #[test]
    fn long_strings() {
        let str_long1 = [
            0xc0, 0x42, 0xfa, 0x5a, 0x24, 0x56, 0x00, 0x00, 0x07, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x25, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x12, 0xc7, 0xef, 0x5a,
            0x24, 0x56, 0x00, 0x00,
        ];
        let str_long2 = [
            0xc0, 0x72, 0x36, 0x99, 0x39, 0x56, 0x00, 0x00, 0x07, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x25, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x12, 0x07, 0x8f, 0x98,
            0x39, 0x56, 0x00, 0x00,
        ];
        let str_long3 = [
            0xc0, 0x22, 0x38, 0xac, 0x25, 0x56, 0x00, 0x00, 0x07, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x25, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x12, 0x47, 0x8a, 0xab,
            0x25, 0x56, 0x00, 0x00,
        ];

        CppLongString::from_bytes(&str_long1).unwrap();
        CppLongString::from_bytes(&str_long2).unwrap();
        CppLongString::from_bytes(&str_long3).unwrap();
    }

    #[test]
    #[should_panic]
    fn bad_long_strings() {
        let bad_str_long1 = [
            0xc0, 0x22, 0x38, 0xac, 0x25, 0x56, 0x00, 0x00, 0x07, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x06, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x12, 0x47, 0x8a, 0xab,
            0x25, 0x56, 0x00, 0x00,
        ];

        CppLongString::from_bytes(&bad_str_long1).unwrap();
    }

    #[test]
    fn string_masks() {
        let short_test_str = String::from("lol poop");
        let short_test_pattern = str_to_pattern(&short_test_str);
        let short_test_correct_pattern = [
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 'l' as u8, 'o' as u8, 'l' as u8, ' ' as u8, 'p' as u8, 'o' as u8,
            'o' as u8, 'p' as u8, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];
        let short_test_correct_mask = [
            false, false, false, false, false, false, false, false, true, true, true, true, true,
            true, true, true, true, true, true, true, true, true, true, true, true, false, false,
            false, false, false, false, false,
        ];

        assert_eq!(short_test_pattern.len(), short_test_correct_pattern.len());
        assert_eq!(
            short_test_pattern.value(),
            &short_test_correct_pattern[..]
        );
        assert_eq!(
            short_test_pattern.mask().unwrap(),
            &short_test_correct_mask[..]
        );
    }

    #[test]
    fn pass_the_vibe_check() {
        let test_str = "Once upon a time and a very good time it was there was a moocow coming down along the road and this moocow that was coming down along the road met a nicens little boy named baby tuckoo";
        let test_str_bytes: Vec<u8> = test_str.bytes().chain(std::iter::once(0x00)).collect();

        check_buffer(&test_str_bytes, test_str.len()).unwrap();
    }

    #[test]
    #[should_panic]
    fn fail_the_vibe_check() {
        let test_str = "Once upon a time and a very good time it was there was a m\0ocow coming down along the road and this moocow that was coming down along the road met a nicens little boy named baby tuckoo";
        let test_str_bytes: Vec<u8> = test_str.bytes().collect();

        check_buffer(&test_str_bytes, test_str.len()).unwrap();
    }
}
