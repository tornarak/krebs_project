use std::fmt::Display;
use std::mem::size_of;
use std::ops::*;
use std::slice;

use super::{BytePattern, FromBytes, ScanMatch, ScanPattern, ALIGN_SIZE};
use crate::mem::MemAddress;
use crate::util::{concat_masks, concat_vecs, from_bytes, to_bytes};

pub const WORD_SIZE: usize = size_of::<usize>();

/// A pattern consisting of a vector of `usize`s.
///
/// # Example
///
/// ```
/// extern crate libkrebs;
/// use libkrebs::pattern_scan::*;
/// use libkrebs::vcpp;
///
/// // Let's make a basic string pattern...
/// let byte_pattern : BytePattern = vcpp::string::str_to_pattern("string test");
///
/// // String patterns are 24 bytes long... a multiple of 8.
/// // Let's make it so that our 64-bit CPU only has to compare 3 words.
/// let word_pattern : WordPattern = WordPattern::from(&byte_pattern as &dyn ScanPattern);
/// assert_eq!(byte_pattern.value(), word_pattern.value());
/// ```
#[derive(Clone)]
pub struct WordPattern {
    value: Vec<usize>,
    mask: Option<Vec<usize>>,
    masked_value: Vec<usize>,

    true_len: usize,

    /// Cached byte representation for `ScanPattern::value()`.
    value_bytes: Vec<u8>,
    /// Cached bool mask for `ScanPattern::mask()`.
    mask_bools: Option<Vec<bool>>,
}

impl WordPattern {
    fn with_true_len(bytes: Vec<u8>, mask: Option<Vec<bool>>, true_len: usize) -> Self {
        if let Some(mask_vec) = &mask {
            assert_eq!(
                mask_vec.len(),
                bytes.len(),
                "Mask and Pattern are not the same size"
            );
        }

        let (bytes, mask) = {
            let len = bytes.len();

            if len % WORD_SIZE != 0 {
                let pad = WORD_SIZE - (len % WORD_SIZE);

                (
                    concat_vecs(bytes, vec![0x00u8; pad]),
                    concat_masks(mask.as_deref(), Some(&vec![false; pad]), len),
                )
            } else {
                (bytes, mask)
            }
        };

        // Cache the byte-level representations for ScanPattern::value() / mask()
        let value_bytes = Vec::from(&bytes[..true_len]);
        let mask_bools: Option<Vec<bool>> = mask.as_ref().map(|m| m[..true_len].to_vec());

        let value = to_word_vec(&bytes);

        let mask_as_bytes: Option<Vec<u8>> = mask.clone().map(|mask_vec| {
            mask_vec
                .into_iter()
                .map(|x| if x { 0xFF } else { 0x00 })
                .collect()
        });

        WordPattern {
            mask: mask_as_bytes.clone().map(|x| to_word_vec(&x)),
            masked_value: match mask_as_bytes {
                Some(mask_bytes) => to_word_vec(
                    &(bytes
                        .into_iter()
                        .zip(mask_bytes.into_iter())
                        .map(|(x, y)| x & y)
                        .collect()),
                ),
                None => value.clone(),
            },
            value,
            true_len,
            value_bytes,
            mask_bools,
        }
    }
}

impl From<&dyn ScanPattern> for WordPattern {
    fn from(pattern: &dyn ScanPattern) -> Self {
        Self::new(pattern.value().to_vec(), pattern.mask().map(|m| m.to_vec()))
    }
}

impl From<BytePattern> for WordPattern {
    fn from(pattern: BytePattern) -> Self {
        Self::new(pattern.value().to_vec(), pattern.mask().map(|m| m.to_vec()))
    }
}

fn to_byte_slice<'a, T: Copy>(slice: &'a [T]) -> &'a [u8] {
    unsafe { slice::from_raw_parts::<u8>(slice.as_ptr() as *const u8, slice.len() * WORD_SIZE) }
}

fn to_word_slice<'a>(slice: &'a [u8]) -> &'a [usize] {
    debug_assert_eq!(
        slice.len() % WORD_SIZE,
        0,
        "Chosen word does not evenly fit"
    );

    unsafe {
        slice::from_raw_parts::<usize>(slice.as_ptr() as *const usize, slice.len() / WORD_SIZE)
    }
}

fn to_word_vec<'a>(vec: &Vec<u8>) -> Vec<usize> {
    let type_size = WORD_SIZE;
    debug_assert_eq!(vec.len() % type_size, 0, "Chosen word does not evenly fit");

    let mut ret = Vec::<usize>::with_capacity(vec.len() / type_size);
    for slice in vec.as_slice().windows(type_size).step_by(type_size) {
        ret.push(from_bytes(slice));
    }

    ret
}

impl FromBytes for WordPattern {
    fn new(bytes: Vec<u8>, mask: Option<Vec<bool>>) -> Self {
        assert_eq!(
            bytes.len() % WORD_SIZE,
            0,
            "Chosen word does not evenly fit"
        );

        let len = bytes.len();

        Self::with_true_len(bytes, mask, len)
    }
}

impl ScanPattern for WordPattern {
    fn len(&self) -> usize {
        self.value.len() * WORD_SIZE
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
        debug_assert_eq!(
            self.len(),
            buf.len(),
            "Slice and pattern have unequal sizes"
        );
        let buf_as_word_slice = to_word_slice(buf);

        if let Some(words_mask) = &self.mask {
            self.masked_value
                .iter()
                .zip(words_mask.iter())
                .zip(buf_as_word_slice.iter())
                .all(|((val, mask), buf)| *val == (*buf & *mask))
        } else {
            self.masked_value
                .iter()
                .zip(buf_as_word_slice.iter())
                .all(|(val, buf)| *val == *buf)
        }
    }

    fn scan_buffer(&self, base_addr: MemAddress, buf: &[u8]) -> Vec<ScanMatch> {
        let pattern_len = self.len();
        assert!(buf.len() >= pattern_len, "Pattern is larger than Buffer");

        let mut matches: Vec<ScanMatch> = Vec::new();
        match &self.mask {
            Some(mask) => {
                for (word_offset, check_slice) in buf
                    .windows(pattern_len)
                    .step_by(ALIGN_SIZE)
                    .map(to_word_slice)
                    .enumerate()
                {
                    if self
                        .masked_value
                        .iter()
                        .zip(check_slice.iter())
                        .zip(mask.iter())
                        .all(|((x, y), m)| *x == (*y & *m))
                    {
                        matches.push(ScanMatch {
                            addr: base_addr + ((word_offset * ALIGN_SIZE) as MemAddress),
                            value: Vec::from(&to_byte_slice(check_slice)[0..self.true_len]),
                        })
                    }
                }
            }
            None => {
                for (word_offset, check_slice) in buf
                    .windows(pattern_len)
                    .step_by(ALIGN_SIZE)
                    .map(to_word_slice)
                    .enumerate()
                {
                    if self
                        .masked_value
                        .iter()
                        .zip(check_slice.iter())
                        .all(|(x, y)| *x == *y)
                    {
                        let the_match = ScanMatch {
                            addr: base_addr + ((word_offset * ALIGN_SIZE) as MemAddress),
                            value: Vec::from(&to_byte_slice(check_slice)[0..self.true_len]),
                        };
                        matches.push(the_match);
                    }
                }
            }
        }

        matches
    }
}

impl Add for WordPattern {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        let new_len = self.value_bytes.len() + other.value_bytes.len();
        let new_value = [&self.value_bytes[..], &other.value_bytes[..]].concat();
        let new_mask = concat_masks(
            self.mask_bools.as_deref(),
            other.mask_bools.as_deref(),
            new_len,
        );

        Self::new(new_value, new_mask)
    }
}

impl Display for WordPattern {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self as &dyn ScanPattern)
    }
}

impl std::fmt::Debug for WordPattern {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let pattern_as_str = match &self.mask {
            Some(mask_vec) => self
                .value
                .iter()
                .zip(mask_vec.iter())
                .map(|(x, m)| {
                    let val_as_bytes = to_bytes(x);
                    let mask_as_bytes = to_bytes(m);

                    val_as_bytes
                        .into_iter()
                        .zip(mask_as_bytes)
                        .map(|(xb, mb)| {
                            if mb != 0 {
                                format!("{:02X}", xb)
                            } else {
                                String::from("??")
                            }
                        })
                        .collect::<Vec<String>>()
                        .join("")
                })
                .collect::<Vec<String>>()
                .join(" "),
            None => self
                .value()
                .iter()
                .map(|x| format!("{:#04X}", *x))
                .collect::<Vec<String>>()
                .join(" "),
        };

        write!(f, "Scan Pattern [{}]", pattern_as_str)
    }
}

impl PartialEq for WordPattern {
    fn eq(&self, other: &Self) -> bool {
        self as &dyn ScanPattern == other as &dyn ScanPattern
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unmasked_query() {
        let query = vec![0x20, 0x30, 0x30, 0x70, 0xFF, 0xFF, 0x7F, 0x7F];
        let pattern = WordPattern::from(BytePattern::new(query, None));
        println!("{}", pattern);
        let ez_match: [u8; 8] = [0x20, 0x30, 0x30, 0x70, 0xFF, 0xFF, 0x7F, 0x7F];
        assert!(pattern.matches(&ez_match[..]));

        let harder_match: [u8; 16] = [
            0x00, 0x20, 0x30,
            0x30, // Although the pattern shows up twice in the linear sequence,
            0x70, 0x10, 0x15, 0x25, // in actual memory all variables are word-aligned,
            0x20, 0x30, 0x30, 0x70, // so it should only match once, at the aligned offset.
            0xFF, 0xFF, 0x7F, 0x7F,
        ];

        let query_result = pattern.scan_buffer(0x00000000, &harder_match);
        assert_eq!(
            query_result,
            vec![ScanMatch {
                addr: 0x08,
                value: Vec::from(&ez_match[..])
            }]
        );
    }
}
