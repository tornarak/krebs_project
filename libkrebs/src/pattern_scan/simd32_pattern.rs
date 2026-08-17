use std::fmt::Display;

use super::{BytePattern, FromBytes, ScanMatch, ScanPattern, ALIGN_SIZE};
use crate::mem::MemAddress;
use crate::util::concat_masks;

use std::simd::u8x32;

/// arch pattern consisting of a single 32-byte SIMD vector.
/// The supplied value must be 32 bytes in length.
///
/// This pattern is ridiculously fast - about 70% faster than [`BytePattern`].
///
/// # Example
///
/// ```
/// #[macro_use]
/// extern crate libkrebs;
/// use libkrebs::pattern_scan::*;
/// use libkrebs::cpp_std::vcpp;
///
/// pub fn main() {
/// 	// Let's make a basic string pattern...
/// 	let byte_pattern : BytePattern = vcpp::string::str_to_pattern("string test");
///
/// 	// With SIMD vectors, our CPU can compare lots of bytes in just one instruction.
/// 	// This means that we can check for an entire string in an instant!
///     // Note that `str_to_pattern` returns a 24-byte pattern.
/// 	// The SIMD pattern constructor automatically adds the dead space at the end.
/// 	let simd_pattern : Simd32Pattern = Simd32Pattern::from(byte_pattern);
/// }
/// ```
///
/// As all comparisons take the same amount of time, the amount of dead space is not important.
/// You can scan for a 4-byte integer with 28 bytes of ignored space and it'll still probably
/// be faster than anything else.
#[derive(Clone)]
pub struct Simd32Pattern {
    mask: Option<u8x32>,
    masked_value: u8x32,

    true_len: usize,

    /// Cached byte representation for `ScanPattern::value()`.
    value_bytes: Vec<u8>,
    /// Cached bool mask for `ScanPattern::mask()`.
    mask_bools: Option<Vec<bool>>,
}

impl Simd32Pattern {
    fn with_true_len(p_value: Vec<u8>, mask: Option<Vec<bool>>, true_len: usize) -> Self {
        assert_eq!(p_value.len(), 32, "Value must be 32 bytes");
        if let Some(mask_vec) = &mask {
            assert_eq!(mask_vec.len(), 32, "Mask must be 32 bytes");
        }

        // Cache the byte/bool representations before consuming into SIMD
        let value_bytes = Vec::from(&p_value[..true_len]);
        let mask_bools: Option<Vec<bool>> = mask.as_ref().map(|m| m[..true_len].to_vec());

        let value: std::simd::prelude::Simd<u8, 32> = u8x32::from_slice(p_value.as_slice());

        let mask_simd: Option<u8x32> = mask.map(|v| {
            let bytes: Vec<u8> = v.iter().map(|b| if *b { 0xFF } else { 0x00 }).collect();
            u8x32::from_slice(&bytes)
        });

        let masked_value = match mask_simd {
            None => value,
            Some(m) => value & m,
        };

        Self {
            mask: mask_simd,
            masked_value,
            true_len,
            value_bytes,
            mask_bools,
        }
    }
}

impl From<&dyn ScanPattern> for Simd32Pattern {
    fn from(other: &dyn ScanPattern) -> Self {
        let len = other.len();
        assert!(
            len <= 32,
            "Expected pattern with <= 32 bytes, got one with {} bytes",
            len
        );

        if len < 32 {
            let padded_val = [other.value(), &vec![0x00; 32 - len]].concat();
            let padded_mask = concat_masks(other.mask(), Some(&vec![false; 32 - len]), 32);

            Self::with_true_len(padded_val, padded_mask, len)
        } else {
            Self::new(other.value().to_vec(), other.mask().map(|m| m.to_vec()))
        }
    }
}

impl From<BytePattern> for Simd32Pattern {
    fn from(other: BytePattern) -> Self {
        Self::from(&other as &dyn ScanPattern)
    }
}

impl FromBytes for Simd32Pattern {
    fn new(value: Vec<u8>, mask: Option<Vec<bool>>) -> Self {
        Self::with_true_len(value, mask, 32)
    }
}

impl ScanPattern for Simd32Pattern {
    fn len(&self) -> usize {
        32usize
    }

    fn true_len(&self) -> usize {
        self.true_len
    }

    fn value(&self) -> &[u8] {
        &self.value_bytes
    }

    fn mask(&self) -> Option<&[bool]> {
        self.mask_bools.as_deref()
    }

    fn matches(&self, buf: &[u8]) -> bool {
        debug_assert_eq!(buf.len(), 32, "Slice and pattern have unequal sizes");
        let buf_as_simd = u8x32::from_slice(buf);

        match self.mask {
            None => self.masked_value == buf_as_simd,
            Some(mask) => self.masked_value == (buf_as_simd & mask),
        }
    }

    fn scan_buffer(&self, base_addr: MemAddress, buf: &[u8]) -> Vec<ScanMatch> {
        let pattern_len = self.len();
        let data_len = self.true_len();
        assert!(buf.len() >= pattern_len, "Pattern is larger than Buffer");

        let mut matches: Vec<ScanMatch> = Vec::new();
        if let Some(mask) = self.mask {
            for (word_offset, check_slice) in
                buf.windows(pattern_len).step_by(ALIGN_SIZE).enumerate()
            {
                if self.masked_value == (u8x32::from_slice(check_slice) & mask) {
                    matches.push(ScanMatch {
                        addr: base_addr + (word_offset * ALIGN_SIZE) as MemAddress,
                        value: Vec::from(&check_slice[0..data_len]),
                    });
                }
            }
        } else {
            for (word_offset, check_slice) in
                buf.windows(pattern_len).step_by(ALIGN_SIZE).enumerate()
            {
                if self.masked_value == u8x32::from_slice(check_slice) {
                    matches.push(ScanMatch {
                        addr: base_addr + (word_offset * ALIGN_SIZE) as MemAddress,
                        value: Vec::from(&check_slice[0..data_len]),
                    });
                }
            }
        }

        matches
    }
}

impl Display for Simd32Pattern {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self as &dyn ScanPattern)
    }
}

impl PartialEq for Simd32Pattern {
    fn eq(&self, other: &Self) -> bool {
        self as &dyn ScanPattern == other as &dyn ScanPattern
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simd_pattern_value() {
        let pattern = Simd32Pattern::from(BytePattern::new(vec![1, 2, 3, 4], None));

        assert_eq!(pattern.value(), &[1, 2, 3, 4]);
    }

    #[test]
    fn simd_pattern_mask() {
        let pattern = Simd32Pattern::from(BytePattern::new(
            vec![1, 2, 3, 4],
            Some(vec![true, false, true, false]),
        ));

        assert_eq!(pattern.mask(), Some(&[true, false, true, false][..]));
    }
}
