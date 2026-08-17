use libkrebs::error::{MemError, UnixMemError};
use libkrebs::mem::MemAddress;
pub use libkrebs::mem::{AddressRange, Buffer, Reader};
pub use libkrebs::pattern_scan::{BytePattern, ChunkScanResult, FromBytes, ScanMatch, ScanPattern};

// Example Memory Sample
const MEM_TEST: [u8; 0x23] = [
    0x00, 0x00, 0xFF, 0xFF, // Match 0x00
    0xFF, 0x00, 0xFF, 0xFF, // Match 0x04
    0xFF, 0xFF, 0xFF, 0xFF, // 0x08
    0x00, 0xFF, 0xFF, 0x00, // 0x0C
    0x00, 0xFF, 0xFF, 0xFF, // Match 0x10
    0x00, 0xFF, 0xFF, 0xFF, // 0x14
    0x00, 0xFF, 0xFF, 0x00, // 0x18
    0xFF, 0xFF, 0xFF, 0xFF, // 0x1C
    0xFF, 0xFF, 0x00,
];

struct TestReader<'a> {
    buf: &'a [u8],
    _no_literal: (),
}

impl<'a> TestReader<'a> {
    pub fn new(buf: &'a [u8]) -> TestReader<'a> {
        TestReader {
            buf,
            _no_literal: (),
        }
    }
}

impl Reader for TestReader<'_> {
    unsafe fn read_n_bytes(
        &mut self,
        base_addr: MemAddress,
        buffer: &mut [u8],
        size: usize,
    ) -> Result<usize, MemError> {
        if base_addr as usize + size <= self.buf.len() {
            let range = base_addr as usize..base_addr as usize + size;
            println!("Reading from range {:?}", range);
            buffer.copy_from_slice(&self.buf[range]);
            Ok(size)
        } else {
            Err(MemError::Unix(UnixMemError::ProcMemReadFailed {
                addr: base_addr,
                size,
                kind: std::io::ErrorKind::UnexpectedEof,
            }))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::{FromBytes, ScanPattern};
    use std::sync::{Arc, Mutex};

    const MAX_READ_SIZE: usize = 48;

    #[test]
    fn basic_scan_test() {
        let reader = Arc::new(Mutex::new(TestReader::new(&MEM_TEST)));
        for i in (8..MAX_READ_SIZE).step_by(libkrebs::pattern_scan::ALIGN_SIZE) {
            let mut buf = Buffer::new(reader.clone(), i as usize);
            let pattern = BytePattern::ignore(3)
                + BytePattern::new(vec![0xFF], None)
                + BytePattern::ignore(3)
                + BytePattern::new(vec![0xFF], None);
            let examble = vec![0x00, 0x00, 0x00, 0xFF, 0x00, 0x00, 0x00, 0xFF];
            assert!(pattern.matches(examble.as_slice()), "pattern should match test buffer");

            let (result, _scan_time) = buf.full_scan(&pattern, AddressRange::new(0x00, 0x20), false);

            assert_eq!(
                result.matches,
                vec![
                    ScanMatch {
                        addr: 0x00,
                        value: vec![0x00, 0x00, 0xFF, 0xFF, 0xFF, 0x00, 0xFF, 0xFF]
                    },
                    ScanMatch {
                        addr: 0x04,
                        value: vec![0xFF, 0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]
                    },
                    ScanMatch {
                        addr: 0x10,
                        value: vec![0x00, 0xFF, 0xFF, 0xFF, 0x00, 0xFF, 0xFF, 0xFF]
                    }
                ]
            )
        }
    }
}
