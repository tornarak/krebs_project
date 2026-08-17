//! Rustler `Encoder` implementations for all libkrebs / yeet_engine error types.
//!
//! Uses an `Enc<E>` newtype wrapper to implement a foreign trait on foreign types.
//!
//! Wire format:
//!   unit variant          → atom
//!   struct/tuple variant  → {atom, [{field, value}, ...]}   (keyword list)
//!   newtype wrapper       → {atom, Enc(inner)}

use rustler::{Encoder, Env, Term};

use libkrebs::error::{
    gcc, vcpp, CommonStringError, KrebsError, MemError, ScannerError, StdError, UnixMemError,
    WinMemError,
};

use crate::atoms;

/// Newtype wrapper — lets us implement `Encoder` for foreign error types.
pub struct Enc<E>(pub E);

// ── helpers ──────────────────────────────────────────────────────────────────

/// Build a keyword list `[{atom, term}, ...]` from pairs.
/// Each `$key` must be an expression that returns a `Term` (e.g. an atom).
/// Each `$val` must already be a `Term`.
macro_rules! kw {
    ($env:expr, [ $( $key:expr => $val:expr ),+ $(,)? ]) => {{
        let pairs: Vec<(Term, Term)> = vec![ $( ($key, $val) ),+ ];
        pairs.encode($env)
    }};
}

// ── KrebsError ───────────────────────────────────────────────────────────────

impl Encoder for Enc<KrebsError> {
    fn encode<'a>(&self, env: Env<'a>) -> Term<'a> {
        match &self.0 {
            KrebsError::Mem(e) => (atoms::mem(), Enc(e.clone())).encode(env),
            KrebsError::CppStd(e) => (atoms::cpp_std(), Enc(e.clone())).encode(env),
        }
    }
}

// ── MemError ──────────────────────────────────────────────────────────────────

impl Encoder for Enc<MemError> {
    fn encode<'a>(&self, env: Env<'a>) -> Term<'a> {
        match &self.0 {
            MemError::Windows(e) => (atoms::windows(), Enc(e.clone())).encode(env),
            MemError::Unix(e)    => (atoms::unix(),    Enc(e.clone())).encode(env),
        }
    }
}

// ── WinMemError ───────────────────────────────────────────────────────────────

impl Encoder for Enc<WinMemError> {
    fn encode<'a>(&self, env: Env<'a>) -> Term<'a> {
        match &self.0 {
            WinMemError::ReadFailed { addr, size, code } => (
                atoms::read_failed(),
                kw!(env, [
                    atoms::addr().encode(env) => addr.encode(env),
                    atoms::size().encode(env) => size.encode(env),
                    atoms::code().encode(env) => code.encode(env)
                ]),
            ).encode(env),

            WinMemError::WriteFailed { addr, size, code } => (
                atoms::write_failed(),
                kw!(env, [
                    atoms::addr().encode(env) => addr.encode(env),
                    atoms::size().encode(env) => size.encode(env),
                    atoms::code().encode(env) => code.encode(env)
                ]),
            ).encode(env),

            WinMemError::ProcessAccessDenied { pid, access_mask } => (
                atoms::process_access_denied(),
                kw!(env, [
                    atoms::pid().encode(env)         => pid.encode(env),
                    atoms::access_mask().encode(env) => access_mask.encode(env)
                ]),
            ).encode(env),

            WinMemError::ProcessCloseFailed { pid, code } => (
                atoms::process_close_failed(),
                kw!(env, [
                    atoms::pid().encode(env)  => pid.encode(env),
                    atoms::code().encode(env) => code.encode(env)
                ]),
            ).encode(env),

            WinMemError::ProcessAlreadyClosed { pid } => (
                atoms::process_already_closed(),
                kw!(env, [ atoms::pid().encode(env) => pid.encode(env) ]),
            ).encode(env),

            WinMemError::AccessDenied => atoms::access_denied().encode(env),

            WinMemError::MemoryAccessDenied { addr } => (
                atoms::memory_access_denied(),
                kw!(env, [ atoms::addr().encode(env) => addr.encode(env) ]),
            ).encode(env),

            WinMemError::InvalidHandle => atoms::invalid_handle().encode(env),

            WinMemError::PartialCopy { copied, expected } => (
                atoms::partial_copy(),
                kw!(env, [
                    atoms::copied().encode(env)   => copied.encode(env),
                    atoms::expected().encode(env) => expected.encode(env)
                ]),
            ).encode(env),

            WinMemError::NullPointer => atoms::null_pointer().encode(env),

            WinMemError::VirtualQueryFailed { addr, code } => (
                atoms::virtual_query_failed(),
                kw!(env, [ atoms::addr().encode(env) => addr.encode(env), atoms::code().encode(env) => code.encode(env) ]),
            ).encode(env),

            WinMemError::ModuleEnumerationFailed { code } => (
                atoms::module_enumeration_failed(),
                kw!(env, [ atoms::code().encode(env) => code.encode(env) ]),
            ).encode(env),

            WinMemError::Misc { code } => (
                atoms::misc(),
                kw!(env, [ atoms::code().encode(env) => code.encode(env) ]),
            ).encode(env),
        }
    }
}

// ── UnixMemError ──────────────────────────────────────────────────────────────

impl Encoder for Enc<UnixMemError> {
    fn encode<'a>(&self, env: Env<'a>) -> Term<'a> {
        match &self.0 {
            UnixMemError::ProcMemReadFailed { addr, size, kind } => (
                atoms::proc_mem_read_failed(),
                kw!(env, [
                    atoms::addr().encode(env) => addr.encode(env),
                    atoms::size().encode(env) => size.encode(env),
                    atoms::kind().encode(env) => format!("{kind:?}").encode(env)
                ]),
            ).encode(env),

            UnixMemError::ProcMemWriteFailed { addr, size, kind } => (
                atoms::proc_mem_write_failed(),
                kw!(env, [
                    atoms::addr().encode(env) => addr.encode(env),
                    atoms::size().encode(env) => size.encode(env),
                    atoms::kind().encode(env) => format!("{kind:?}").encode(env)
                ]),
            ).encode(env),

            UnixMemError::ProcMapsOpenFailed { pid, kind } => (
                atoms::proc_maps_open_failed(),
                kw!(env, [
                    atoms::pid().encode(env)  => pid.encode(env),
                    atoms::kind().encode(env) => format!("{kind:?}").encode(env)
                ]),
            ).encode(env),

            UnixMemError::ProcMapsRowParseError { field_count, row } => (
                atoms::proc_maps_row_parse_error(),
                kw!(env, [
                    atoms::field_count().encode(env) => field_count.encode(env),
                    atoms::row().encode(env)         => row.as_str().encode(env)
                ]),
            ).encode(env),

            UnixMemError::ProcMapsAddrRangeParseFailed { input } => (
                atoms::proc_maps_addr_range_parse_failed(),
                kw!(env, [ atoms::input().encode(env) => input.as_str().encode(env) ]),
            ).encode(env),

            UnixMemError::ProcMapsPermsParseFailed { input } => (
                atoms::proc_maps_perms_parse_failed(),
                kw!(env, [ atoms::input().encode(env) => input.as_str().encode(env) ]),
            ).encode(env),

            UnixMemError::ProcessOpenFailed { pid, kind } => (
                atoms::process_open_failed(),
                kw!(env, [
                    atoms::pid().encode(env)  => pid.encode(env),
                    atoms::kind().encode(env) => format!("{kind:?}").encode(env)
                ]),
            ).encode(env),
        }
    }
}

// ── StdError ──────────────────────────────────────────────────────────────────

impl Encoder for Enc<StdError> {
    fn encode<'a>(&self, env: Env<'a>) -> Term<'a> {
        match &self.0 {
            StdError::Vcpp(e)         => (atoms::vcpp(),          Enc(e.clone())).encode(env),
            StdError::Gcc(e)          => (atoms::gcc(),           Enc(e.clone())).encode(env),
            StdError::CommonString(e) => (atoms::common_string(), Enc(e.clone())).encode(env),
        }
    }
}

// ── vcpp ──────────────────────────────────────────────────────────────────────

impl Encoder for Enc<vcpp::Error> {
    fn encode<'a>(&self, env: Env<'a>) -> Term<'a> {
        match &self.0 {
            vcpp::Error::String(e) => (atoms::string(), Enc(e.clone())).encode(env),
            vcpp::Error::Vector(e) => (atoms::vector(), Enc(e.clone())).encode(env),
        }
    }
}

impl Encoder for Enc<vcpp::StringError> {
    fn encode<'a>(&self, env: Env<'a>) -> Term<'a> {
        match &self.0 {
            vcpp::StringError::Short(e) => (atoms::short(), Enc(e.clone())).encode(env),
            vcpp::StringError::Long(e)  => (atoms::long(),  Enc(e.clone())).encode(env),
        }
    }
}

impl Encoder for Enc<vcpp::ShortStringError> {
    fn encode<'a>(&self, env: Env<'a>) -> Term<'a> {
        match &self.0 {
            vcpp::ShortStringError::ZeroLength => atoms::zero_length().encode(env),
            vcpp::ShortStringError::BadAllocSize { got } => (
                atoms::bad_alloc_size(),
                kw!(env, [ atoms::got().encode(env) => got.encode(env) ]),
            ).encode(env),
        }
    }
}

impl Encoder for Enc<vcpp::LongStringError> {
    fn encode<'a>(&self, env: Env<'a>) -> Term<'a> {
        match &self.0 {
            vcpp::LongStringError::TooShort { length } => (
                atoms::too_short(),
                kw!(env, [ atoms::length().encode(env) => length.encode(env) ]),
            ).encode(env),
            vcpp::LongStringError::AllocTooSmall { alloc_size } => (
                atoms::alloc_too_small(),
                kw!(env, [ atoms::alloc_size().encode(env) => alloc_size.encode(env) ]),
            ).encode(env),
            vcpp::LongStringError::InvalidAllocSize { alloc_size } => (
                atoms::invalid_alloc_size(),
                kw!(env, [ atoms::alloc_size().encode(env) => alloc_size.encode(env) ]),
            ).encode(env),
            vcpp::LongStringError::NullPtr => atoms::null_ptr().encode(env),
        }
    }
}

impl Encoder for Enc<vcpp::VectorError> {
    fn encode<'a>(&self, env: Env<'a>) -> Term<'a> {
        match &self.0 {
            vcpp::VectorError::InvalidRange { first_addr, last_addr } => (
                atoms::invalid_range(),
                kw!(env, [
                    atoms::first_addr().encode(env) => first_addr.encode(env),
                    atoms::last_addr().encode(env)  => last_addr.encode(env)
                ]),
            ).encode(env),
            vcpp::VectorError::Misaligned { diff, element_size } => (
                atoms::misaligned(),
                kw!(env, [
                    atoms::diff().encode(env)         => diff.encode(env),
                    atoms::element_size().encode(env) => element_size.encode(env)
                ]),
            ).encode(env),
            vcpp::VectorError::Io(e) => (atoms::io(), Enc(e.clone())).encode(env),
        }
    }
}

// ── gcc ───────────────────────────────────────────────────────────────────────

impl Encoder for Enc<gcc::Error> {
    fn encode<'a>(&self, env: Env<'a>) -> Term<'a> {
        match &self.0 {
            gcc::Error::String(e) => (atoms::string(), Enc(e.clone())).encode(env),
        }
    }
}

impl Encoder for Enc<gcc::StringError> {
    fn encode<'a>(&self, env: Env<'a>) -> Term<'a> {
        match &self.0 {
            gcc::StringError::Short(e) => (atoms::short(), Enc(e.clone())).encode(env),
            gcc::StringError::Long(e)  => (atoms::long(),  Enc(e.clone())).encode(env),
        }
    }
}

impl Encoder for Enc<gcc::ShortStringError> {
    fn encode<'a>(&self, env: Env<'a>) -> Term<'a> {
        match &self.0 {
            gcc::ShortStringError::InvalidLength { length } => (
                atoms::invalid_length(),
                kw!(env, [ atoms::length().encode(env) => length.encode(env) ]),
            ).encode(env),
        }
    }
}

impl Encoder for Enc<gcc::LongStringError> {
    fn encode<'a>(&self, env: Env<'a>) -> Term<'a> {
        match &self.0 {
            gcc::LongStringError::NullPtr => atoms::null_ptr().encode(env),
            gcc::LongStringError::CapacityTooSmall { capacity, length } => (
                atoms::capacity_too_small(),
                kw!(env, [
                    atoms::capacity().encode(env) => capacity.encode(env),
                    atoms::length().encode(env)   => length.encode(env)
                ]),
            ).encode(env),
        }
    }
}

// ── CommonStringError ─────────────────────────────────────────────────────────

impl Encoder for Enc<CommonStringError> {
    fn encode<'a>(&self, env: Env<'a>) -> Term<'a> {
        match &self.0 {
            CommonStringError::BufferLengthMismatch { buf_len, str_len } => (
                atoms::buffer_length_mismatch(),
                kw!(env, [
                    atoms::buf_len().encode(env) => buf_len.encode(env),
                    atoms::str_len().encode(env) => str_len.encode(env)
                ]),
            ).encode(env),
            CommonStringError::NoNullTerminator { str_len } => (
                atoms::no_null_terminator(),
                kw!(env, [ atoms::str_len().encode(env) => str_len.encode(env) ]),
            ).encode(env),
            CommonStringError::EmbeddedNullBytes => atoms::embedded_null_bytes().encode(env),
            CommonStringError::Io(e) => (atoms::io(), Enc(e.clone())).encode(env),
            CommonStringError::InvalidUtf8 { source } => (
                atoms::invalid_utf8(),
                kw!(env, [ atoms::source().encode(env) => source.to_string().encode(env) ]),
            ).encode(env),
        }
    }
}

// ── ScannerError ──────────────────────────────────────────────────────────────

impl Encoder for Enc<ScannerError> {
    fn encode<'a>(&self, env: Env<'a>) -> Term<'a> {
        match &self.0 {
            ScannerError::NotInHeap { addr, desc } => (
                atoms::not_in_heap(),
                kw!(env, [
                    atoms::addr().encode(env) => addr.encode(env),
                    atoms::name().encode(env) => desc.as_str().encode(env)
                ]),
            ).encode(env),
            ScannerError::NotInModule { addr, desc } => (
                atoms::not_in_module(),
                kw!(env, [
                    atoms::addr().encode(env) => addr.encode(env),
                    atoms::name().encode(env) => desc.as_str().encode(env)
                ]),
            ).encode(env),
        }
    }
}
