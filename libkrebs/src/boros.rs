//! A specialized container for continuous streams of bytes.

use std::fmt::{Debug, Display};

use super::util::concat_vecs;

/// A byte buffer that can be destructured into multiple parts.
///
/// This class uses the [Haskell terminology](https://hackage.haskell.org/package/base-4.12.0.0/docs/Data-List.html)
/// for list destructuring. The first `head_size` bytes make up the `head`,
/// the last `tail_size` bytes make up the `tail`; the first `tail_size` bytes
/// make up the `init`; and the last `head_size` bytes make up the `last`.
///
/// This class was designed to allow large sections of memory to be scanned contiguously.
/// It does this by writing to the `tail` and then "swallowing its tail" in between writes
/// like the mythical [Ouroboros](https://en.wikipedia.org/wiki/Ouroboros) - moving the contents of `last` to the `head`.
#[derive(Clone)]
pub struct Boros {
    buf: Vec<u8>,
    /// The size of the head.
    pub head_size: usize,
    /// The size of the tail.
    pub tail_size: usize,
}

impl Boros {
    /// Creates a new Boros with the given head and tail size.
    ///
    /// # Panics
    ///
    /// Panics if the the head size is greater than the tail size.
    pub fn new(head_size: usize, tail_size: usize) -> Boros {
        assert!(head_size <= tail_size, "Head too big to eat tail");

        Boros {
            buf: vec![0; head_size + tail_size],
            head_size,
            tail_size,
        }
    }

    /// Gets the total size of the internal buffer.
    pub fn size(&self) -> usize {
        self.head_size + self.tail_size
    }

    /// Creates a new Boros with an empty head of the given size and the
    /// given vector as the tail.
    ///
    /// # Panics
    ///
    /// Panics if the the head size is greater than the tail size.
    pub fn from(head_size: usize, tail: Vec<u8>) -> Boros {
        assert!(head_size <= tail.len(), "Head too big to eat tail");
        let empty_head = vec![0; head_size];

        Boros {
            head_size,
            tail_size: tail.len(),

            buf: concat_vecs(empty_head, tail),
        }
    }

    /// Returns the entire buffer.
    pub fn full(&self) -> &[u8] {
        self.buf.as_slice()
    }

    /// Returns the head.
    pub fn head(&mut self) -> &mut [u8] {
        self.buf.split_at_mut(self.head_size).0
    }

    /// Returns the head as an immutable slice.
    pub fn head_immut(&self) -> &[u8] {
        self.buf.split_at(self.head_size).0
    }

    /// Returns the init.
    pub fn init(&self) -> &[u8] {
        self.buf.split_at(self.size() - self.head_size).0
    }

    /// Returns the tail.
    pub fn tail(&mut self) -> &mut [u8] {
        self.buf.split_at_mut(self.head_size).1
    }

    /// Returns the tail as an immutable slice.
    pub fn tail_immut(&self) -> &[u8] {
        self.buf.split_at(self.head_size).1
    }

    /// Returns the last.
    pub fn last(&self) -> &[u8] {
        self.buf.split_at(self.size() - self.head_size).1
    }

    /// Clones the internal buffer.
    pub fn to_vec(&self) -> Vec<u8> {
        self.buf.clone()
    }

    /// Moves the contents of the tail to the head.
    /// This is an idempotent operation.
    pub fn swallow_tail(&mut self) {
        let (head, tail) = self.buf.split_at_mut(self.head_size);
        let (_mid, last) = tail.split_at(tail.len() - self.head_size);
        debug_assert_eq!(
            head.len(),
            last.len(),
            "head and last must be the same size"
        );
        head.copy_from_slice(last)
    }
}

impl Display for Boros {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{:?} | {:?}]", self.head_immut(), self.tail_immut())
    }
}

impl Debug for Boros {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{:?} | {:?}]", self.head_immut(), self.tail_immut())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creation_methods() {
        let b0 = Boros::new(2, 4);
        assert_eq!(b0.to_vec(), vec![0, 0, 0, 0, 0, 0]);

        let b1 = Boros::from(2, vec![1, 2, 3, 4]);
        assert_eq!(b1.to_vec(), vec![0, 0, 1, 2, 3, 4]);
    }

    #[test]
    fn components() {
        let mut b0 = Boros::from(2, vec![1, 2, 3, 4]);
        assert_eq!(vec![0, 0, 1, 2, 3, 4], b0.to_vec());

        assert_eq!(vec![0, 0], b0.head());
        assert_eq!(vec![1, 2, 3, 4], b0.tail());

        assert_eq!(vec![0, 0, 1, 2], b0.init());
        assert_eq!(vec![3, 4], b0.last());
    }
}
