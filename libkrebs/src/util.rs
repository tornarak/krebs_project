//! Basic utility functions relevant to libkrebs.
//!
//! Why aren't these in the standard library
//! (and if they are, then why am I too stupid to find them)?

/// Concatenates two vectors.
pub fn concat_vecs<T: Copy>(v0: Vec<T>, v1: Vec<T>) -> Vec<T> {
    v0.into_iter().chain(v1.into_iter()).collect()
}

/// Concatenates two byte masks (borrowed).
pub fn concat_masks(
    m0: Option<&[bool]>,
    m1: Option<&[bool]>,
    new_len: usize,
) -> Option<Vec<bool>> {
    match (m0, m1) {
        (None, None) => None,
        (Some(mv0), None) => {
            let leen = new_len - mv0.len();
            Some([mv0, &vec![true; leen]].concat())
        }
        (None, Some(mv1)) => Some([&vec![true; new_len - mv1.len()][..], mv1].concat()),
        (Some(mv0), Some(mv1)) => Some([mv0, mv1].concat()),
    }
}

/* C-Style Type Punning */

/// Converts a slice of type `&[u8]` to a value of the given type.
/// Keep endian-ness in mind!
///
/// # Panics
///
/// Panics if the given type and the given slice have different sizes.
pub fn from_bytes<T: Copy>(bytes: &[u8]) -> T {
    assert_eq!(
        std::mem::size_of::<T>(),
        bytes.len(),
        "Attempt to cast to a type larger than buffer"
    );
    unsafe { *(bytes.as_ptr() as *const T) }
}

/// Converts a value of the given type to `Vec<u8>`.
/// Keep endian-ness in mind!
pub fn to_bytes<T: Copy>(val: T) -> Vec<u8> {
    let size: usize = std::mem::size_of::<T>();
    let mut buf: Vec<u8> = Vec::with_capacity(size);
    unsafe {
        *(buf.as_mut_ptr() as *mut T) = val;
        buf.set_len(size);
    }

    buf
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typecast_works() {
        let bytes: [u8; 4] = [0x01, 0x01, 0x01, 0x00];
        let u32_from_bytes: u32 = from_bytes(&bytes);

        // YMMV, depending on system endian-ness
        assert_eq!(u32_from_bytes, 0x00010101);

        let bytes_from_u32: Vec<u8> = to_bytes(0x00010101);
        assert_eq!(bytes_from_u32.len(), 4);
        assert!(bytes.iter().zip(bytes_from_u32.iter()).all(|(a, b)| a == b))
    }

    #[test]
    #[should_panic]
    fn reject_different_sizes() {
        let bytes = [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF];

        from_bytes::<u32>(&bytes[..]);
    }
}
