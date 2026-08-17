//!	Faculties for scanning bytes in a buffer. Includes the [`ScanPattern`] trait and
//! three structs that implement it: [`BytePattern`], [`WordPattern`], and [`Simd32Pattern`]
//! (in increasing order of speed).
//!
//! Also includes the [`ScanMatch`] and [`ChunkScanResult`] data types for storing scan data.
//!
//! # Building compound patterns
//!
//! [`FromBytes`]'s convenience constructors, combined with the `+` operator, can be used to
//! easily construct complex scan patterns out of smaller pieces.
//!
//! ## Example
//!
//! The FBI has rooted your computer and is throttling your access to your... "cat pictures".
//! Thankfully, you have a Rust IDE and libkrebs, and, through some clever reverse engineering,
//! have found the layout of the struct that contains the FBI virus's secret keys.
//!
//! You now have to find the instance of said struct in memory.
//!
//! ```
//! use libkrebs::pattern_scan::*;
//! use libkrebs::util::to_bytes;
//!
//! #[repr(C)]
//! #[derive(Copy, Clone)]
//! pub struct CoolStruct {
//! 	_junk: 				u32,
//! 	// Always 3
//! 	three: 				u32,
//! 	// Always [0x00, 0x00, 0x00, 0x00, 0x00, 0x00]
//! 	pub six_nulls: 		[u8; 6],
//! 	// Padding
//! 	_pad:				[u8; 2],
//! 	// Contains FBI secrets
//! 	pub important_int: 	u32
//! }
//!
//! impl Scannable<BytePattern> for CoolStruct {
//! 	fn to_pattern(&self) -> BytePattern {
//! 		BytePattern::ignore(4)
//! 		+ BytePattern::from_primitive(3 as u32, None)
//! 		+ BytePattern::null(6)
//! 		+ BytePattern::ignore(2)
//! 		+ BytePattern::from_primitive(self.important_int, None)
//! 	}
//! }
//!
//! fn main() {
//! 	let ebin_struct : CoolStruct = CoolStruct {
//! 		_junk: 0x0F,
//! 		three: 3,
//! 		six_nulls: [0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
//! 		_pad: [0x32, 0x57],
//! 		important_int: 5
//! 	};
//!
//! 	let ebin_pattern = ebin_struct.to_pattern();
//! 	let ebin_bytes = to_bytes(ebin_struct);
//!
//! 	// It matches the original struct's bytes
//! 	assert!(ebin_pattern.matches(ebin_bytes.as_slice()));
//!
//! 	// Look at all the typing we saved with those constructors!
//! 	assert_eq!(
//! 		ebin_pattern,
//! 		BytePattern::new(
//! 			vec![
//! 				0x00, 0x00, 0x00, 0x00,
//! 				0x03, 0x00, 0x00, 0x00,
//! 				0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
//! 				0x05, 0x00, 0x00, 0x00
//! 			],
//! 			Some(vec![
//! 				false, false, false, false,
//! 				true, true, true, true,
//! 				true, true, true, true, true, true,
//! 				false, false,
//! 				true, true, true, true
//! 			])
//! 		)
//! 	);
//!
//! 	// Let's go into maximum overdrive by using SIMD!
//!     // The constructor can take a `BytePattern` or a reference to a `dyn ScanPattern`.
//! 	let ebin_simd_pattern = Simd32Pattern::from(&ebin_pattern as &dyn ScanPattern);
//! 	assert_eq!(ebin_pattern.value(), ebin_simd_pattern.value());
//! }
//! ```

mod byte_pattern;
mod simd32_pattern;
mod word_pattern;

pub use byte_pattern::BytePattern;
pub use simd32_pattern::Simd32Pattern;
pub use word_pattern::WordPattern;

/* IMPORTS */

// std imports
use std::fmt;

// types
use crate::mem::AddressRange;
use crate::mem::MemAddress;

/* Constants  */

/// The system word size.
pub const ALIGN_SIZE: usize = 4;

/// Can be converted to a ScanPattern.
///
/// For types that implement `Copy`, you can use [`FromBytes::from_primitive`].
pub trait Scannable<T: ScanPattern> {
    fn to_pattern(&self) -> T;
}

/// A pattern of bytes that can be scanned for in a buffer.
///
/// Consists of two parallel vectors:
/// A vector of the bytes that make up the pattern, and an optional byte mask.
/// The two vectors are guaranteed to be the same size.
///
/// Scan Patterns (and the member structs) can be concatenated
/// with the `+` operator, returning a pattern with the type of the first operand.
///
/// If either pattern has a mask, the result of the concatenation
/// will have a mask as well.
pub trait ScanPattern: Send + Sync {
    /// Returns the length of the pattern.
    fn len(&self) -> usize;

    /// Returns the true length of the underlying data.
    /// This is useful for patterns with padding.
    ///
    /// Defaults to [`len`].
    fn true_len(&self) -> usize {
        self.len()
    }

    /// Returns the value of the bytes that are being scanned for.
    ///
    /// The pattern does not necessarily represent the bytes this way internally.
    fn value(&self) -> &[u8];

    /// Returns a mask with a length equal to that of `value`.
    /// A `false` will lead to the parallel byte of
    /// `value` being ignored.
    fn mask(&self) -> Option<&[bool]>;

    /// Checks the pattern against the given slice.
    ///
    /// The `==` operator is the same as this function.
    ///
    /// # Panics
    ///
    /// Panics if the pattern and slice are not the same length.
    fn matches(&self, buf: &[u8]) -> bool;

    /// Scans the entire buffer for the given pattern.
    ///
    /// Since this function is supposed to scan actual memory, it only
    /// checks every [`ALIGN_SIZE`] bytes to account for variable alignment. This means
    /// that the provided buffer has to be word-aligned for this function to work properly.
    ///
    /// # Panics
    ///
    /// Panics if the pattern is larger than the buffer.
    fn scan_buffer(&self, base_addr: MemAddress, buf: &[u8]) -> Vec<ScanMatch> {
        let pattern_len = self.len();
        assert!(buf.len() >= pattern_len, "Pattern is larger than Buffer");

        let mut matches: Vec<ScanMatch> = Vec::new();
        for (word_offset, check_slice) in buf.windows(pattern_len).step_by(ALIGN_SIZE).enumerate() {
            if self.matches(check_slice) {
                matches.push(ScanMatch {
                    addr: base_addr + ((word_offset * ALIGN_SIZE) as MemAddress),
                    value: Vec::from(check_slice),
                })
            }
        }

        matches
    }
}

pub trait FromBytes: ScanPattern {
    /// Creates a new pattern from the byte vector and mask.
    ///
    /// # Panics
    ///
    /// Panics if the bytes and mask are not the same length.
    fn new(bytes: Vec<u8>, mask: Option<Vec<bool>>) -> Self;

    /// Creates a new pattern from a byte slice and mask.
    fn from_bytes(bytes: &[u8], mask: Option<Vec<bool>>) -> Self
    where
        Self: Sized,
    {
        Self::new(Vec::from(bytes), mask)
    }

    /// Creates a new pattern from a string and mask.
    fn from_str(string: &str, mask: Option<Vec<bool>>) -> Self
    where
        Self: Sized,
    {
        Self::new(string.bytes().collect(), mask)
    }

    /// Creates a new pattern from the raw bytes of any `Copy` value.
    fn from_primitive<T: Copy>(val: T, mask: Option<Vec<bool>>) -> Self
    where
        Self: Sized,
    {
        Self::new(crate::util::to_bytes(val), mask)
    }

    /// A pattern of `n` null bytes with no mask.
    fn null(n: usize) -> Self
    where
        Self: Sized,
    {
        Self::new(vec![0x00; n], None)
    }

    /// A pattern of `n` fully-wildcarded (ignored) bytes.
    fn ignore(n: usize) -> Self
    where
        Self: Sized,
    {
        Self::new(vec![0x00; n], Some(vec![false; n]))
    }

    /// Creates a new pattern that is a clone of the given pattern,
    /// but with the given mask.
    fn with_mask<T: ScanPattern + ?Sized>(pattern: &T, mask: Option<Vec<bool>>) -> Self
    where
        Self: Sized,
    {
        Self::new(pattern.value().to_vec(), mask)
    }
}

/// Builds a byte mask from a string; any char other than `'?'` is kept (`true`).
pub fn str_mask(s: &str) -> Vec<bool> {
    s.bytes().map(|c| c != b'?').collect()
}

/// Builds a byte mask from a byte slice; any non-zero byte is kept (`true`).
pub fn bytes_mask(bytes: &[u8]) -> Vec<bool> {
    bytes.iter().map(|b| *b != 0x00).collect()
}

impl fmt::Display for dyn ScanPattern {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let pattern_as_str = if let Some(mask_vec) = self.mask() {
            self.value()
                .iter()
                .zip(mask_vec.iter())
                .map(|(x, m)| {
                    if *m {
                        format!("{:04X}", *x)
                    } else {
                        String::from("????")
                    }
                })
                .collect::<Vec<String>>()
                .join(" ")
        } else {
            self.value()
                .iter()
                .map(|x| format!("{:#04X}", *x))
                .collect::<Vec<String>>()
                .join(" ")
        };

        write!(f, "Scan Pattern [{}]", pattern_as_str)
    }
}

impl fmt::Debug for dyn ScanPattern {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Scan Pattern [{}]", self.to_string())
    }
}

impl PartialEq<Self> for dyn ScanPattern {
    fn eq(&self, other: &Self) -> bool {
        self.len() == other.len() && self.value() == other.value() && self.mask() == other.mask()
    }
}

impl PartialEq<[u8]> for dyn ScanPattern {
    fn eq(&self, other: &[u8]) -> bool {
        self.matches(other)
    }
}

impl PartialEq<&[u8]> for dyn ScanPattern + Send + Sync {
    fn eq(&self, other: &&[u8]) -> bool {
        self.matches(*other)
    }
}

/* Scan Data */

/// An address in memory at which a pattern was matched,
/// as well as the bytes that triggered the match.
#[derive(Clone)]
pub struct ScanMatch {
    /// The match address.
    pub addr: MemAddress,
    /// The bytes matched.
    pub value: Vec<u8>,
}

impl PartialEq for ScanMatch {
    fn eq(&self, other: &Self) -> bool {
        self.addr == other.addr && self.value == other.value
    }
}

impl fmt::Display for ScanMatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Matched Value [{}] at {:#08X}",
            self.value
                .iter()
                .map(|byte| format!("{:#04X}", byte))
                .collect::<Vec<String>>()
                .join(", "),
            self.addr
        )
    }
}

impl fmt::Debug for ScanMatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Matched Value [{}] at {:#08X}",
            self.value
                .iter()
                .map(|byte| format!("{:#04X}", byte))
                .collect::<Vec<String>>()
                .join(", "),
            self.addr
        )
    }
}

/// The result of scanning a chunk of memory.
#[derive(Clone)]
pub struct ChunkScanResult {
    /// The pattern that was scanned for.
    pub pattern: BytePattern,
    /// The scan range.
    pub range: AddressRange,
    /// A vector of matches.
    pub matches: Vec<ScanMatch>,
}

impl ChunkScanResult {
    /// Returns `true` if the result contains any matches.
    pub fn has_matches(&self) -> bool {
        self.matches.len() > 0
    }
}

impl fmt::Display for ChunkScanResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} matches for pattern {} in range {}",
            self.matches.len(),
            self.pattern,
            self.range
        )
    }
}

impl fmt::Debug for ChunkScanResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} matches for pattern {} in range {}",
            self.matches.len(),
            self.pattern,
            self.range
        )
    }
}
