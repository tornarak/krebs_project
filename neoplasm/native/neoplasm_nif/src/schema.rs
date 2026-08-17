use rustler::{Atom, Decoder, Encoder, Env, NifResult, Term};

// ---------------------------------------------------------------------------
// Newtype decoders for Elixir terms with no direct Rust equivalent
// ---------------------------------------------------------------------------

/// Decodes any Elixir atom to its string name.
#[derive(Clone)]
pub struct AtomStr(pub String);

impl<'a> Decoder<'a> for AtomStr {
    fn decode(term: Term<'a>) -> NifResult<Self> {
        Ok(AtomStr(term.atom_to_string()?))
    }
}

impl Encoder for AtomStr {
    fn encode<'a>(&self, env: Env<'a>) -> Term<'a> {
        self.0.encode(env)
    }
}

/// Decodes a Rekto `data_type` term:
///   `:u32`        → `"u32"`
///   `{:u8, 16}`   → `"[u8; 16]"`
///   module atom   → `"MySchema"` (strips `"Elixir."`)
#[derive(Clone)]
pub struct DataType(pub String);

impl<'a> Decoder<'a> for DataType {
    fn decode(term: Term<'a>) -> NifResult<Self> {
        if term.is_atom() {
            let s = term.atom_to_string()?;
            Ok(DataType(s.trim_start_matches("Elixir.").to_string()))
        } else {
            let (type_term, count) = <(Term<'a>, usize)>::decode(term)?;
            Ok(DataType(format!("[{}; {}]", type_term.atom_to_string()?, count)))
        }
    }
}

impl Encoder for DataType {
    fn encode<'a>(&self, env: Env<'a>) -> Term<'a> {
        self.0.encode(env)
    }
}

/// Decodes `nil` → `None`, module atom → `Some("ModuleName")`.
#[derive(Clone)]
pub struct OptionalModule(pub Option<String>);

impl<'a> Decoder<'a> for OptionalModule {
    fn decode(term: Term<'a>) -> NifResult<Self> {
        let s = term.atom_to_string()?;
        Ok(OptionalModule(if s == "nil" {
            None
        } else {
            Some(s.trim_start_matches("Elixir.").to_string())
        }))
    }
}

impl Encoder for OptionalModule {
    fn encode<'a>(&self, env: Env<'a>) -> Term<'a> {
        self.0.encode(env)
    }
}

// ---------------------------------------------------------------------------
// Schema structs with manual Decoder impls (avoids NifStruct __struct__ check)
// ---------------------------------------------------------------------------

/// Field decoded from `%Rekto.Schema.FieldInfo{}` (opts skipped).
#[derive(Clone)]
pub struct FieldInfo {
    pub name: AtomStr,
    pub data_type: DataType,
    pub offset: usize,
    pub size: usize,
    pub points_to: OptionalModule,
}

impl<'a> Decoder<'a> for FieldInfo {
    fn decode(term: Term<'a>) -> NifResult<Self> {
        let env = term.get_env();
        Ok(FieldInfo {
            name: AtomStr::decode(field(term, env, crate::atoms::name())?)?,
            data_type: DataType::decode(field(term, env, crate::atoms::data_type())?)?,
            offset: usize::decode(field(term, env, crate::atoms::offset())?)?,
            size: usize::decode(field(term, env, crate::atoms::size())?)?,
            points_to: OptionalModule::decode(field(term, env, crate::atoms::points_to())?)?,
        })
    }
}

/// Decoded from `%Rekto.Schema.Info{}` (size + fields only).
#[derive(Clone)]
pub struct SchemaInfoDecoded {
    pub size: usize,
    pub fields: Vec<FieldInfo>,
}

impl<'a> Decoder<'a> for SchemaInfoDecoded {
    fn decode(term: Term<'a>) -> NifResult<Self> {
        let env = term.get_env();
        Ok(SchemaInfoDecoded {
            size: usize::decode(field(term, env, crate::atoms::size())?)?,
            fields: Vec::<FieldInfo>::decode(field(term, env, crate::atoms::fields())?)?,
        })
    }
}

/// Stored schema — `SchemaInfoDecoded` annotated with the module name and atom.
#[derive(Clone)]
pub struct SchemaInfo {
    /// Display name with `Elixir.` stripped.
    pub name: String,
    /// The actual Elixir module atom — send this back to Elixir unchanged.
    pub module_atom: Atom,
    pub size: usize,
    pub fields: Vec<FieldInfo>,
}

impl SchemaInfo {
    pub fn new(module_atom: Atom, name: String, decoded: SchemaInfoDecoded) -> Self {
        Self { name, module_atom, size: decoded.size, fields: decoded.fields }
    }
}

// ---------------------------------------------------------------------------
// Helper
// ---------------------------------------------------------------------------

fn field<'a>(map: Term<'a>, env: Env<'a>, key: Atom) -> NifResult<Term<'a>> {
    map.map_get(key.encode(env))
}
