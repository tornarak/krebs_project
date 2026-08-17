//! Rust representation of Visual C++ `std::string`.
//!
//! Credit to [Shahar Mike](https://shaharmike.com/) for his
//! [research on std::string](https://archive.li/Agazq).
//!
//! # Safety note
//!
//! Deserializing a struct from a memory address does NOT confirm that the
//! memory actually contains that type — it only rules out structurally invalid
//! states. False positives (arbitrary bytes that happen to pass validation) and
//! false negatives (valid objects modified by the host process) are both possible.

use std::iter;

use crate::error::{vcpp::{LongStringError, ShortStringError, StringError}, CommonStringError, StdError};
use crate::pattern_scan::{BytePattern, FromBytes, ScanPattern};

use crate::Verifiable;

use crate::mem::{MemAddress, Reader};
use crate::util::{concat_vecs, to_bytes};

// the general strategy is 1.5x but the rounding is inconsistent
// (The actual memory allocated includes the null terminator, making it one greater than
// the alloc size variable in the string, which doesn't; this array represents the latter)
const ALLOC_SIZES: [MemAddress; 28] = [
    0x7, 0x1F, 0x2F, 0x46, 0x69, 0x9D, 0xEB, 0x160, 0x210, 0x318, 0x4A4, 0x6F6, 0xA71, 0xFA9,
    0x17A4, 0x2362, 0x34FF, 0x4F6B, 0x770D, 0xB280, 0x10BAC, 0x1916E, 0x25A11, 0x38706, 0x54A75,
    0x7EF9C, 0xBE756, 0x11DAED,
];

/// An std::string (VC++) of unknown type.
///
/// Used internally by [`CppString`].
#[repr(C)]
#[derive(Copy, Clone)]
pub struct CppUnknownString {
    _unk: [u8; 16],
    /// The string length.
    pub length: MemAddress,
    /// The string allocation size.
    pub alloc_size: MemAddress,
}

impl Verifiable for CppUnknownString {}

/// An std::string (VC++) with up to 16 characters (including null terminator).
///
/// Consists of a 16-byte buffer,
/// the string length,
/// and the allocated space (always 15).
///
/// The bytes in the buffer after the
/// null terminator are set to 0xCC in debug mode.
/// In release mode they're just garbage data.
#[repr(C)]
#[derive(Copy, Clone)]
pub struct CppShortString {
    //	_junk: 				u32, 		// only exists in debug mode
    /// A buffer containing the null-terminated string.
    ///
    /// The bytes after the null terminator are either
    /// 0xCC (debug) or garbage data (release).
    pub string: [u8; 16],
    /// The length of the string.
    pub length: MemAddress,
    /// The space allocated for the string.
    /// Always 0x0000000F (15).
    pub alloc_size: MemAddress,
}

/// An std::string (VC++) with more than 16 characters (including null terminator).
///
/// Consists of the word-sized pointer to the C string, garbage bytes filling out the rest
/// of the 16-byte union shared with the short-string buffer, the string length, and the
/// allocated space.
///
/// The union is always 16 bytes regardless of target bitness (its size is set by the inline
/// short-string buffer capacity, not by pointer width) — so the junk gap shrinks as the
/// pointer grows, but `length`/`alloc_size` always start at byte offset 16.
///
/// Growth strategy is 1.5x.
#[repr(C)]
#[derive(Copy, Clone)]
pub struct CppLongString {
    //	_junk: 				u32, 		// debug only
    /// The address of the C string.
    pub c_str_addr: MemAddress,
    _junk2: [u8; 16 - std::mem::size_of::<MemAddress>()],
    /// The length of the string.
    pub length: MemAddress,
    /// The space allocated for the string.
    ///
    /// The minimum for long strings is 32;
    /// further allocation sizes increase exponentially by a factor of 1.5.
    pub alloc_size: MemAddress,
}

impl CppShortString {
    /// Converts the internal buffer into a Rust `String`.
    ///
    /// # Errors
    ///
    /// Returns an error if the internal buffer or string encoding is invalid.
    pub fn to_string(&self) -> Result<String, CommonStringError> {
        let ebin_part_of_string = &self.string[0..self.length + 1];
        check_buffer(ebin_part_of_string, self.length)?;

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
        if self.length == 0 {
            Err(StdError::Vcpp(StringError::Short(ShortStringError::ZeroLength).into()))
        } else if self.alloc_size != 0x0F {
            Err(StdError::Vcpp(StringError::Short(ShortStringError::BadAllocSize { got: self.alloc_size }).into()))
        } else {
            let buf = &self.string[0..self.length + 1];
            check_buffer(buf, self.length)?;

            Ok(())
        }
    }
}

impl CppLongString {
    /// Returns the referenced C string as a `Vec<u8>`.
    ///
    /// # Errors
    ///
    /// Returns an error if the read fails, or if the
    /// C string is invalid.
    pub fn get_c_str<T: Reader>(&self, reader: &mut T) -> Result<Vec<u8>, CommonStringError> {
        let mut str_buf = vec![0x00; self.length + 1];

        reader.read_to_buffer(self.c_str_addr, str_buf.as_mut_slice())
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
        let no_null = Vec::from(&c_str[0..c_str.len() - 1]);

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
        if self.length < 16 {
            Err(StdError::Vcpp(StringError::Long(LongStringError::TooShort { length: self.length }).into()))
        } else if self.alloc_size < 15 {
            Err(StdError::Vcpp(StringError::Long(LongStringError::AllocTooSmall { alloc_size: self.alloc_size }).into()))
        } else if !ALLOC_SIZES.contains(&self.alloc_size) {
            Err(StdError::Vcpp(StringError::Long(LongStringError::InvalidAllocSize { alloc_size: self.alloc_size }).into()))
        } else if self.c_str_addr == 0 {
            Err(StdError::Vcpp(StringError::Long(LongStringError::NullPtr).into()))
        } else {
            Ok(())
        }
    }
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

// pub enum CppString<U: Reader> {
//     Short(WeakForeign<CppShortString, U>),
//     Long(WeakForeign<CppLongString, U>),
// }

// impl<'a, U: Reader> CppString<U> {
//     /// Returns a weak pointer to the given Address.
//     pub fn from_addr(reader: Arc<U>, addr: MemAddress) -> Result<Self, String> {
//         let unk_str: CppUnknownString = try_or_string! { reader.read_to_type(addr) };

//         if unk_str.length < 16 {
//             Ok(CppString::Short(WeakForeign::new(reader, addr)))
//         } else {
//             Ok(CppString::Long(WeakForeign::new(reader, addr)))
//         }
//     }

//     /// Wrapper for [`CppShortString::to_string`] or [`CppLongString::to_string`],
//     /// depending on the string type.
//     pub fn to_string(&self) -> Result<String, String> {
//         match self {
//             CppString::Short(shorty) => shorty.to_string(),
//             CppString::Long(longy) => longy.to_string(),
//         }
//     }
// }

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
        Err(CommonStringError::BufferLengthMismatch { buf_len: bytes.len(), str_len: len })
    } else if bytes[len] != b'\0' {
        Err(CommonStringError::NoNullTerminator { str_len: len })
    } else if bytes[..len].iter().any(|x| *x == b'\0') {
        Err(CommonStringError::EmbeddedNullBytes)
    } else {
        Ok(())
    }
}

/// Gets the `alloc_size` value of an std::string with
/// the given length.
///
/// For some God-forsaken reason, the `alloc_size` field
/// in an std::string does not include the null terminator.
/// The actual space allocated is thus one more than this
/// function's return value.
///
/// Short strings always have 15 (actually 16) bytes allocated.
///
/// The smallest size for a long string is 31 (actually 32) bytes;
/// the subsequent sizes are obtained by multiplying the next
/// smallest size by 1.5 exponentially.
///
/// Don't try to implement this yourself - it's some
/// of the weirdest weakly typed rounding I've ever seen,
/// and the off-by-one errors will make you want to do
/// a trust fall with eight feet of taut rope.
///
/// # Panics
///
/// Panics if the length is greater than 1,170,157.
/// Will anyone ever legitimately encounter this error?
pub fn get_alloc_size(len: MemAddress) -> MemAddress {
    if len < 16 {
        0x0000000F
    } else {
        // Get largest alloc size that won't fit the string,
        // then return its successor
        match ALLOC_SIZES.iter().find(|x| **x > len) {
            Some(val) => *val,
            None => {
                panic!("too big to determine alloc size");
            }
        }
    }
}

/// Creates a scan pattern for the given string.
///
/// # Panics
///
/// Panics if the string is over 1,170,157 bytes.
/// Why are you looking for such a big string?
pub fn str_to_pattern(s: &str) -> BytePattern {
    let len: MemAddress = s.len();
    let alloc_size = get_alloc_size(len);
    let len_and_alloc_as_bytes = concat_vecs(to_bytes(len), to_bytes(alloc_size)); // Glues the vectors together

    if len < 16 {
        let str_field_pattern = BytePattern::new(
            s.bytes().chain(iter::once(0x00 as u8)).collect(), // Null Terminator
            None,
        ) + BytePattern::ignore(15 - len);

        debug_assert_eq!(
            str_field_pattern.len(),
            16,
            "string field vector not filled"
        );

        str_field_pattern + BytePattern::new(len_and_alloc_as_bytes, None)
    } else {
        BytePattern::ignore(16) + BytePattern::new(len_and_alloc_as_bytes, None)
    }
}

// The three `Cpp*String` structs must stay the same total size — `CppStringValue::from_unk`
// `transmute`s between them. Checked at compile time, for both bitnesses.
const _: () = assert!(
    std::mem::size_of::<CppUnknownString>() == std::mem::size_of::<CppShortString>()
);
const _: () = assert!(
    std::mem::size_of::<CppUnknownString>() == std::mem::size_of::<CppLongString>()
);

#[cfg(all(test, target_pointer_width = "32"))]
mod tests_32 {
    // These test bytes are taken from actual C++ objects that I inspected in memory
    // (a real 32-bit MSVC process) — obviously I can't generate them on the fly.
    use super::*;

    #[test]
    fn short_strings() {
        let str_short1 = [
            0x50, 0x61, 0x72, 0x74, 0x0, 0xcc, 0xcc, 0xcc, 0xcc, 0xcc, 0xcc, 0xcc, 0xcc, 0xcc,
            0xcc, 0xcc, 0x4, 0x0, 0x0, 0x0, 0xf, 0x0, 0x0, 0x0,
        ];
        let str_short2 = [
            0x50, 0x61, 0x72, 0x74, 0x0, 0xcc, 0xcc, 0xcc, 0xcc, 0xcc, 0xcc, 0xcc, 0xcc, 0xcc,
            0xcc, 0xcc, 0x4, 0x0, 0x0, 0x0, 0xf, 0x0, 0x0, 0x0,
        ];
        let str_short3 = [
            0x4e, 0x6f, 0x20, 0x50, 0x61, 0x72, 0x74, 0x0, 0xcc, 0xcc, 0xcc, 0xcc, 0xcc, 0xcc,
            0xcc, 0xcc, 0x7, 0x0, 0x0, 0x0, 0xf, 0x0, 0x0, 0x0,
        ];

        CppShortString::from_bytes(&str_short1).unwrap();
        CppShortString::from_bytes(&str_short2).unwrap();
        CppShortString::from_bytes(&str_short3).unwrap();
    }

    #[test]
    #[should_panic]
    fn bad_short_strings() {
        let bad_str_short1 = [
            0x50, 0x61, 0x0, 0x74, 0x0, 0xcc, 0xcc, 0xcc, 0xcc, 0xcc, 0xcc, 0xcc, 0xcc, 0xcc, 0xcc,
            0xcc, 0x4, 0x0, 0x0, 0x0, 0xf, 0x0, 0x0, 0x0,
        ];

        CppShortString::from_bytes(&bad_str_short1).unwrap();
    }

    #[test]
    fn long_strings() {
        let str_long1 = [
            0xf8, 0x26, 0xde, 0x0, 0x44, 0x3b, 0x2, 0x0, 0x5c, 0xf7, 0xcf, 0x0, 0x5f, 0x1, 0x3,
            0x0, 0x16, 0x0, 0x0, 0x0, 0x1f, 0x0, 0x0, 0x0,
        ];
        let str_long2 = [
            0x8, 0x21, 0xde, 0x0, 0x5, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x5c, 0xf7, 0xcf, 0x0,
            0x16, 0x0, 0x0, 0x0, 0x1f, 0x0, 0x0, 0x0,
        ];
        let str_long3 = [
            0x28, 0x6b, 0xdd, 0x0, 0x5, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x5c, 0xf7, 0xcf, 0x0,
            0x2f, 0x0, 0x0, 0x0, 0x2f, 0x0, 0x0, 0x0,
        ];

        CppLongString::from_bytes(&str_long1).unwrap();
        CppLongString::from_bytes(&str_long2).unwrap();
        CppLongString::from_bytes(&str_long3).unwrap();
    }

    #[test]
    #[should_panic]
    fn bad_long_strings() {
        let bad_str_long1 = [
            0x00, 0x00, 0x0, 0x00, 0x0, 0xcc, 0xcc, 0xcc, 0xcc, 0xcc, 0xcc, 0xcc, 0xcc, 0xcc, 0xcc,
            0xcc, 0xFF, 0x0, 0x0, 0x0, 0xf, 0x0, 0x0, 0x0,
        ];

        CppLongString::from_bytes(&bad_str_long1).unwrap();
    }

    #[test]
    fn string_masks() {
        let short_test_str = String::from("lol");
        let short_test_pattern = str_to_pattern(&short_test_str);
        let short_test_correct_pattern = [
            'l' as u8, 'o' as u8, 'l' as u8, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x03, 0x00, 0x00, 0x00, 0x0F, 0x00, 0x00, 0x00,
        ];

        assert_eq!(short_test_pattern.len(), short_test_correct_pattern.len());
        assert_eq!(
            short_test_pattern.value(),
            &short_test_correct_pattern[..]
        );

        let long_test_str = String::from("lol lol lol lol lol lol");
        let long_test_pattern = str_to_pattern(&long_test_str);
        let long_test_bytes: [u8; 24] = [
            0x00, 0xF2, 0xCD, 0x00, 0x05, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xD0, 0xFE,
            0x9F, 0x00, 0x17, 0x00, 0x00, 0x00, 0x1F, 0x00, 0x00, 0x00,
        ];
        let long_test_correct_pattern: [u8; 24] = [
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x17, 0x00, 0x00, 0x00, 0x1F, 0x00, 0x00, 0x00,
        ];

        assert!(long_test_pattern.matches(&long_test_bytes[..]));
        assert_eq!(long_test_pattern.len(), short_test_correct_pattern.len());
        assert_eq!(
            long_test_pattern.value(),
            &long_test_correct_pattern[..]
        );
    }

}

// `str_to_pattern`, `CppShortString`/`CppLongString` layouts, and the fixture bytes above
// are all 32-bit-shaped (see `tests_32`). No real 64-bit MSVC capture exists yet — capturing
// one requires a live 64-bit MSVC target (see the project's Windows VM access notes) rather
// than fabricated bytes, per this module's own testing philosophy (see `tests_32`'s doc
// comment). The type-level fix (word-sized `length`/`alloc_size`/`c_str_addr`, verified via
// the compile-time size assertions above) makes this correct once such fixtures exist.
#[cfg(all(test, target_pointer_width = "64"))]
mod tests_64 {
    // TODO: needs a real capture from a 64-bit MSVC target (see Windows VM access in
    // project memory) — port `tests_32`'s short/long-string and `string_masks` fixtures
    // once real bytes are available.
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alloc_size_deduction() {
        assert_eq!(get_alloc_size(3), 15);
        assert_eq!(get_alloc_size(18), 31);
        assert_eq!(get_alloc_size(31), 47);
        assert_eq!(get_alloc_size(35), 47);
    }

    #[test]
    #[should_panic]
    fn big_af_string_alloc_size_deduction() {
        get_alloc_size(0x1337420);
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
        let test_str = "Once upon a time and a very good time it was there was a mo\0cow coming down along the road and this moocow that was coming down along the road met a nicens little boy named baby tuckoo";
        let test_str_bytes: Vec<u8> = test_str.bytes().collect();

        check_buffer(&test_str_bytes, test_str.len()).unwrap();
    }
}
