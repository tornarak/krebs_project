# Krebs Project — CLAUDE.md

A binary memory query engine and analysis toolkit. Portfolio project demonstrating
Rust/Elixir interop, memory scanning, C++ struct parsing, and MCP tooling.

**Primary languages:** Rust, Elixir
**Build requires:** Rust nightly (for `portable_simd` + `associated_type_defaults`)

---

## Component Map

```
krebs_project/
├── Cargo.toml         ← workspace root (members: libkrebs)
├── SCHEMAS.md         ← Rekto schema DSL reference (field types, constraints, MCP workflow)
├── libkrebs/          Rust library — core memory I/O, pattern scanning, C++ struct parsing, scanner
│   └── examples/      cpp_scan + assault_cube (AssaultCube hack demo) + cpp_fixture/ C++ companion
├── rekto/             Elixir app   — Ecto-inspired binary struct schema DSL
├── krebs/             Elixir app   — NIF bindings + MCP server + HexDump + Scanner GenServer
│   └── native/libkrebs_nif/  Rust NIF crate (standalone — outside workspace, managed by rustler)
├── neoplasm/          Elixir app   — egui GUI frontend (depends on krebs + rekto)
│   └── native/neoplasm_nif/  Rust NIF crate — egui/eframe window + GUI↔Elixir bridge
└── deprecated/        Old code — cancex (Roblox schemas), yeet_engine, assault_krebs, aleph_*, kreblox
```

---

## libkrebs (Rust crate)

**Path:** `libkrebs/`
**Nightly features:** `#![feature(portable_simd)]`, `#![feature(associated_type_defaults)]`

### Key modules

| Path                    | Contents                                                                                       |
|-------------------------|------------------------------------------------------------------------------------------------|
| `src/lib.rs`            | Crate root; defines `NativeProcess` type alias                                                 |
| `src/error.rs`          | All error types (`KrebsError`, `MemError`, `StdError`, `ScannerError`, vcpp/gcc/common errors) |
| `src/mem/`              | `Reader`/`Writer` traits, `ProcMap`, `Region`, `Module`, `Buffer`, `Process` trait             |
| `src/pattern_scan/`     | `BytePattern`, `WordPattern`, `Simd32Pattern`, `ScanMatch`, `ScanPattern` trait                |
| `src/scanner/`          | `Scanner`, `MatchPredicate` — parallel heap/module scanning built on `rayon`                   |
| `src/cpp_std/vcpp/`     | MSVC `std::string` and `std::vector` representations                                           |
| `src/cpp_std/gcc/`      | GCC `std::string` representation                                                               |
| `src/unix/`             | `ProcFs` — Linux process handle via `/proc/{pid}/mem`                                          |
| `src/win/`              | Windows process handle via Win32 API                                                           |
| `src/verifiable.rs`     | `Verifiable` trait (default `type Error = Infallible`)                                         |
| `examples/cpp_scan.rs`  | Interactive demo — attach to C++ process, scan for known string                                |
| `examples/cpp_fixture/` | C++ companion: `linux/main.cpp`, `windows/` (VS project), `Makefile`                           |

### Platform abstraction

```rust
// In lib.rs — the canonical way to refer to the native process handle
#[cfg(windows)] pub type NativeProcess = win::OpenedProcess;
#[cfg(unix)]    pub type NativeProcess = unix::ProcFs;
```

```
KrebsError
├── Mem(MemError)
│   ├── Windows(WinMemError)   — Win32 error codes (u32)
│   └── Unix(UnixMemError)     — io::ErrorKind variants
└── CppStd(StdError)
    ├── Vcpp(vcpp::Error)
    │   ├── String(vcpp::StringError) → Short / Long
    │   └── Vector(vcpp::VectorError)
    ├── Gcc(gcc::Error)
    │   └── String(gcc::StringError) → Short / Long
    └── CommonString(CommonStringError)  — cross-ABI buffer/IO/UTF-8 errors

ScannerError  (peer of KrebsError — address validation, not I/O)
├── NotInHeap { addr, desc }
└── NotInModule { addr, desc }
```

All error types implement `Clone`. No `cfg` on type definitions; only behavioral code is gated.

### Pattern scan types
- `BytePattern` — any length, slowest
- `WordPattern` — length must be multiple of word size
- `Simd32Pattern` — up to 32 bytes, padded; fastest (requires `portable_simd`)

---

## libkrebs::scanner

**Path:** `libkrebs/src/scanner/`
**Extra dep:** `rayon`

Formerly a standalone `yeet_engine` crate; folded into libkrebs. The old crate is preserved
at `deprecated/yeet_engine/` for reference.

### Key types

**`Scanner`** (`src/scanner/scanner.rs`)
- Wraps `Arc<Mutex<NativeProcess>>` + `MemoryLayout` + `read_size`
- Constructors: `new(pid, read_size)`, `from_proc(proc, read_size)`, `from_arc(arc, read_size)`
- `from_arc` is used by the NIF to share a process handle between `ProcessResource` and `ScannerResource`
- Scanning: `scan_heap`, `scan_heap_until`, `scan_module`, `scan_module_until` (all `rayon`-parallel; no `print` param — timing logged via `log::info!`)
- Helpers: `is_in_heap`, `is_in_module`, `heap_ptr`, `module_ptr`
- Implements `Reader`

**`ScannerError`** — lives in `libkrebs::error`, re-exported at `libkrebs::scanner::ScannerError`

**Terminology note:** "heap" = all writable memory after the executable; "module" = writable memory inside the executable image.

---

## assault_cube example

**Path:** `libkrebs/examples/assault_cube/`
**Run:** `cargo +nightly run --example assault_cube [PID]` (falls back to interactive stdin)
**Status:** Working — verified on Linux 64-bit and Windows.

AssaultCube memory hack demo. Shows libkrebs usage for reading game state.

### Platform-specific offsets (`src/assault_cube_hack.rs`)

All platform-varying constants use `#[cfg(windows)]` / `#[cfg(unix)]`.

| Constant           | Windows      | Linux (64-bit) | Notes                                        |
|--------------------|-------------|----------------|----------------------------------------------|
| `player_obj_ptr`   | `0x0050F4F4` | `0x005A3518`   | Static ptr in module → player object base    |
| `hp_offset`        | `+0xF8`      | `+0x100`       | +8 from 64-bit vtable pointer growth         |
| `gun_offset`       | `+0x374`     | `+0x398`       | 9-slot array × 4→8 bytes; start at `+0x350` |
| `reserve_offset`   | `+0x10`      | `+0x20`        | Within gun struct; 4 preceding ptrs grew     |
| `magazine_offset`  | `+0x14`      | `+0x28`        | Within gun struct                            |
| `ammo_value_offset`| `0x00`       | `0x00`         | Gun ammo ptr points directly at int32        |
| `can_jump_offset`  | `+0x69`      | `+0x69`        | Same on both (unverified on Linux)           |
| `velocity_offset`  | `+0x28`      | `+0x28`        | Same on both                                 |

Linux process map (verified against live 64-bit `linux_64_client`):
- Module: `0x400000..0x59F000`, writable region `0x59B000..0x59F000`
- Heap: `~0x19E27000..0x1CBED000`

---

## rekto (Elixir app)

**Path:** `rekto/`
**Config:** `config :rekto, :word_type, :u32` (default; also supports `:u64`)

### Key modules

| Module                     | Purpose                                                               |
|----------------------------|-----------------------------------------------------------------------|
| `Rekto.Schema`             | DSL for defining binary struct schemas (macros, callbacks)            |
| `Rekto.Schema.Info`        | Consolidated introspection struct (replaces per-field callbacks)      |
| `Rekto.Schema.Helpers`     | Schema utility functions                                              |
| `Rekto.Schema.Constraints` | Constraint definitions (`non_null`, `in_heap`, `in_module`, `range`)  |
| `Rekto.Schema.Sanity`      | Auto-injected sanity checks for DSL primitives (`:this_pointer`)      |
| `Rekto.Schema.Metadata`    | Per-struct metadata: `addr`, `assoc_type`, `memo`                     |
| `Rekto.Serialization`      | `from_bytes/2,3`, `to_bytes/2`, `get_type_size!/1`, `get_word_size/0` |
| `Rekto.SchemaRegistry`     | ETS table of all compiled schema modules                              |
| `Rekto.Void`               | Marker type for untyped pointer targets                               |
| `Rekto.MemoTable`          | Caller-owned ETS table for `memoize: true` field enforcement          |

For a full field-type reference, constraint syntax, and MCP workflow, see **`SCHEMAS.md`** at the project root.

### Schema callbacks (public contract)
- `__size__/0` — byte size of the struct
- `__schema__/0` — returns `%Rekto.Schema.Info{}`
- `from_binary/2` — `(binary, map | Metadata.t) → {:ok, struct} | {:error, any}`
- `to_binary/1` — `(struct) → binary`
- `garbage_mask/0`, `custom_mask/1` (optional)

### `Rekto.Schema.Metadata`
```elixir
%Rekto.Schema.Metadata{
  addr: non_neg_integer | nil,   # struct's address in memory; nil if not known
  assoc_type: :none | :embed | :pointer,
  memo: any                      # optional ETS tid from Rekto.MemoTable
}
```
`Metadata.default/0` returns `%{addr: nil, assoc_type: :none}` (no memo). Plain maps accepted.

### C++ memory-pattern field types (v1.2)

In addition to raw types (`:u32`, `:f32`, etc.) and `points_to`, the schema DSL provides:

| Syntax | Expands to |
|--------|-----------|
| `field :f, :void_pointer` | `points_to :f, Rekto.Void` — word-sized ptr to untyped target |
| `field :f, :vtable` | void_pointer + `constraints: [non_null: true]` + `memoize: true` + `in_module: true` |
| `field :f, :this_pointer` | word-sized field; sanity check asserts `value == __meta__.addr` (skipped when `addr: nil`) |
| `field :f, :u32, memoize: true` | any field — postlude injects `MemoTable.check_struct_with_meta/1` |
| `field :f, :u32, in_module: true` | informational opt — no rekto enforcement; upstream code reads via schema introspection |

**`:this_pointer`** — single `{Rekto.Schema.Sanity, :check_this_pointers}` check injected
once regardless of how many `:this_pointer` fields exist. Checks all of them.

**`memoize: true`** — single `{Rekto.MemoTable, :check_struct_with_meta}` check injected. Reads
the ETS tid from `struct.__meta__.memo`; no-op when `nil`. Caller creates and passes the table:

```elixir
# In upstream supervision tree (e.g. Krebs.Scanner)
children = [{Rekto.MemoTable, name: :scanner_memo}]

# When deserializing
tid = Rekto.MemoTable.tid(:scanner_memo)
MySchema.from_binary(bytes, %{addr: 0x1234, assoc_type: :pointer, memo: tid})
```

`Rekto.MemoTable` is **not** in rekto's supervision tree — rekto itself is pure.

**`in_module: true` / `in_heap: true`** — informational opts only; stored in field info, readable via
`Helpers.get_fields_with_opt/2`. Consumer projects implement enforcement by wrapping `use Rekto.Schema`
with their own sanity checks that inspect these opts against Scanner's known memory layout:

```elixir
defmodule MyProject.Schema do
  def module_before_heap(%{} = struct) do
    struct |> Helpers.get_fields_with_opt(:in_heap)
    # |> validate against Scanner's known heap regions...
    :ok
  end

  defmacro __using__(_) do
    use Rekto.Schema
    sanity MyProject.Schema, :module_before_heap
  end
end
```

### `{:string_buffer, n}` type
Fixed-size null-padded string buffer (C-style `char[n]`).

- `get_type_size!({:string_buffer, n})` → `n`
- `to_bytes(str, {:string_buffer, n})` — pads with null bytes to exactly `n`; raises if `byte_size(str) > n` (`<= n` allowed, so fully-packed buffers with no terminator are valid)
- `from_bytes(binary, {:string_buffer, n})`; returns `{:error, {:wrong_size, actual, n}}` if `byte_size(binary) != n`
- In neoplasm schema queries, string values pass through `parse_for_type/2` as-is

### `:word` type
- Compile-time platform-width integer (4 or 8 bytes)
- Configured via `Application.compile_env(:rekto, :word_type, :u32)`
- `Rekto.Serialization.get_word_size/0` returns the byte count as a constant

### SchemaRegistry
- ETS table populated at startup by scanning `:rekto` + apps in `:rekto, :schema_apps` config
- Hot-reload: schema postlude tries `Rekto.SchemaRegistry.register(__MODULE__)` (fails silently)
- `Rekto.Schema.has_schema?/1` — handles both compiled and in-compilation modules
- API: `SchemaRegistry.all/0 → [module]`, `SchemaRegistry.count/0`

---

## libkrebs_nif (Elixir + Rust NIF)

**Path:** `krebs/`
**NIF crate:** `native/libkrebs_nif/` (links `libkrebs` only)

### Application startup

`Krebs.Application` starts:
1. `Krebs.LogBuffer` — ring buffer GenServer for log entries (must be first)
2. `Krebs.ScannerRegistry` — ETS: active scanner instances `{name, pid}`
3. `Krebs.ScanSetRegistry` — ETS: active scan sets `{name, pid}`
4. `Bandit` HTTP server — MCP server on port 4040 (`:mcp_port` env, default 4040)

After `Supervisor.start_link` returns, `Application.start/2` also:
- Adds `Krebs.LogBuffer.Backend` as an Elixir Logger backend
- Calls `Krebs.Nif.nif_log_init(Process.whereis(Krebs.LogBuffer))` to wire the Rust log bridge

### NIF resource model

Two BEAM resources backed by Rust:
- **`ProcessResource`** — `Arc<Mutex<NativeProcess>>`; created by `Nif.attach/2`
- **`ScannerResource`** — `Mutex<Scanner>`; created by `Nif.scanner_new/2` (clones the Arc)

Both are wrapped in NifStructs for ergonomic Elixir access:
- `%Krebs.ProcessRef{resource: ...}` — `"Elixir.Krebs.ProcessRef"`
- `%Krebs.ScannerRef{resource: ...}` — `"Elixir.Krebs.ScannerRef"`

### NIF surface (`Krebs.Nif`)

Raw stubs — do not call directly from application code; use `Krebs.Scanner`.

```
Patterns:        new_scan_pattern, concat_scan_patterns
Process static:  list_processes, search_processes, list_windows, search_windows
Process attach:  attach, process_pid, executable_name, window_names, access_level, close
Memory I/O:      read, write
Proc map:        regions, modules
Scanner:         scanner_new, refresh_layout, is_in_memory, scan
Log bridge:      nif_log_init
```

### Error encoding (`src/errors.rs`)

`Enc<E>` newtype wraps all libkrebs error types with `Encoder` impls.
Wire format: unit variant → atom; struct/tuple variant → `{atom, [{field, value}, ...]}`

### `Krebs.Scanner` GenServer

Multi-instance; default name `Krebs.Scanner`.

```elixir
# Start
Scanner.start_link(pid: 1234)
Scanner.start_link(pid: 1234, name: :game, access: :read_write, read_size: 2*1024*1024)

# Key operations
Scanner.read(addr, size)           # → {:ok, binary} | {:error, _}
Scanner.write(addr, data)          # → {:ok, n} | {:error, _}
Scanner.regions()                  # → {:ok, [%Krebs.Region{}]}
Scanner.modules()                  # → {:ok, [%Krebs.Module{}]}
Scanner.scan(pattern, recipient, opts) # async; sends %ScanMatch{} structs or {:done, match_count} to recipient
Scanner.scan_stream(pattern, opts) # async; returns Stream of %ScanMatch{}-es
Scanner.in_memory?(addr, mem_type)
Scanner.refresh_layout()           # cast — async
Scanner.read_type(addr, type)      # reads + deserializes via Rekto.Serialization
```

Auto-refreshes memory layout every 60s via `Process.send_after`.

### `Krebs.Scanner.ScanSet`

Iterative scanning (Cheat Engine "Next Scan" model). A named GenServer that accumulates
scan results and intersects them on each subsequent scan.

```elixir
{:ok, ss} = ScanSet.start_link(name: :health)
ScanSet.scan(ss, Krebs.Scanner, pattern, :heap)
ScanSet.await(ss)    # blocks until scan committed
ScanSet.count(ss)
ScanSet.results(ss)  # → MapSet of addresses
ScanSet.reset(ss)
```

### `Krebs.Repo`

Stateless module implementing `Rekto.Repo` against `Krebs.Scanner`. No GenServer.
`use Rekto.Repo` provides `get/3`, `all/2`, `one/2`, `preload/2`.

```elixir
# Read a struct from memory
Krebs.Repo.get(MySchema, 0x12345678)

# Query — scans heap by default
query = Rekto.Query.from(MySchema, where: [hp: 100])
{:ok, results} = Krebs.Repo.all(query)
{:ok, results} = Krebs.Repo.all(query, mem_type: :module)
{:ok, results} = Krebs.Repo.all(query, scanner: :game)  # named scanner
```

`convert/3` — sugar for `get/3` when you already have a struct and want to reread
it at the same address as a different schema.

`execute_query/2` opts: `:mem_type` (`:heap` | `:module`, default `:heap`),
`:scanner` (GenServer name/pid, default `Krebs.Scanner`).

Pattern optimization: leading/trailing wildcard bytes are stripped before scanning —
shorter patterns scan faster.

### `Krebs.HexDump`

ANSI-colored hex dump with type inference.

```elixir
HexDump.hex_dump(addr, n_bytes)              # word_size from Rekto.Serialization.get_word_size()
HexDump.hex_dump(addr, n_bytes, word_size)
HexDump.hex_dump(server, addr, n_bytes)
HexDump.hex_dump(server, addr, n_bytes, word_size)
HexDump.type_dump(addr, n_bytes, type)       # fixed type, no inference
```

Inference per chunk: `[:u32, :i32, :f32, :heap, :module]` for 4-byte words;
`[:u64, :i64, :f64, :heap, :module, :u32, :i32, :f32]` for 8-byte words.

### MCP Server

Vancouver-based MCP server at port 4040.

Tools: `list_scanners`, `list_processes`, `search_processes`, `list_windows`, `search_windows`,
`attach`, `detach`, `regions`, `modules`, `read_bytes`, `write_bytes`, `hex_dump`,
`scan`, `scan_set_new`, `scan_set_scan`, `scan_set_results`,
`scan_set_reset`, `scan_set_delete`, `scan_set_list`, `read_type`, `list_schemas`,
`schema_info`, `eval`, `get_logs`.

Pattern syntax for `scan` / `scan_set_scan`: space-separated hex bytes, `??` as wildcard.
`eval` tool: executes arbitrary Elixir code with `scanner` binding.
`get_logs` tool: fetches recent entries from `Krebs.LogBuffer`; params `limit` (int, default 50) and `level` (string: `trace`/`debug`/`info`/`warning`/`error`).

### `Krebs.LogBuffer`

Ring buffer for log entries from both Elixir Logger and the Rust NIF layer.

- GenServer owning `:krebs_log_buffer` ETS `:ordered_set` table (500-entry cap)
- Async inserts via `GenServer.cast` — no caller blocks
- Receives `{:krebs_log, level_atom, target, message}` messages from Rust bridge
- Also fed by `Krebs.LogBuffer.Backend` (Elixir `:gen_event` Logger backend)

```elixir
LogBuffer.insert(entry)       # async; entry is a map with :ts, :level, :source, :module, :message
LogBuffer.get_recent(n \\ 50) # → [map], oldest first
LogBuffer.clear()
```

Entry map shape:
```elixir
%{
  ts: "2024-01-01T00:00:00.000Z",  # ISO 8601
  level: :info,                     # :error | :warning | :info | :debug | :trace
  source: :elixir,                  # :elixir | :rust
  module: "Krebs.Scanner",
  message: "..."
}
```

### Logging architecture

```
Rust (libkrebs + libkrebs_nif)
  log::info!/debug!/warn!/error!/trace!
       │
       ▼
  nif_logger::NifLogger (log::Log impl)
       │  mpsc::SyncSender<LogEntry>  (channel cap=256)
       ▼
  forwarding thread → OwnedEnv::send_and_clear → {:krebs_log, level_atom, target, message}
       │
       ▼
Krebs.LogBuffer (handle_info {:krebs_log, ...})
       │
       ▼
  ETS ring buffer (:krebs_log_buffer)

Elixir (Logger)
  Logger.info/debug/warning/error
       │
       ▼
  Krebs.LogBuffer.Backend (:gen_event handler)
       │
       ▼
  Krebs.LogBuffer.insert/1 (GenServer.cast)
       │
       ▼
  ETS ring buffer (:krebs_log_buffer)
```

- Rust `log::Level::Warn` maps to Elixir atom `:warning` (OTP convention)
- All five levels preserved: `:error`, `:warning`, `:info`, `:debug`, `:trace`
- `nif_log_init` is safe to call multiple times; old forwarding thread exits when its channel is replaced

### Mix task

```bash
iex -S mix krebs.attach 1234
iex -S mix krebs.attach --name RobloxPlayerBeta
iex -S mix krebs.attach --pid 1234
```

---

## neoplasm (Elixir + Rust GUI)

**Path:** `neoplasm/`
**Deps:** `krebs`, `rekto` (path deps); `rustler`
**NIF crate:** `native/neoplasm_nif/` — egui/eframe window; outside the Cargo workspace

Neoplasm is the graphical frontend. It wraps all of krebs's capabilities (attach, scan, schema
query) in an egui GUI window managed by a Rustler NIF. All scan logic stays in Elixir/krebs;
the NIF owns only the render loop and the GUI↔Elixir message bridge.

### Application startup

`Neoplasm.Application` starts:
1. `Neoplasm.Session` — GenServer; cross-tab persistent state (saved addresses, loaded schemas) — stub for now
2. `Neoplasm.Scanner` — GenServer; owns attach/scan lifecycle
3. `Neoplasm.Window` — GenServer; owns the NIF lifecycle and routes GUI events

`Neoplasm.Window.init/1` calls `Nif.open_window(self())` and `Nif.set_schemas(...)` immediately.

### Elixir modules

| Module | Role |
|--------|------|
| `Neoplasm.Nif` | Rustler stubs — single contact point with `neoplasm_nif` |
| `Neoplasm.Window` | GenServer; routes GUI events → Scanner or Krebs; fires NIF push-calls back |
| `Neoplasm.Scanner` | GenServer; owns the `Krebs.Scanner` instance (named `:neoplasm_krebs_scanner`); streams results to NIF |

### GUI ↔ Elixir data flow

```
GUI thread (egui render loop)
  user action → GuiEvent
       │  OwnedEnv::send_and_clear → {:neoplasm, event_atom, ...args}
       ▼
Neoplasm.Window (GenServer handle_info)
  routes to Neoplasm.Scanner or Krebs directly
       │
       ▼
Neoplasm.Scanner (GenServer handle_cast / handle_info)
  calls Krebs.Scanner, Krebs.Repo, etc.
  streams ScanMatch → Nif.push_results / Nif.set_scan_status

Elixir → GUI (NIF push calls, any process may call):
  Nif.set_attached(Option<{pid, name}>, Option<error_string>)
  Nif.set_processes([{pid, name}])
  Nif.set_schemas([{name, %Schema.Info{}}])
  Nif.set_scan_status(:idle | :scanning | {:done, count})
  Nif.push_results([addr])
  Nif.clear_results()
  Nif.set_scan_pattern(hex_string)   ← byte preview pushed by Elixir
  Nif.close_window()
```

### GUI events (Rust → Elixir)

All events are tuples `{:neoplasm, event_atom, ...args}` sent to `Neoplasm.Window`:

| Event | Args | Handler |
|-------|------|---------|
| `list_processes_requested` | — | calls `Krebs.Nif.list_processes()`, pushes via `set_processes` |
| `attach_requested` | `pid :: u32` | `Neoplasm.Scanner.attach(pid)` |
| `detach_requested` | — | `Neoplasm.Scanner.detach()` |
| `primitive_scan_requested` | `type_name, value, region, next` | cast `{:scan_primitive, ...}` to Scanner |
| `primitive_preview_requested` | `type_name, value` | Elixir serializes → `set_scan_pattern` |
| `schema_preview_requested` | `schema_name, fields` | `Krebs.Repo.pattern_hex_for_query` → `set_scan_pattern` |
| `query_requested` | `schema_name, fields` | cast `{:query, ...}` to Scanner |
| `iex_requested` | — | spawns `x-terminal-emulator` with `iex --remsh` |
| `window_closed` | — | `{:stop, :normal, state}` |

### Byte pattern pipeline

All value→bytes conversion goes through Elixir for correctness (endianness, type width):

- **Typed primitives** (f32, u32, bool, word, etc.): `Rekto.Serialization.to_bytes(value, type)`
- **Bytes (hex) mode**: `Krebs.MCP.parse_hex_pattern(str)` — same format as MCP `scan` tool
- **Schema queries**: `Krebs.Repo.pattern_hex_for_query/1` — full struct layout with `??` for unset bytes (leading offset bytes and trailing remainder also shown as `??`)

Preview events (`primitive_preview_requested`, `schema_preview_requested`) trigger Elixir to push
back a hex string via `Nif.set_scan_pattern/1`; the GUI snapshots `GuiState::scan_pattern` each
frame and shows it in the Bytes display.

### Rust NIF architecture (`neoplasm_nif`)

**Source layout:**
```
native/neoplasm_nif/src/
├── lib.rs              — mod declarations; pub use types::*; rustler::init!
├── atoms.rs            — rustler::atoms! macro
├── schema.rs           — Decoder impls for SchemaInfo / FieldInfo from Elixir
├── types.rs            — shared data model: ValueType, value_to_hex, ScanRegion,
│                         ScanStatus, HexRow, MemKind, MemRegion
└── window/
    ├── mod.rs          — GuiState, WindowState, GuiCommand, STATE, with_gui_state
    ├── app.rs          — NeoplasmApp, ActiveTab, eframe::App impl, toolbar
    ├── scanner_tab.rs  — ScannerTab, SearchMode, FieldState, SavedAddress, impl ScannerTab
    ├── hex_dump_tab.rs — HexDumpTab, impl HexDumpTab, primary_byte_color
    ├── events.rs       — GuiEvent enum, send_event, encode_event
    └── nifs.rs         — all 13 #[rustler::nif] pub fns
```

`static STATE: Mutex<Option<WindowState>>` lives in `window/mod.rs`.
`with_gui_state<F: FnOnce(&mut GuiState)>` is the shared helper — locks STATE then
gui_state, calls `f`, releases both. All NIF callsites use it instead of double-locking.

`WindowState`:
```
Mutex<Option<WindowState>>
  ├── alive: AtomicBool
  ├── cmd_tx: Sender<GuiCommand>          ← Elixir → GUI thread commands (Close)
  ├── gui_state: Arc<Mutex<GuiState>>     ← written by NIF calls, read by render loop
  ├── calls: Mutex<HashMap<token, Sender>>← pending nif_reply slots (future use)
  └── recipient: LocalPid                 ← Neoplasm.Window pid; target for GUI events
```

`open_window` behaviour: if the window thread is alive but `Neoplasm.Window` crashed and
restarted with a new pid, `recipient` is updated in-place — window keeps running. If the
thread is dead or absent, fresh `WindowState` is built and a new thread is spawned.

`GuiState` (written by NIF calls, snapshot-read by render loop each frame):
```rust
pub struct GuiState {
    pub processes: Vec<(u32, String)>,
    pub results: Vec<u64>,
    pub scan_status: ScanStatus,           // Idle | Scanning | Done(n)
    pub schemas: Vec<SchemaInfo>,
    pub attached: Option<(u64, String)>,   // Some((pid, name)) = attached
    pub scan_pattern: String,              // Elixir-computed hex preview
    pub hex_rows: Vec<HexRow>,             // hex dump inspector rows
    pub memory_layout: Vec<MemRegion>,     // for mem-map sidebar
    pub error: Option<String>,             // generic error modal
}
```

The render loop (`NeoplasmApp::update`) briefly locks `gui_state` once per frame to snapshot
all fields into local app/tab state, then releases the lock before any egui rendering.

### GUI structure

Tabs: **Scanner** and **Hex Dump**, toggled from the toolbar.

`NeoplasmApp` holds app-level state: `attached`, `error`, `processes`,
`selected_process`, `active_tab`.

**Scanner tab** (`ScannerTab`): search mode, value/pattern input, type selection, region,
results, schemas, field states, `scan_pattern`, saved addresses.

Layout (Scanner tab):
- **Toolbar** (top): process ComboBox + Refresh; tab buttons; IEx + Attach/Detach (right)
- **Saved addresses** (bottom, min 160px): editable address list with labels
- **Central** (two columns): left = results list (distinct bg color); right = scan controls

Scan controls:
- Region selector (Heap / Module)
- Mode toggle: **Primitive** or **Schema** (mutually exclusive)
- *Primitive mode*: type ComboBox; optional array element type + count; value input; Bytes display; Scan + Next Scan
- *Schema mode*: schema ComboBox; per-field rows (checkbox, name label, type label, value input, bytes preview); full struct pattern display; Scan

**Hex Dump tab** (`HexDumpTab`): address input + Go, scrollable hex grid with type-coloured
bytes and gutter annotations, click/shift-click row selection.
- **Right sidebar** (36px): memory map visualization — heap=orange, module=purple, other=gray; red hline for current page
- **Bottom panel** (min 72px): raw hex of selected byte range

### Supported primitive types

`ValueType` (in `types.rs`) covers the full `Rekto.Serialization` primitive set plus
`text` and `array`:
`bool, byte, i8, u8, u16, i16, u32, i32, u64, i64, f32, f64, word, bytes (hex), text (utf-8), array`

`value_to_hex(value_str, data_type)` — local LE serialisation for per-field byte preview
in schema mode. Lives in `types.rs` so it's accessible to any future consumer.

`type_name()` returns the Elixir string for each type — `"bytes"` routes to
`parse_hex_pattern`, all others route to `Rekto.Serialization.to_bytes`.

### Schema decoding (Rust side)

`schema.rs` contains manual `Decoder` impls for `%Rekto.Schema.Info{}` and `%Rekto.Schema.FieldInfo{}`:
- `AtomStr` — decodes any Elixir atom to its string name
- `DataType` — decodes `:u32` → `"u32"`, `{:u8, 16}` → `"[u8; 16]"`, module atom → `"MySchema"`
- `SchemaInfoDecoded` — size + fields; passed to `set_schemas` NIF

### Build

```bash
cd neoplasm && mix deps.get && mix compile
iex -S mix   # starts Window GenServer, opens egui window
```

---

## cancex (deprecated)

**Path:** `deprecated/cancex/`

Roblox game schema modules (`Cancex.Rblx.*`) preserved for archaeological /
reference value. `Cancex.Repo` has been superseded by `Krebs.Repo`.

---

## Known Issues / Current Diagnostics

No known compile errors. Pre-existing test issues:
- `word_pattern::tests::test_unmasked_query` — UB crash unrelated to recent work (skip with `--skip word_pattern`)
- `gcc_string` test: one `todo!()` pre-existing failure

---

## Build

```bash
# All Rust crates (workspace at project root)
cargo +nightly check --workspace
cargo +nightly test --workspace -- --skip word_pattern

# cpp_scan example + C++ companion
cd libkrebs/examples/cpp_fixture && make        # builds cpp_component binary
cargo +nightly run --example cpp_scan [PID]     # attach to running cpp_component

# assault_cube example (AssaultCube hack demo)
cargo +nightly run --example assault_cube [PID]

# liblibkrebs_nif NIF (Elixir manages its own build; NIF crate is outside the workspace)
cd liblibkrebs_nif && mix deps.get && mix compile

# rekto
cd rekto && mix deps.get && mix compile

# neoplasm GUI
cd neoplasm && mix deps.get && mix compile
iex -S mix   # opens egui window on startup

# Required rekto config (in rekto/config/config.exs or consumer app)
config :rekto, :word_type, :u32   # or :u64 for 64-bit targets
```

### Cargo workspace

`Cargo.toml` at the project root is the workspace. Members: `libkrebs`, `assault_krebs`.
Shared `target/` — no duplicate compiled artifacts across crates.

The NIF crate at `krebs/native/libkrebs_nif/` is **not** a workspace member — rustler manages
its build independently. It declares `[workspace]` in its own `Cargo.toml` to opt out of the
root workspace. Its `path` dep (`../../../libkrebs`) still resolves into the workspace target.

---

## Conventions

- **Error handling:** `?` everywhere; no `unwrap` in library code (NIF resource guards use `unwrap` for mutex poisoning which is unrecoverable anyway).
- **Scan results are streamed:** `scan` NIF sends `%ScanMatch{}` messages to a recipient pid, then `{:done, count}` or `{:error, reason}`. `ScanSet` uses this protocol.
- **ErlChannel:** Rust-side bridge from scan thread → BEAM pid. Uses `std::sync::mpsc` internally with a dedicated forwarding thread.
- **`read_bytes` in Nif returns binary** via `String::from_utf8_unchecked` (Erlang binary, not charlist).
- **`write` in Nif takes `Vec<u8>`** via `binary_to_list` conversion at the Elixir boundary.
- **Logging:** Use `log::info!/debug!/warn!/error!/trace!` in Rust (libkrebs + NIF crate) and `Logger.info/debug/warning/error` in Elixir. All entries flow to `Krebs.LogBuffer` and are accessible via the `get_logs` MCP tool. No manual `println!` or `eprintln!` in library code.
- **Scan `print` param removed:** `scan_heap`, `scan_heap_until`, `scan_module`, `scan_module_until` no longer take a `print: bool` argument — timing is always logged via `log::info!`.
