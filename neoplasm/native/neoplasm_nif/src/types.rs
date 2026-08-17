// ---------------------------------------------------------------------------
// Shared data model types — Rust-side representation of Rekto/Elixir types,
// scan state, and memory layout primitives.
// ---------------------------------------------------------------------------

// ── Value types ─────────────────────────────────────────────────────────────

#[derive(Default, PartialEq, Clone, Copy, Debug)]
pub enum ValueType {
    #[default]
    F32,
    F64,
    U32,
    I32,
    U64,
    I64,
    U8,
    I8,
    U16,
    I16,
    Bool,
    Byte,
    Word,
    Pointer,
    SavedAddress,
    Bytes,
    Text,
    Array,
    StringBuffer,
}

impl ValueType {
    pub fn label(self) -> &'static str {
        match self {
            Self::F32 => "f32",
            Self::F64 => "f64",
            Self::U32 => "u32",
            Self::I32 => "i32",
            Self::U64 => "u64",
            Self::I64 => "i64",
            Self::U8 => "u8",
            Self::I8 => "i8",
            Self::U16 => "u16",
            Self::I16 => "i16",
            Self::Bool => "bool",
            Self::Byte => "byte",
            Self::Word => "word",
            Self::Pointer => "pointer (hex)",
            Self::SavedAddress => "saved address",
            Self::Bytes => "bytes (hex)",
            Self::Text => "text (utf-8)",
            Self::Array => "array",
            Self::StringBuffer => "string_buffer",
        }
    }

    pub fn all() -> &'static [ValueType] {
        &[
            Self::Bool,
            Self::Byte,
            Self::I8,
            Self::U8,
            Self::U16,
            Self::I16,
            Self::U32,
            Self::I32,
            Self::U64,
            Self::I64,
            Self::F32,
            Self::F64,
            Self::Word,
            Self::Pointer,
            Self::SavedAddress,
            Self::Bytes,
            Self::Text,
            Self::Array,
            Self::StringBuffer,
        ]
    }

    /// Scalar primitive types — valid as array element types.
    pub fn primitive_types() -> &'static [ValueType] {
        &[
            Self::Bool,
            Self::Byte,
            Self::I8,
            Self::U8,
            Self::U16,
            Self::I16,
            Self::U32,
            Self::I32,
            Self::U64,
            Self::I64,
            Self::F32,
            Self::F64,
            Self::Word,
        ]
    }

    /// Types selectable in the hex dump interpretation panel for watching.
    /// Includes scalars + compound types (array, text, string_buffer).
    pub fn watchable_types() -> &'static [ValueType] {
        &[
            Self::Bool,
            Self::Byte,
            Self::I8,
            Self::U8,
            Self::U16,
            Self::I16,
            Self::U32,
            Self::I32,
            Self::U64,
            Self::I64,
            Self::F32,
            Self::F64,
            Self::Word,
            Self::Pointer,
            Self::Text,
            Self::Array,
            Self::StringBuffer,
        ]
    }

    /// Encode this type as the corresponding Elixir atom term.
    /// Uses pre-declared atoms from `crate::atoms` for correctness and efficiency.
    pub fn type_atom<'a>(self, env: rustler::Env<'a>) -> rustler::Term<'a> {
        use rustler::Encoder;
        match self {
            Self::Bool => crate::atoms::bool().encode(env),
            Self::Byte => crate::atoms::byte().encode(env),
            Self::I8 => crate::atoms::i8().encode(env),
            Self::U8 => crate::atoms::u8().encode(env),
            Self::U16 => crate::atoms::u16().encode(env),
            Self::I16 => crate::atoms::i16().encode(env),
            Self::U32 => crate::atoms::u32().encode(env),
            Self::I32 => crate::atoms::i32().encode(env),
            Self::U64 => crate::atoms::u64().encode(env),
            Self::I64 => crate::atoms::i64().encode(env),
            Self::F32 => crate::atoms::f32().encode(env),
            Self::F64 => crate::atoms::f64().encode(env),
            Self::Word | Self::Pointer | Self::SavedAddress => crate::atoms::word().encode(env),
            // Non-primitive types shouldn't be used as TypeRef::Primitive —
            // fall back to encoding the type_name string so callers notice the mistake.
            other => other.type_name().encode(env),
        }
    }

    /// Elixir type name sent in PrimitiveScanRequested / PrimitivePreviewRequested.
    /// "bytes" → Krebs.MCP.parse_hex_pattern; "text" → UTF-8 bytes;
    /// "array" is formatted as "array/{elem}/{count}" by the caller.
    pub fn type_name(self) -> &'static str {
        match self {
            Self::F32 => "f32",
            Self::F64 => "f64",
            Self::U32 => "u32",
            Self::I32 => "i32",
            Self::U64 => "u64",
            Self::I64 => "i64",
            Self::U8 => "u8",
            Self::I8 => "i8",
            Self::U16 => "u16",
            Self::I16 => "i16",
            Self::Bool => "bool",
            Self::Byte => "byte",
            Self::Word => "word",
            // Both routed to Elixir as :word; Rust side handles the conversion.
            Self::Pointer | Self::SavedAddress => "word",
            Self::Bytes => "bytes",
            Self::Text => "text",
            Self::Array => "array",
            Self::StringBuffer => "string_buffer",
        }
    }
}

/// Serialize a primitive value string to a space-separated uppercase hex string.
/// Returns `""` on parse failure or unsupported type.
pub fn value_to_hex(value_str: &str, data_type: &str) -> String {
    let bytes: Vec<u8> = match data_type {
        "bool" => match value_str {
            "true" | "1" => vec![1u8],
            "false" | "0" => vec![0u8],
            _ => return String::new(),
        },
        "u8" | "byte" => match value_str.parse::<u8>() {
            Ok(v) => vec![v],
            _ => return String::new(),
        },
        "i8" => match value_str.parse::<i8>() {
            Ok(v) => v.to_le_bytes().to_vec(),
            _ => return String::new(),
        },
        "u16" => match value_str.parse::<u16>() {
            Ok(v) => v.to_le_bytes().to_vec(),
            _ => return String::new(),
        },
        "i16" => match value_str.parse::<i16>() {
            Ok(v) => v.to_le_bytes().to_vec(),
            _ => return String::new(),
        },
        "u32" | "word" => match value_str.parse::<u32>() {
            Ok(v) => v.to_le_bytes().to_vec(),
            _ => return String::new(),
        },
        "i32" => match value_str.parse::<i32>() {
            Ok(v) => v.to_le_bytes().to_vec(),
            _ => return String::new(),
        },
        "u64" => match value_str.parse::<u64>() {
            Ok(v) => v.to_le_bytes().to_vec(),
            _ => return String::new(),
        },
        "i64" => match value_str.parse::<i64>() {
            Ok(v) => v.to_le_bytes().to_vec(),
            _ => return String::new(),
        },
        "f32" => match value_str.parse::<f32>() {
            Ok(v) => v.to_le_bytes().to_vec(),
            _ => return String::new(),
        },
        "f64" => match value_str.parse::<f64>() {
            Ok(v) => v.to_le_bytes().to_vec(),
            _ => return String::new(),
        },
        dt if dt.starts_with("[string_buffer;") => {
            // "[string_buffer; N]" — string value padded to N bytes
            if let Some(n) = dt
                .trim_start_matches("[string_buffer; ")
                .trim_end_matches(']')
                .parse::<usize>()
                .ok()
            {
                let mut buf = value_str.as_bytes().to_vec();
                buf.resize(n, 0);
                buf
            } else {
                return String::new();
            }
        }
        _ => return String::new(),
    };
    bytes
        .iter()
        .map(|b| format!("{:02X}", b))
        .collect::<Vec<_>>()
        .join(" ")
}

/// A type reference that encodes to the correct Elixir term for `Rekto.Serialization.datatype?/1`.
/// Primitives → atom, schemas → module atom, arrays → `{atom, count}`, string buffers → `{:string_buffer, n}`.
#[derive(Debug, Clone, Copy)]
pub enum TypeRef {
    Primitive(ValueType),
    Schema(rustler::Atom),
    /// `{:string_buffer, n}` — fixed-size null-padded C string.
    StringBuffer(usize),
    /// `{elem_type_atom, count}` — typed array of primitives.
    Array(ValueType, usize),
}

impl TypeRef {
    pub fn encode<'a>(&self, env: rustler::Env<'a>) -> rustler::Term<'a> {
        use rustler::Encoder;
        use rustler::types::tuple::make_tuple;
        match self {
            TypeRef::Primitive(vt) => vt.type_atom(env),
            TypeRef::Schema(atom) => atom.encode(env),
            TypeRef::StringBuffer(n) => {
                make_tuple(env, &[
                    crate::atoms::string_buffer().encode(env),
                    n.encode(env),
                ])
            }
            TypeRef::Array(elem, count) => {
                make_tuple(env, &[
                    elem.type_atom(env),
                    count.encode(env),
                ])
            }
        }
    }
}

// ── Scan state ───────────────────────────────────────────────────────────────

#[derive(Default, PartialEq, Clone, Copy, Debug)]
pub enum ScanRegion {
    #[default]
    Heap,
    Module,
}

#[derive(Default, PartialEq, Clone, Copy, Debug)]
pub enum ScanStatus {
    #[default]
    Idle,
    Scanning,
    Done(usize),
}

// ── Memory layout ────────────────────────────────────────────────────────────

#[derive(Clone, Default)]
pub struct HexRow {
    pub addr: u64,
    pub bytes: Vec<u8>,
    pub annotations: Vec<String>,
}

#[derive(Clone, Copy, PartialEq)]
pub enum MemKind {
    Heap,
    Module,
    Other,
}

#[derive(Clone)]
pub struct MemRegion {
    pub base: u64,
    pub size: u64,
    pub kind: MemKind,
}
