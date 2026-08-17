# Rekto Schema Reference

A practical reference for writing Rekto schemas via the Krebs MCP server.

---

## Table of Contents

1. [What is a Schema?](#what-is-a-schema)
2. [Basic Structure](#basic-structure)
3. [Field Types](#field-types)
4. [Field Options](#field-options)
5. [Pointer Fields](#pointer-fields)
6. [Special Field Shortcuts](#special-field-shortcuts)
7. [Constraints](#constraints)
8. [Sanity Checks](#sanity-checks)
9. [Transformations](#transformations)
10. [Schema Inheritance](#schema-inheritance)
11. [Metadata and Address Context](#metadata-and-address-context)
12. [MCP Workflow](#mcp-workflow)
13. [Common C++ Patterns](#common-c-patterns)
14. [Error Reference](#error-reference)

---

## What is a Schema?

A Rekto schema is an Elixir module that maps a fixed-size byte buffer to a struct.
It is roughly analogous to a C `struct` definition. When you call `MySchema.from_binary(bytes)`,
Rekto reads each field from the buffer in offset order, runs constraints and sanity checks,
and returns `{:ok, %MySchema{...}}` or `{:error, reason}`.

Schemas are used with `Krebs.Scanner` to deserialize structs found in another process's memory:

```
scan memory → get address → read bytes → MySchema.from_binary(bytes, %{addr: address})
```

---

## Basic Structure

```elixir
defmodule MyApp.MyStruct do
  use Rekto.Schema

  schema do
    field :health,    :u32
    field :max_health, :u32
    field :position_x, :f32
    field :position_y, :f32
    field :position_z, :f32
  end
end
```

`use Rekto.Schema` injects:
- The `schema/1`, `field/3`, `points_to/3`, `sanity/1`, `transform/1` macros
- Implementations of `from_binary/1,2`, `to_binary/1`, `__size__/0`, `__schema__/0`, `garbage_mask/0`
- An Elixir struct with all declared fields plus `__meta__`
- Auto-registration in `Rekto.SchemaRegistry`

**Fields must be declared in strictly increasing offset order.** A compile-time error is raised
if they overlap or are declared out of order.

---

## Field Types

### Primitive types

| Type    | Size   | Description                        |
|---------|--------|------------------------------------|
| `:bool` | 1 byte | `0` → `false`, non-zero → `true`   |
| `:byte` | 1 byte | Unsigned byte (alias for `:u8`)    |
| `:u8`   | 1 byte | Unsigned 8-bit integer             |
| `:i8`   | 1 byte | Signed 8-bit integer               |
| `:u16`  | 2 bytes | Unsigned 16-bit integer           |
| `:i16`  | 2 bytes | Signed 16-bit integer             |
| `:u32`  | 4 bytes | Unsigned 32-bit integer           |
| `:i32`  | 4 bytes | Signed 32-bit integer             |
| `:f32`  | 4 bytes | 32-bit IEEE 754 float             |
| `:u64`  | 8 bytes | Unsigned 64-bit integer           |
| `:i64`  | 8 bytes | Signed 64-bit integer             |
| `:f64`  | 8 bytes | 64-bit IEEE 754 double            |
| `:word` | 4 or 8 bytes | Platform-width integer. Configured via `config :rekto, :word_type, :u32 \| :u64` |
| `{:string_buffer, n}` | `n` bytes | Fixed-size null-padded string buffer — see below |

All integers are **little-endian**. All reads are unsigned unless the `i`-prefix type is used.

### Array types

Arrays are expressed as a `{type, count}` tuple. Any type (including primitives and schemas)
can be used as the element type.

```elixir
field :name_buf,   {:u8, 16}     # 16 bytes read as a list of integers
field :matrix,     {:f32, 16}    # 4×4 float matrix as a flat list
field :inventory,  {ItemSchema, 8}  # 8 embedded ItemSchema structs
```

Arrays deserialize to Elixir lists.

### Embedded schemas

Pass a module name as the type to embed another schema inline. The embedded schema's fields
are read starting at the current offset. The result is a nested struct.

```elixir
field :transform, TransformSchema   # embeds TransformSchema at the current offset
```

The embedded struct's `__meta__` will have `assoc_type: :embed`.

### `{:string_buffer, n}` type

A fixed-size `n`-byte buffer for C-style null-terminated strings (`char name[n]`).

```elixir
field :name, {:string_buffer, 32}   # reads 32 bytes; deserializes to a string
```

- **`from_binary`** — returns `{:error, {:wrong_size, actual, n}}` if the binary is not `n` bytes.
- **`to_bytes`** — pads the string with null bytes to exactly `n` bytes. Raises if
  `byte_size(str) > n`. A fully-packed buffer with no null terminator (`byte_size(str) == n`) is
  allowed.
- **Scanning** — `to_bytes` produces the full `n`-byte pattern (content + null padding), so a scan
  will match the exact padded layout. Use the hex `bytes` mode with wildcards if you want to match
  the content prefix only.

### `:word` type

`:word` is a compile-time constant sized by `config :rekto, :word_type`. Use it for any field
whose size depends on the target process's pointer width — particularly `size_t`, `ptrdiff_t`,
and pointer-sized integers that are not themselves pointers.

```elixir
# In config/config.exs (set once for your target):
config :rekto, :word_type, :u64   # 64-bit target
config :rekto, :word_type, :u32   # 32-bit target
```

---

## Field Options

Options are passed as a keyword list after the field type:

```elixir
field :name, type, opt1: value1, opt2: value2
```

### Reserved options (handled by Rekto)

| Option            | Values                       | Description |
|-------------------|------------------------------|-------------|
| `:offset`         | `non_neg_integer`            | Byte offset of this field from the start of the struct. If omitted, the field immediately follows the previous one. |
| `:constraints`    | `[field_constraint]`         | List of constraints checked during `from_binary/2`. See [Constraints](#constraints). |
| `:points_to`      | `module \| nil`             | Internal marker set by the `points_to/3` macro. Do not set manually. |
| `:ptr_type`       | `:u32 \| :u64`              | Internal marker set by `points_to/3`. Do not set manually. |
| `:pointer_offset` | `integer \| field_name`      | Byte offset between the raw pointer address and the target struct base. See [`:pointer_offset`](#pointer_offset) below. |

### `:pointer_offset`

When a pointer field stores an address that doesn't point to the *start* of the target
struct — e.g. it points to a specific field within it — use `:pointer_offset` to declare
the relationship. During `Repo.preload`, the offset is **subtracted** from the raw address
to recover the base of the target struct. For `:this_pointer` fields, the expected value is
`__meta__.addr + offset`.

```elixir
# Integer form — fixed byte count
field :c_str, :this_pointer, pointer_offset: 16

# Field-name form — resolved at runtime to the byte offset of that field
field :c_str, :this_pointer, pointer_offset: :buffer

# Cross-schema: "this pointer points to :items within TargetSchema"
points_to :items_ptr, TargetSchema, pointer_offset: :items
```

The field-name form is preferred over hardcoded integers — it stays correct if the target
schema's layout changes, and reads as documentation of intent. Resolution happens at
runtime, so compile order between schemas doesn't matter.

**Resolution rules:**
- For `:this_pointer` fields: the field name is looked up in the field's own schema.
- For `points_to` fields: the field name is looked up in the pointed-to schema.
- For plain fields (no `this_pointer`, no `points_to`): the field name is looked up in the
  field's own schema.

**Real-world example — GCC `std::string` (SSO short form):**

```elixir
defmodule Rekto.GCC.StdString.Short do
  use Rekto.Schema

  schema do
    field :c_str,   :this_pointer, pointer_offset: :buffer, constraints: [non_null: true]
    field :length,  :word, constraints: [non_null: true, range: {1, 15}]
    field :buffer,  {:string_buffer, 16}
  end
end
```

Here `c_str` stores the address of the inline `buffer` field. The sanity check verifies that
`c_str == __meta__.addr + offset_of(:buffer)`. If this struct were pointed to by another
schema and preloaded, the raw `c_str` value minus `offset_of(:buffer)` would give the base
address of the `Short` struct.

### Custom options

Any other option is stored in the field's `opts` keyword list and is accessible via
`Rekto.Schema.Helpers`. Custom options have no effect on serialization — they are metadata
for use in sanity checks or by consumer code.

```elixir
field :vtable_ptr, :u32, in_module: true   # stored in opts; no automatic enforcement
field :heap_ptr,   :u32, in_heap: true     # stored in opts; no automatic enforcement
```

Use `Rekto.Schema.Helpers.get_fields_with_opt(struct, :in_module)` to retrieve fields
that have a given custom option.

---

## Pointer Fields

Use the `points_to/3` macro to declare a pointer field. Under the hood this is a regular
integer field, but Rekto wraps its value in a `Rekto.Association.NotLoaded` struct after
deserialization, recording the target address and type for later loading.

```elixir
points_to :target_field, TargetSchema
points_to :target_field, TargetSchema, ptr_type: :u64   # explicit 64-bit pointer
points_to :raw_ptr,      Rekto.Void                     # untyped pointer
```

After deserialization, a pointer field holds:
```elixir
%Rekto.Association.NotLoaded{addr: 0xDEADBEEF, data_type: TargetSchema}
```

**Default pointer width** is `:u32`. To change the default for all schemas:
```elixir
config :rekto, :default_ptr_type, :u64
```

`Rekto.Void` is a sentinel type for pointers whose concrete type is determined at runtime
(e.g. `void*`). Attempting to preload a `Rekto.Void` pointer will raise an exception.
A transformation (see below) is the normal way to set the concrete type on a `Rekto.Void` pointer.

---

## Special Field Shortcuts

These are shorthand macros that expand to common combinations of `field` and `points_to`:

### `:void_pointer`

```elixir
field :my_ptr, :void_pointer
```

Expands to: `points_to :my_ptr, Rekto.Void` — a word-sized untyped pointer.

### `:vtable`

```elixir
field :vftable, :vtable
```

Expands to a void pointer with three additional options:
- `constraints: [non_null: true]` — vtable pointers are never null in well-formed objects
- `memoize: true` — memoize the deserialized value for consistency checks
- `in_module: true` — vtable pointers always point into the executable image

Use this for the vtable pointer at offset 0 of any polymorphic C++ object.

### `:this_pointer`

```elixir
field :self, :this_pointer
field :c_str, :this_pointer, pointer_offset: :buffer   # points to a specific field
```

A word-sized field that must equal the address of the struct itself (`__meta__.addr`) plus
an optional `:pointer_offset`. Rekto injects a sanity check that compares the field's value
to `__meta__.addr + offset`; if the address is `nil` (unknown), the check is skipped.

Common use cases:
- C++ structs that store a `this` pointer (offset 0, no `pointer_offset` needed)
- Self-referential pointers like GCC `std::string`'s `c_str` field, which points to the
  inline `buffer` within the same struct (`pointer_offset: :buffer`)

---

## Constraints

Constraints are checked per-field during `from_binary/2`. If any constraint fails,
`from_binary/2` returns `{:error, [{field_name, error_reason}]}`.

Pass constraints as a list to the `:constraints` field option:

```elixir
field :length, :u32, constraints: [non_null: true, range: {1, 15}]
field :tag,    :u32, constraints: [const: 0xDEADBEEF]
```

### Constraint reference

| Constraint                       | Passes when                                       | Error tuple                          |
|----------------------------------|---------------------------------------------------|--------------------------------------|
| `:non_null`                      | Value is not `0` or `nil`                         | `{:non_null, value}`                 |
| `{:const, v}`                    | Value equals `v` exactly                          | `{:expected_const, v, value}`        |
| `{:not_const, v}`                | Value does not equal `v`                          | `{:unexpected_const, v}`             |
| `{:in, [v1, v2, ...]}`           | Value is a member of the list                     | `{:not_in, value, list}`             |
| `{:range, {min, max}}`           | `min <= value <= max` (inclusive)                 | `{:out_of_range, value, min, max}`   |
| `{:func, &Module.fun/1}`         | Function returns `true`, `:ok`, or `{:ok, _}`     | `{:func_false, fun, value}`          |
| `{:not, constraint}`             | The inner constraint **fails**                    | `{:constraint_must_not_hold, c}`     |
| `{:and, {c1, c2}}`              | Both `c1` and `c2` pass                           | `{:and_failed, r1, r2}`              |
| `{:or, {c1, c2}}`               | At least one of `c1`, `c2` passes                 | `{:or_failed, r1, r2}`              |
| `{:xor, {c1, c2}}`              | Exactly one of `c1`, `c2` passes                  | `{:xor_failed, r1, r2}`             |

**Important:** Only named functions (`&Module.function/arity`) work in `:func` constraints.
Anonymous functions (`fn x -> ... end`) cannot be stored as module attributes, so they
will raise a compile error.

```elixir
# Correct: named function
def positive?(n), do: n > 0
field :count, :i32, constraints: [func: &__MODULE__.positive?/1]

# Wrong: anonymous function (compile error)
field :count, :i32, constraints: [func: fn n -> n > 0 end]
```

---

## Sanity Checks

Sanity checks run after all field constraints pass. They receive the fully populated struct
and return `:ok` or `{:error, reason}`. They are for cross-field validation.

```elixir
defmodule PlayerStruct do
  use Rekto.Schema

  sanity :hp_lte_max_hp

  schema do
    field :hp,     :u32
    field :max_hp, :u32
  end

  def hp_lte_max_hp(%__MODULE__{hp: hp, max_hp: max}) do
    if hp <= max, do: :ok, else: {:error, "hp #{hp} exceeds max_hp #{max}"}
  end
end
```

- Declare with `sanity :function_name` (current module) or `sanity ModuleName, :function_name`
  for a function defined elsewhere.
- The function receives the struct as its only argument.
- Return `:ok` to pass, or `{:error, reason}` to fail.
- Sanity failures cause `from_binary/2` to return `{:error, reason}`.
- Multiple `sanity` declarations run in the order declared.

---

## Transformations

Transformations run last in the `from_binary/2` pipeline, after all constraints and sanity
checks have passed. They receive the struct and return either a modified struct or a different
type entirely.

```elixir
defmodule VersionedBuffer do
  use Rekto.Schema

  transform :annotate_version

  schema do
    field :version, :u16
    field :data,    {:u8, 32}
  end

  def annotate_version(%__MODULE__{version: v} = s) when v >= 3 do
    %__MODULE__{s | data: Enum.take(s.data, 16)}   # v3+ truncates data
  end
  def annotate_version(s), do: s
end
```

- Declare with `transform :function_name` or `transform ModuleName, :function_name`.
- The function receives the current struct and returns either a struct or `{:error, reason}`.
- Transformations can return a **different struct type** (e.g. selecting between two subtypes).
  When doing this, pass `struct.__meta__` to preserve address context.
- Multiple `transform` declarations run in declaration order.

---

## Schema Inheritance

A schema can extend another schema, inheriting all its fields, constraints, sanity checks,
and transformations. The child's fields are appended starting at the end of the parent.

```elixir
defmodule BaseObject do
  use Rekto.Schema

  schema do
    field :vftable, :vtable
    field :refcount, :u32
  end
end

defmodule DerivedObject do
  use Rekto.Schema

  schema extends: BaseObject do
    # these fields start at offset 8 (after vftable + refcount)
    field :name_ptr, :u32
    field :flags,    :u32
  end
end
```

```elixir
Rekto.Schema.Helpers.get_field_names(DerivedObject)
# => [:vftable, :refcount, :name_ptr, :flags]

Rekto.Schema.Helpers.get_field_offset!(DerivedObject, :name_ptr)
# => 8
```

The child struct is a distinct Elixir struct — it does not have an Elixir `is_a?` relationship
with the parent. Inheritance is purely about layout.

---

## Metadata and Address Context

Every deserialized struct has a `__meta__` field of type `Rekto.Schema.Metadata`:

```elixir
%Rekto.Schema.Metadata{
  addr: non_neg_integer | nil,   # the struct's address in the target process, or nil
  assoc_type: :none | :embed | :pointer,
  memo: any                      # ETS tid for memoization
}
```

Pass metadata as the second argument to `from_binary/2`:

```elixir
{:ok, struct} = MySchema.from_binary(bytes, %{addr: 0x7FFF1234, assoc_type: :pointer})
```

`Rekto.Schema.Metadata.default/0` returns `%{addr: nil, assoc_type: :none}`. Calling
`from_binary(bytes)` (one argument) uses the default.

### `assoc_type` values

| Value       | Meaning                                                          |
|-------------|------------------------------------------------------------------|
| `:none`     | Top-level struct (not part of an association)                    |
| `:embed`    | Embedded directly inside another struct's byte buffer            |
| `:pointer`  | Read by following a pointer from another struct                  |

### When `addr` matters

- `:this_pointer` sanity checks compare the field value to `addr`; they are skipped if `addr: nil`.
- `memoize: true` fields use `addr` as the ETS lookup key; skipped if `addr: nil`.
- `Krebs.Repo.get/3` and `Krebs.Scanner.read_type/2` always populate `addr` correctly.

---

## MCP Workflow

### Discovering existing schemas

```
list_schemas
```
Returns a list of all compiled schema module names as strings.

```
schema_info {"module": "Rekto.VCPP.StdString.Short"}
```
Returns field-by-field breakdown:
```
Module: Rekto.VCPP.StdString.Short
Size: 24 bytes
Fields:
  +0x000 [16b] :buffer :: {:string_buffer, 16}
  +0x010 [4b]  :length :: :u32 [non_null: true, range: {0, 15}]
  +0x014 [4b]  :capacity :: :u32 [non_null: true, const: 15]
```

Columns: `+offset [size_in_bytes] :field_name :: type [constraints] -> pointer_target`

### Writing and loading a new schema

Use the `eval` tool to define a schema at runtime. The module will be compiled into the
running BEAM instance and auto-registered in `Rekto.SchemaRegistry`.

```
eval {"code": "
defmodule MyGame.Player do
  use Rekto.Schema

  schema do
    field :hp,        :u32
    field :max_hp,    :u32
    field :position,  {:f32, 3}
    field :name_len,  :u32,  offset: 0x50
  end
end
"}
```

After `eval` succeeds, `schema_info {"module": "MyGame.Player"}` will show the layout.

### Reading a struct from memory

```
read_type {"addr": "0x12345678", "type": "MyGame.Player"}
```

Or with the scanner name if you have multiple scanners:
```
eval {"code": "Krebs.Repo.get(MyGame.Player, 0x12345678)"}
```

### Scanning for structs

```
scan {"pattern": "?? ?? ?? ?? 64 00 00 00", "mem_type": "heap"}
```

Wildcards (`??`) skip bytes you don't know. Use `schema_info` to find the bytes
at a known offset (e.g. `max_hp = 100` at offset 4 → pattern `?? ?? ?? ?? 64 00 00 00`).

---

## Common C++ Patterns

### Plain data struct

```elixir
defmodule MyGame.Vec3 do
  use Rekto.Schema

  schema do
    field :x, :f32
    field :y, :f32
    field :z, :f32
  end
end
```

### Struct with vtable (polymorphic C++ object)

```elixir
defmodule MyGame.Entity do
  use Rekto.Schema

  schema do
    field :vftable, :vtable       # 4 or 8 bytes; non-null; in module image
    field :refcount, :u32
    field :hp,       :u32
    field :max_hp,   :u32
  end
end
```

### Struct with pointer to heap data

```elixir
defmodule MyGame.Inventory do
  use Rekto.Schema

  schema do
    points_to :items, MyGame.Item   # raw address stored as :u32 (or :u64 if configured)
    field :count,    :u32
    field :capacity, :u32, constraints: [range: {0, 1024}]
  end
end
```

### Struct with padding / unknown fields

Use `:offset` to skip bytes you don't care about:

```elixir
defmodule MyGame.Player do
  use Rekto.Schema

  schema do
    field :hp,       :u32
    field :max_hp,   :u32
    # 40 bytes of unknown fields follow
    field :position, MyGame.Vec3, offset: 0x30
    # skip more unknown data
    field :team_id,  :u32, offset: 0x80
  end
end
```

Skipped bytes are silently discarded by `from_binary/2`.

### Struct with a fixed-size string buffer

```elixir
defmodule MyGame.Player do
  use Rekto.Schema

  schema do
    field :hp,   :u32
    field :name, {:string_buffer, 32}   # char name[32]
  end
end
```

Querying by name scans for the exact padded bytes:

```elixir
query = Rekto.Query.from(MyGame.Player, where: [name: "Alice"])
{:ok, results} = Krebs.Repo.all(query)
```

### Discriminated union (two layouts sharing a prefix)

```elixir
defmodule Buffer.Prefix do
  use Rekto.Schema

  transform :resolve_type

  schema do
    field :flags, :u32
    field :size,  :u32
  end

  def resolve_type(%__MODULE__{flags: f} = prefix) do
    if Bitwise.band(f, 0x1) == 0 do
      {:ok, s} = Buffer.Short.from_binary(Buffer.Prefix.to_binary(prefix), prefix.__meta__)
      s
    else
      {:ok, l} = Buffer.Long.from_binary(Buffer.Prefix.to_binary(prefix), prefix.__meta__)
      l
    end
  end
end
```

---

## Error Reference

### Constraint errors

`from_binary/2` returns `{:error, [{field_name, reason}]}` when constraints fail.
`reason` is a structured tuple, not a plain string:

| Error tuple                         | Meaning                                        |
|-------------------------------------|------------------------------------------------|
| `{:non_null, 0}`                    | Field was zero / nil                           |
| `{:expected_const, expected, got}`  | Wrong value for a `{:const, v}` constraint     |
| `{:unexpected_const, v}`            | Value matched a `{:not_const, v}` constraint   |
| `{:not_in, value, list}`            | Value not in the `{:in, [...]}` list           |
| `{:out_of_range, value, min, max}`  | Value outside `{:range, {min, max}}`           |
| `{:func_false, fun, value}`         | Custom function returned `false` or `:error`   |
| `{:constraint_must_not_hold, c}`    | Value matched a `{:not, c}` constraint         |
| `{:and_failed, r1, r2}`             | One or both branches of `{:and, ...}` failed   |
| `{:or_failed, r1, r2}`              | Both branches of `{:or, ...}` failed           |
| `{:xor_failed, r1, r2}`             | Both or neither branch of `{:xor, ...}` passed |

### Sanity check errors

Sanity check failures return `{:error, reason}` where `reason` is whatever the sanity
function returns as its error value (a string, tuple, or any term).

### Binary size errors

If the binary passed to `from_binary/2` is not exactly `MySchema.__size__/0` bytes,
deserialization will raise (not return an error). Use `Krebs.Scanner.read(addr, MySchema.__size__())`
to read the correct number of bytes before calling `from_binary/2`.
