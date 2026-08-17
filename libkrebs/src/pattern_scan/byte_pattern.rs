use std::fmt::Display;
use std::ops::Add;

use super::{FromBytes, ScanPattern};
use crate::util::concat_masks;

/// The simplest scan pattern.
/// Its underlying representation is simply a vector of bytes.
///
/// Can scan for data of any length at the cost of speed.
/// Still extremely fast - in my benchmarks, about 1 GB per second.
#[derive(Clone, Debug)]
pub struct BytePattern {
    value: Vec<u8>,
    mask: Option<Vec<bool>>,
}

impl From<&dyn ScanPattern> for BytePattern {
    fn from(pattern: &dyn ScanPattern) -> Self {
        Self::new(pattern.value().to_vec(), pattern.mask().map(|m| m.to_vec()))
    }
}

impl FromBytes for BytePattern {
    fn new(value: Vec<u8>, mask: Option<Vec<bool>>) -> BytePattern {
        if let Some(mask_vec) = &mask {
            assert_eq!(
                mask_vec.len(),
                value.len(),
                "Mask and Pattern are not the same size"
            );
        }

        BytePattern { value, mask }
    }
}

impl ScanPattern for BytePattern {
    fn len(&self) -> usize {
        self.value.len()
    }

    fn value(&self) -> &[u8] {
        &self.value
    }

    fn mask(&self) -> Option<&[bool]> {
        self.mask.as_deref()
    }

    fn matches(&self, buf: &[u8]) -> bool {
        debug_assert_eq!(
            self.len(),
            buf.len(),
            "Slice and pattern have unequal sizes"
        );

        if let Some(mask_vec) = &self.mask {
            self.value
                .iter()
                .zip(buf.iter()) // (Element of value, Element of buf)
                .zip(mask_vec.iter()) // ((Element of value, Element of buf), Compare?)
                .all(|((e0, e1), cmp)| !cmp || e0 == e1) // Hole in mask OR elements are equal
        } else {
            self.value
                .iter()
                .zip(buf.iter()) // (Element of value, Element of buf)
                .all(|(e0, e1)| e0 == e1) // Equality Test
        }
    }
}

impl Add for BytePattern {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        let new_len = self.value.len() + other.value.len();
        let new_value = [&self.value[..], &other.value[..]].concat();
        let new_mask = concat_masks(
            self.mask.as_deref(),
            other.mask.as_deref(),
            new_len,
        );

        BytePattern::new(new_value, new_mask)
    }
}

impl Display for BytePattern {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self as &dyn ScanPattern)
    }
}

impl PartialEq for BytePattern {
    fn eq(&self, other: &Self) -> bool {
        self as &dyn ScanPattern == other as &dyn ScanPattern
    }
}

#[cfg(test)]
mod tests {
    use super::super::{bytes_mask, str_mask, FromBytes, ScanPattern};
    use super::*;

    #[test]
    fn create_patterns() {
        let bytes: [u8; 4] = [0x68, 0x65, 0x68, 0x65];
        let byte_vec: Vec<u8> = Vec::from(&bytes[..]);
        let b_string = "hehe";

        let s0 = BytePattern::new(byte_vec, None);
        let s1 = BytePattern::from_bytes(&bytes, None);
        let s2 = BytePattern::from_str(b_string, None);

        assert!(s0
            .value()
            .iter()
            .zip(s1.value().iter())
            .zip(s2.value().iter())
            .all(|((a, b), c)| a == b && b == c));

        let mask_vec = vec![true, false, false, true];
        let mask_byte_vec: [u8; 4] = [0xFF, 0x00, 0x00, 0xFF];
        let mask_str = "X??X";

        let m0 = BytePattern::with_mask(&s0, Some(mask_vec));
        let m1 = BytePattern::with_mask(&s1, Some(bytes_mask(&mask_byte_vec)));
        let m2 = BytePattern::with_mask(&s2, Some(str_mask(mask_str)));

        println!("{} {} {}", &m0, &m1, &m2);

        assert!(m0
            .mask()
            .iter()
            .zip(m1.mask().iter())
            .zip(m2.mask().iter())
            .all(|((a, b), c)| a == b && b == c));

        println!("{} {} {}\n{} {} {}", s0, s1, s2, m0, m1, m2);
    }

    #[test]
    #[should_panic]
    fn bad_mask() {
        let sp = BytePattern::new(vec![1, 2, 3, 4], None);
        BytePattern::with_mask(&sp, Some(vec![true, true, false, false, true]));
    }

    #[test]
    fn test_unmasked_query() {
        let query = vec![0x20, 0x30, 0x30, 0x70];
        let pattern: BytePattern = BytePattern::new(query, None);
        let ez_match: [u8; 4] = [0x20, 0x30, 0x30, 0x70];
        assert!(pattern.matches(&ez_match[..]));

        let harder_match: [u8; 16] = [
            0x00, 0x20, 0x30,
            0x30, // Although the pattern shows up twice in the linear sequence,
            0x70, 0x10, 0x15, 0x25, // In actual memory all variables are word-aligned,
            0x20, 0x30, 0x30, 0x70, // So it should only match once, with the aligned value
            0xFF, 0xFF, 0x7F, 0x7F, // padding bytes, not part of the pattern
        ];

        let query_result = pattern.scan_buffer(0x00000000, &harder_match);
        assert_eq!(query_result.len(), 1);
        assert_eq!(query_result.get(0).unwrap().addr, 0x8);
    }

    #[test]
    fn pattern_concatenation() {
        let bytes: [u8; 4] = [0x68, 0x65, 0x65, 0x68];
        let mask: [bool; 4] = [true, false, true, true];
        let pen = BytePattern::from_str("he", Some(vec![true, false]));
        let apple = BytePattern::from_str("eh", None);
        let apple_pen: BytePattern = pen + apple;

        assert_eq!(apple_pen.value(), &bytes[..]);
        assert_eq!(apple_pen.mask().unwrap(), &mask[..]);
    }

    #[test]
    fn pattern_constructors() {
        let nools = BytePattern::null(5);
        let nahs = BytePattern::ignore(7);
        let easy_pattern = BytePattern::new(vec![0x01, 0x02, 0x03, 0x04], None);
        let yeezy_pattern = BytePattern::new(
            vec![0x4B, 0x41, 0x4E, 0x59, 0x45],
            Some(str_mask("???XX")),
        );
        let ebin_int = BytePattern::from_primitive(0x0F as u32, None);

        assert_eq!(nools, BytePattern::new(vec![0x00; 5], None));

        assert_eq!(nahs, BytePattern::new(vec![0x00; 7], Some(vec![false; 7])));

        assert_eq!(
            easy_pattern,
            BytePattern::new(vec![0x01, 0x02, 0x03, 0x04], None)
        );

        assert_eq!(
            yeezy_pattern,
            BytePattern::from_str("KANYE", Some(vec![false, false, false, true, true]))
        );

        assert_eq!(
            ebin_int,
            BytePattern::new(vec![0x0F, 0x00, 0x00, 0x00], None)
        );
    }

    #[test]
    fn simd() {}
}
