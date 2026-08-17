use std::sync::Arc;

use rustler::{Binary, ResourceArc};

use libkrebs::mem::MemAddress;
use libkrebs::pattern_scan::*;

pub const WORD_SIZE: usize = std::mem::size_of::<usize>();

#[derive(NifUnitEnum)]
pub enum PatternType {
    Byte,
    Word,
    Simd,
}

pub struct PatternResource {
    pub pattern: Arc<dyn ScanPattern>,
}

#[derive(NifStruct)]
#[module = "Krebs.ScanPattern"]
pub struct PatternStruct {
    pub pattern_ref: ResourceArc<PatternResource>,
    pub pattern_type: PatternType,

    pub bytes: Vec<u8>,
    pub mask: Option<Vec<u8>>,
}

impl PatternStruct {
    pub fn pattern<'a>(&'a self) -> &'a Arc<dyn ScanPattern> {
        &self.pattern_ref.pattern
    }
}

impl From<BytePattern> for PatternStruct {
    fn from(pattern: BytePattern) -> Self {
        let bytes = pattern.value().to_vec();
        let mask = pattern.mask().map(|bools| {
            bools
                .iter()
                .map(|b| if *b { 0xFFu8 } else { 0x00u8 })
                .collect::<Vec<u8>>()
        });

        let (pattern_type, pattern) = optimize_pattern(pattern);

        PatternStruct {
            pattern_ref: ResourceArc::new(PatternResource { pattern }),
            pattern_type,
            bytes,
            mask,
        }
    }
}

#[derive(NifStruct)]
#[module = "Krebs.ScanMatch"]
pub struct MatchStruct {
    pub addr: MemAddress,
    pub value: String,
}

impl From<ScanMatch> for MatchStruct {
    fn from(s: ScanMatch) -> Self {
        let ScanMatch { addr, value } = s;
        Self {
            addr,
            // SAFETY: This is not a real UTF-8 string — it's an Erlang binary.
            // Rustler encodes String as an Erlang binary (byte list), not a
            // charlist, so invalid UTF-8 is harmless: the bytes pass through
            // to the BEAM untouched. The value is never interpreted as text
            // on the Rust side.
            value: unsafe { String::from_utf8_unchecked(value) },
        }
    }
}

impl MatchStruct {
    //	pub fn with_length(s: ScanMatch, len: usize) -> Self {
    //		let ScanMatch { addr, value } = s;
    //		let first_n_bytes = value.into_iter()
    //			.take(len)
    //			.collect();
    //
    //		Self { addr, value: unsafe { String::from_utf8_unchecked(first_n_bytes) } }
    //	}
}

fn optimize_pattern(byte_pattern: BytePattern) -> (PatternType, Arc<dyn ScanPattern>) {
    if byte_pattern.len() <= 32 {
        (
            PatternType::Simd,
            Arc::new(Simd32Pattern::from(byte_pattern)),
        )
    } else if byte_pattern.len() % WORD_SIZE == 0 {
        (PatternType::Word, Arc::new(WordPattern::from(byte_pattern)))
    } else {
        (PatternType::Byte, Arc::new(byte_pattern))
    }
}

#[nif]
pub fn new_scan_pattern(bytes_bin: Binary, mask_bin: Option<Binary>) -> PatternStruct {
    let bytes = Vec::from(&bytes_bin[..]);
    let mask_bools = mask_bin.map(|bin| Vec::from(&bin[..]).into_iter().map(|x| x != 0).collect());

    let byte_pattern = BytePattern::new(bytes.clone(), mask_bools.clone());

    PatternStruct::from(byte_pattern)
}

#[nif]
pub fn concat_scan_patterns(p0: PatternStruct, p1: PatternStruct) -> PatternStruct {
    PatternStruct::from(
        BytePattern::from(p0.pattern().as_ref()) + BytePattern::from(p1.pattern().as_ref()),
    )
}
