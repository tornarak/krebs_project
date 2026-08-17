//! I added `Arc` and `Mutex` to my code to make it thread-safe.
//! It somehow made the code even faster when running in a single thread.
#![feature(test)]

use std::io;
use std::sync::{Arc, Mutex};

extern crate rand;
extern crate test;

use libkrebs::error::MemError;
use rand::RngCore;

use libkrebs::cpp_std::vcpp;
use libkrebs::mem::{AddressRange, Buffer, MemAddress, Reader};
use libkrebs::pattern_scan::*;

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
            //			println!("Reading from range {:?}", range);
            buffer.copy_from_slice(&self.buf[range]);
            Ok(size)
        } else {
            Err(MemError::Unix(
                libkrebs::error::UnixMemError::ProcMemReadFailed {
                    addr: base_addr,
                    size: size,
                    kind: io::ErrorKind::UnexpectedEof,
                },
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use test::Bencher;

    const BUF_SIZE: usize = 50 * 1024 * 1024;
    const MIN_MATCHES: usize = 10;

    fn create_buffer(pattern: &dyn ScanPattern) -> Vec<u8> {
        let mut rng = rand::thread_rng();
        let mut random_values = vec![0x00u8; BUF_SIZE];
        let buf_len = random_values.len();
        rng.try_fill_bytes(&mut random_values[..]).unwrap();
        (0..MIN_MATCHES)
            .map(|_| {
                let rand = rng.next_u64() as usize;
                let padded_rand = rand - rand % 4;
                padded_rand % (buf_len - pattern.len())
            })
            .map(|i| {
                println!("Placing at address {:08X}", i);
                i
            })
            .for_each(|i| {
                (&mut random_values[i..i + pattern.len()])
                    .copy_from_slice(pattern.value())
            });

        random_values
    }

    #[bench]
    fn bench_int_scan(b: &mut Bencher) {
        let pattern = WordPattern::from(BytePattern::new(
            vec![0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08],
            None,
        ));
        let buf = create_buffer(&pattern);

        b.iter(|| {
            let results = pattern.scan_buffer(0x0, &buf[..]);
            assert!(results.len() >= MIN_MATCHES, "{}", results.len());
        });
    }

    #[bench]
    fn bench_str_byte_scan(b: &mut Bencher) {
        let pattern = vcpp::string::str_to_pattern("BIGMANTYRONE");
        let buf = create_buffer(&pattern);

        b.iter(|| {
            let results = pattern.scan_buffer(0x0, &buf[..]);
            assert!(results.len() >= MIN_MATCHES, "{}", results.len());
        });
    }

    #[bench]
    fn bench_str_word_scan(b: &mut Bencher) {
        let pattern = WordPattern::from(vcpp::string::str_to_pattern("BIGMANTYRONE"));
        let buf = create_buffer(&pattern);

        b.iter(|| {
            let results = pattern.scan_buffer(0x0, &buf[..]);
            assert!(results.len() >= MIN_MATCHES, "{}", results.len());
        });
    }

    #[bench]
    fn bench_str_simd_scan(b: &mut Bencher) {
        let str_bytes = vcpp::string::str_to_pattern("BIGMANTYRONE");
        let buf = create_buffer(&str_bytes);
        let pattern = Simd32Pattern::from(str_bytes + BytePattern::ignore(8));

        b.iter(|| {
            let results = pattern.scan_buffer(0x0, &buf[..]);
            assert!(results.len() >= MIN_MATCHES, "{}", results.len());
        });
    }

    #[bench]
    fn bench_big_af_scan(b: &mut Bencher) {
        let mut big_vec = vec![0x00u8; 1000];
        let mut rng = rand::thread_rng();
        rng.try_fill_bytes(&mut big_vec[..]).unwrap();
        let pattern = WordPattern::from(BytePattern::new(big_vec, None));
        let buf = create_buffer(&pattern);

        b.iter(|| {
            let results = pattern.scan_buffer(0x0, &buf[..]);
            dbg!(&results);
            assert!(results.len() >= MIN_MATCHES, "{}", results.len());
        });
    }

    #[bench]
    fn bench_buf_simd_scan(b: &mut Bencher) {
        let str_bytes = vcpp::string::str_to_pattern("BIGMANTYRONE");
        let buf = create_buffer(&str_bytes);
        let pattern = Simd32Pattern::from(str_bytes + BytePattern::ignore(8));
        let buf_reader = TestReader::new(buf.as_slice());
        let mut mem_buf = Buffer::new(Arc::new(Mutex::new(buf_reader)), 1024 * 1024);

        b.iter(|| {
            let (results, _scan_time) =
                mem_buf.full_scan(&pattern, AddressRange::new(0x00, buf.len() as MemAddress), false);
            println!("{} results", results.matches.len());
            assert!(
                results.matches.len() >= MIN_MATCHES,
                "{}",
                results.matches.len()
            );
        });
    }

    #[bench]
    fn bench_simd_simple_scan(b: &mut Bencher) {
        let pattern = Simd32Pattern::new(vec![0xFF; 32], None);
        let buf = create_buffer(&pattern);

        b.iter(|| {
            let results = pattern.scan_buffer(0x0, &buf[..]);
            assert!(results.len() >= MIN_MATCHES, "{}", results.len());
        });
    }
}
