//! Traits for structs that can validate their own state.
//!
//! `DeepVerifiable`, `ForeignPtr`, and `WeakForeign` are stubbed out below;
//! they were used by older code and are preserved for reference.

use crate::util::from_bytes;

/// A trait for structs and data types that have valid and invalid states.
///
/// This is implemented for all primitive types by default.
pub trait Verifiable: Copy {
    /// The error type produced by validation methods.
    /// Defaults to [`Infallible`](std::convert::Infallible) for types with no invalid states.
    type Error: std::error::Error = std::convert::Infallible;

    /// All the verification that can be done on the struct data itself.
    ///
    /// If a struct's state is entirely internal and it contains
    /// no pointers, this is all you need.
    fn verify_shallow(&self) -> Result<(), Self::Error> {
        Ok(())
    }

    /// Creates a new instance of the struct from a byte slice.
    /// Internally calls [`verify_shallow`].
    ///
    /// If you need to override this, you're using the trait wrong.
    fn from_bytes(bytes: &[u8]) -> Result<Self, Self::Error> {
        assert_eq!(
            bytes.len(),
            std::mem::size_of::<Self>(),
            "Byte slice and target struct are different sizes"
        );
        let inst = from_bytes::<Self>(bytes);
        inst.verify_shallow()?;

        Ok(inst)
    }
}

/// A trait for structs and data types that contain pointers or
/// other data that can be further verified by accessing the host
/// process's memory.
///
/// For instance, if you're scanning for instances of a C++ class
/// with virtual methods, you can implement this trait to check
/// whether your type contains a valid vftable.
///
/// This is implemented for all primitive types by default.
// pub trait DeepVerifiable<T: Reader>: Verifiable {
//     /// All the verification that can be done when you have access
//     /// to the host process's memory.
//     ///
//     /// For instance, [`CppString`]'s implementation of this function
//     /// checks if the string is too long for the internal buffer; if so,
//     /// it checks whether the contained pointer points to a valid C string.
//     ///
//     /// If you have no use for this function in your implementation, just return `Ok(())`.
//     fn verify_deep(&self, _reader: &T, _addr: MemAddress) -> Result<(), String> {
//         Ok(())
//     }

//     /// Creates a new instance of the struct from the process's memory at the given address.
//     /// Internally calls [`verify_shallow`], then [`verify_deep`].
//     ///
//     /// If you need to override this, you're using the trait wrong.
//     fn from_addr(reader: &T, addr: MemAddress) -> Result<Self, String> {
//         let mut byte_buffer = vec![0x00; std::mem::size_of::<Self>()];
//         try_or_string! { reader.read_to_buffer(addr, byte_buffer.as_mut_slice()) };

//         let inst = Self::from_bytes(&byte_buffer)?;
//         inst.verify_deep(reader, addr)?;
//         Ok(inst)
//     }
// }

impl Verifiable for u8 {}
impl Verifiable for u32 {}
impl Verifiable for u64 {}
impl Verifiable for u128 {}

impl Verifiable for i8 {}
impl Verifiable for i32 {}
impl Verifiable for i64 {}
impl Verifiable for i128 {}

impl Verifiable for f32 {}
impl Verifiable for f64 {}

impl Verifiable for isize {}
impl Verifiable for usize {}

impl Verifiable for char {}
impl Verifiable for bool {}

// impl<T: Reader> DeepVerifiable<T> for u8 {}
// impl<T: Reader> DeepVerifiable<T> for u32 {}
// impl<T: Reader> DeepVerifiable<T> for u64 {}
// impl<T: Reader> DeepVerifiable<T> for u128 {}
// impl<T: Reader> DeepVerifiable<T> for i8 {}
// impl<T: Reader> DeepVerifiable<T> for i32 {}
// impl<T: Reader> DeepVerifiable<T> for i64 {}
// impl<T: Reader> DeepVerifiable<T> for i128 {}
// impl<T: Reader> DeepVerifiable<T> for f32 {}
// impl<T: Reader> DeepVerifiable<T> for f64 {}
// impl<T: Reader> DeepVerifiable<T> for isize {}
// impl<T: Reader> DeepVerifiable<T> for usize {}
// impl<T: Reader> DeepVerifiable<T> for char {}
// impl<T: Reader> DeepVerifiable<T> for bool {}

// /// Trait for "foreign pointers": structures that
// /// point to data in another process.
// ///
// /// Because you have no control over the process's memory use,
// /// this will always be a weak pointer.
// pub trait ForeignPtr<T, U: Reader> {
//     fn get_value(&self) -> Result<T, String>;

//     fn is_valid(&self) -> bool {
//         self.get_value().is_ok()
//     }
// }

// /// Default implementation of [`ForeignPtr`].
// ///
// /// Due to Rust's coherency rules, this cannot be extended outside
// /// of this library. You may want to create your own [`ForeignPtr`]
// /// if you wish to do such a thing.
// pub struct WeakForeign<T: DeepVerifiable<U>, U: Reader> {
//     /// The address pointed to.
//     pub addr: MemAddress,
//     /// The Reader that can read the address.
//     pub reader: Arc<U>,
//     nuffin: std::marker::PhantomData<*const T>,
// }

// impl<'a, T: DeepVerifiable<U>, U: Reader> WeakForeign<T, U> {
//     /// Creates a new `WeakForeign`, pointing to the given address in
//     /// the given process.
//     pub fn new(reader: Arc<U>, addr: MemAddress) -> Self {
//         WeakForeign::<T, U> {
//             addr,
//             reader,
//             nuffin: std::marker::PhantomData,
//         }
//     }
// }

// impl<'a, T: DeepVerifiable<U>, U: Reader> ForeignPtr<T, U> for WeakForeign<T, U> {
//     fn get_value(&self) -> Result<T, String> {
//         T::from_addr(&*self.reader, self.addr)
//     }
// }
