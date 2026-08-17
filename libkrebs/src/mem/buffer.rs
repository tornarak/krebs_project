//! A special buffer for memory scans.
//! Essentially a state machine bundled with a [`Boros`].

use std::cmp::min;
use std::fmt::{Debug, Display};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use log::warn;

use crate::pattern_scan;
use crate::util::concat_vecs;
use crate::Boros;
use pattern_scan::ALIGN_SIZE;
use pattern_scan::{BytePattern, ChunkScanResult, FromBytes, ScanMatch, ScanPattern};

use super::reader::{AddressRange, MemAddress, Reader};

#[derive(Clone, Default)]
pub struct ScanTime {
    pub range: Vec<AddressRange>,

    pub init_time: Duration,
    pub read_time: Duration,
    pub lock_time: Duration,
    pub scan_time: Duration,
    pub state_time: Duration,
}

impl ScanTime {
    pub fn scan_size(&self) -> usize {
        self.range
            .iter()
            .fold(0usize, |total, range| total + range.size() as usize)
    }

    pub fn total(&self) -> Duration {
        self.init_time + self.read_time + self.lock_time + self.scan_time + self.state_time
    }
}

impl std::ops::Add for ScanTime {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Self {
            range: concat_vecs(self.range, other.range),

            init_time: self.init_time + other.init_time,
            read_time: self.read_time + other.read_time,
            lock_time: self.lock_time + other.lock_time,
            scan_time: self.scan_time + other.scan_time,
            state_time: self.state_time + other.state_time,
        }
    }
}

impl std::ops::AddAssign for ScanTime {
    fn add_assign(&mut self, other: Self) {
        self.range.extend(other.range);
        self.init_time += other.init_time;
        self.read_time += other.read_time;
        self.lock_time += other.lock_time;
        self.scan_time += other.scan_time;
        self.state_time += other.state_time;
    }
}

pub fn thousands_separator<T: ToString>(i: T) -> String {
    // "12345678" -> "12,345,678"
    i.to_string()
        // '1', '2', '3', '4', '5', '6', '7', '8'
        .bytes()
        // '8', '7', '6', '5', '4', '3', '2', '1'
        .rev()
        // ['8', '7', '6', '5', '4', '3', '2', '1']
        .collect::<Vec<u8>>()
        // ['8', '7', '6'], ['5', '4', '3'], ['2', '1']
        .chunks(3)
        // ['2', '1'], ['5', '4', '3'], ['8', '7', '6']
        .rev()
        // ["12"], ["345"], ["678"]
        .map(|x: &[u8]| -> String {
            String::from_utf8(x.iter().map(|x| *x).rev().collect()).unwrap()
        })
        // [["12"], ["345"], ["678"]]
        .collect::<Vec<String>>()
        // "12,345,678"
        .join(",")
}

impl Display for ScanTime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Scan of {} ranges ({} bytes) in {} ns\nInitialization Time: {} ns\nTotal Read Time: {} ns\n\tTotal Time Waiting on Mutex: {} ns\nTotal Scan Time: {} ns\nTotal State Calculation Time: {} ns",
               self.range.len(),
               thousands_separator(self.scan_size()),
               thousands_separator(self.total().as_nanos()),
               thousands_separator(self.init_time.as_nanos()),
               thousands_separator(self.read_time.as_nanos()),
               thousands_separator(self.lock_time.as_nanos()),
               thousands_separator(self.scan_time.as_nanos()),
               thousands_separator(self.state_time.as_nanos())
        )
    }
}

impl Debug for ScanTime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_string())
    }
}

/// A buffer that can read a continuous block of another process's memory.
/// Memory is read and scanned from the host process `read_size` bytes at a time.
pub struct Buffer<T: Reader> {
    // Actual buffer; New buffer allocated for each pattern
    buf: Boros,
    // Process that memory is being read from
    reader: Arc<Mutex<T>>,
    read_size: usize,
}

impl<T: Reader> Buffer<T> {
    /// Creates a new Buffer representing the given address range in the given process.
    ///
    /// `read_size` should ideally be in the megabytes.
    pub fn new(reader: Arc<Mutex<T>>, read_size: usize) -> Buffer<T> {
        assert!(
            read_size % ALIGN_SIZE == 0,
            "Read size must be a multiple of word size"
        );

        Buffer {
            buf: Boros::new(2, 2),
            reader,
            read_size,
        }
    }

    fn load_chunk(&mut self, state: &ScanState) -> (Option<usize>, Duration) {
        let ScanState { buf_range, .. } = state;
        let expected_bytes_read = buf_range.size() as usize;
        let mutex_start = SystemTime::now();
        let mut reader_ref = self.reader.lock().unwrap();
        let mutex_duration = SystemTime::now().duration_since(mutex_start).unwrap();

        let bytes_res = reader_ref.read_range(
            *buf_range,
            &mut (self.buf.tail())[0..buf_range.size() as usize],
        );

        if bytes_res.is_err() {
            warn!(
                "Skipped range {} (error {})",
                buf_range,
                bytes_res.unwrap_err()
            );
            (None, mutex_duration)
        } else {
            let bytes_read = bytes_res.unwrap();
            assert_eq!(
                expected_bytes_read, bytes_read,
                "Incorrect number of bytes read ({} when {} expected)",
                bytes_read, expected_bytes_read
            );
            (Some(bytes_read), mutex_duration)
        }
    }

    fn scan_chunk(&self, state: &ScanState, pattern: &dyn ScanPattern) -> Vec<ScanMatch> {
        let (base_addr, buf) = match state.status {
            Begin => (state.buf_range.start_addr, self.buf.tail_immut()),
            Middle(skipped_last) => {
                if skipped_last {
                    (state.buf_range.start_addr, self.buf.tail_immut())
                } else {
                    (
                        state.buf_range.start_addr - self.buf.head_size as MemAddress,
                        self.buf.full(),
                    )
                }
            }
            Last(skipped_last) => {
                if skipped_last {
                    (
                        state.buf_range.start_addr - self.buf.head_size as MemAddress,
                        &(self.buf.tail_immut())[0..state.buf_range.size() as usize],
                    )
                } else {
                    (
                        state.buf_range.start_addr - self.buf.head_size as MemAddress,
                        &(self.buf.full())[0..self.buf.head_size + state.buf_range.size() as usize],
                    )
                }
            }
            End => panic!("State Machine Broke"),
        };

        pattern.scan_buffer(base_addr, buf)
    }

    fn read_chunk(
        &mut self,
        state: &ScanState,
        pattern: &dyn ScanPattern,
    ) -> (bool, Vec<ScanMatch>, Duration, Duration, Duration) {
        let load_start = SystemTime::now();
        let (loaded, mutex_duration) = self.load_chunk(&state);
        let load_duration = SystemTime::now().duration_since(load_start).unwrap();

        match loaded {
            Some(_load_size) => {
                let scan_start = SystemTime::now();
                let chunk_scan = self.scan_chunk(&state, pattern);
                let scan_duration = SystemTime::now().duration_since(scan_start).unwrap();
                (
                    false,
                    chunk_scan,
                    load_duration,
                    mutex_duration,
                    scan_duration,
                )
            }
            None => (
                true,
                vec![],
                load_duration,
                mutex_duration,
                Duration::new(0, 0),
            ),
        }
    }

    /// Scans the entire range for the given pattern, returning all results.
    ///
    /// If `lazy` is set to true, returns as soon as a chunk is found with matching values.
    ///
    /// # Errors
    ///
    /// Errors in the reading process are ignored internally.
    pub fn full_scan(
        &mut self,
        pattern: &dyn ScanPattern,
        scan_range: AddressRange,
        lazy: bool,
    ) -> (ChunkScanResult, ScanTime) {
        assert!(
            scan_range.start_addr % ALIGN_SIZE as MemAddress == 0,
            "Scan range must be word-aligned"
        );
        let init_start = SystemTime::now();
        let (padded_pattern_size, _) = get_aligned_pattern_size(pattern);
        let head_size = padded_pattern_size - ALIGN_SIZE;
        if self.buf.size() != head_size + self.read_size {
            //          println!("Allocating New Buffer");
            self.buf = Boros::new(head_size, self.read_size);
        }

        let mut results: Vec<ScanMatch> = vec![];
        let mut state = ScanState::new(scan_range, self.read_size);
        let init_time = SystemTime::now().duration_since(init_start).unwrap();

        let mut load_time = Duration::new(0, 0);
        let mut lock_time = Duration::new(0, 0);
        let mut scan_time = Duration::new(0, 0);
        let mut state_time = Duration::new(0, 0);

        loop {
            let (skipped, chunk_results, load_duration, mutex_duration, scan_duration) =
                self.read_chunk(&state, pattern);
            results.extend(chunk_results);

            let state_start = SystemTime::now();
            state = ScanState::next_chunk(state, skipped);

            match state.status {
                Begin => self.buf.swallow_tail(),
                Middle(skipped_last) | Last(skipped_last) => {
                    if !skipped_last {
                        self.buf.swallow_tail()
                    }
                }
                End => break,
            }
            let state_duration = SystemTime::now().duration_since(state_start).unwrap();

            load_time += load_duration;
            lock_time += mutex_duration;
            scan_time += scan_duration;
            state_time += state_duration;

            if lazy && results.len() > 0 {
                break;
            }
        }

        (
            ChunkScanResult {
                pattern: BytePattern::new(pattern.value().to_vec(), pattern.mask().map(|m| m.to_vec())),
                matches: results,
                range: scan_range,
            },
            ScanTime {
                range: vec![scan_range],
                init_time,
                read_time: load_time,
                lock_time,
                scan_time,
                state_time,
            },
        )
    }

    pub fn find_one(
        &mut self,
        pattern: &dyn ScanPattern,
        scan_range: AddressRange,
    ) -> (Option<ScanMatch>, ScanTime) {
        let (chunk_scan_result, scan_time) = self.full_scan(pattern, scan_range, true);

        match chunk_scan_result.matches.get(0) {
            Some(match_ref) => (Some(match_ref.clone()), scan_time),
            None => (None, scan_time),
        }
    }
}

#[derive(Debug)]
enum ScanStatus {
    Begin,
    Middle(bool), // Whether the last was skipped_last
    Last(bool),   // Ditto
    End,
}

use ScanStatus::*;

/// State machine data for memory scans.
#[derive(Debug)]
struct ScanState {
    buf_range: AddressRange,
    read_size: usize,
    scan_range: AddressRange,
    status: ScanStatus,
}

impl ScanState {
    fn new(scan_range: AddressRange, read_size: usize) -> ScanState {
        ScanState {
            buf_range: AddressRange::new(
                scan_range.start_addr,
                min(
                    scan_range.end_addr,
                    scan_range.start_addr + read_size as MemAddress,
                ),
            ),
            read_size,
            scan_range,
            status: Begin,
        }
    }

    fn next_chunk(state: ScanState, skipped_last: bool) -> ScanState {
        match state.status {
            Begin | Middle(_) => {
                let ScanState {
                    buf_range,
                    read_size,
                    scan_range,
                    ..
                } = state;
                let new_start = buf_range.end_addr;
                let (new_end, new_status) = {
                    let bytes_till_end = scan_range.end_addr - buf_range.end_addr;

                    if bytes_till_end == 0 {
                        // the end
                        (scan_range.end_addr, End)
                    } else if bytes_till_end <= read_size as MemAddress {
                        // only one read left
                        (scan_range.end_addr, Last(skipped_last))
                    } else {
                        (new_start + read_size as MemAddress, Middle(skipped_last))
                    }
                };

                ScanState {
                    buf_range: AddressRange::new(new_start, new_end),
                    status: new_status,
                    ..state
                }
            }
            Last(_) => ScanState {
                status: End,
                ..state
            },
            End => state,
        }
    }
}

// Gets the size of the pattern when padded to align with memory
// Returns (padded_size, padding)
fn get_aligned_pattern_size(pattern: &dyn ScanPattern) -> (usize, usize) {
    let len = pattern.len();
    let pad = (ALIGN_SIZE - (len % ALIGN_SIZE)) % ALIGN_SIZE;

    (len + pad, pad)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn computes_aligns() {
        let p = BytePattern::null(3);
        assert_eq!(get_aligned_pattern_size(&p), (4, 1));

        let p1 = BytePattern::null(4);
        assert_eq!(get_aligned_pattern_size(&p1), (4, 0));

        let p2 = BytePattern::null(18);
        assert_eq!(get_aligned_pattern_size(&p2), (20, 2));
    }

    #[test]
    fn separator() {
        assert_eq!(thousands_separator(1), "1");
        assert_eq!(thousands_separator(100), "100");
        assert_eq!(thousands_separator(1_000), "1,000");
        assert_eq!(thousands_separator(10_000), "10,000");
        assert_eq!(thousands_separator(100_000), "100,000");
        assert_eq!(thousands_separator(1_000_000), "1,000,000");
        assert_eq!(thousands_separator(10_000_000), "10,000,000");
        assert_eq!(thousands_separator(100_000_000), "100,000,000");
        assert_eq!(thousands_separator(1_000_000_000), "1,000,000,000");
    }
}
