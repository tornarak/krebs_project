# Krebs Project Roadmap

## Current State (Working)

### libkrebs_ex (Rust NIF) — REFACTORED
- ⚠️ Linux write path untested — `write` NIF compiled and plumbed but `/proc/{pid}/mem` writes unverified; may need ptrace

## Phase 1: Core Infrastructure Cleanup

### 1.1 Error Type Refactor — COMPLETE

- ✅ `libkrebs`: all errors consolidated in `src/error.rs` with structured hierarchy
- ✅ String errors granular: `vcpp::StringError` and `gcc::StringError` now have nested `Short(...)` / `Long(...)` sub-enums
- ✅ `CommonStringError` at `StdError` level (peer of `Vcpp`/`Gcc`) — cross-ABI buffer/IO/UTF-8 errors
- ✅ `Verifiable` impls for string types use `type Error = StdError` so `verify_shallow` holds both ABI and common errors
- ✅ `yeet_engine`: refactored to use `MemError` / `ScannerError`, multi-platform
- ✅ `libkrebs_ex`: `Enc<E>` newtype; all NIFs return structured error tuples to Elixir

### 1.2 Rekto Extensions — COMPLETE

- ✅ `Rekto.Void` — marker type for untyped pointer targets; `Rekto.Placeholder` removed
- ✅ `:void_pointer` field type — DSL shorthand for `points_to :f, Rekto.Void`
- ✅ `:vtable` field type — void_pointer + `constraints: [non_null: true]` + `memoize: true` + `in_module: true`
- ✅ `:this_pointer` field type — word-sized field; postlude injects a single `Sanity.check_this_pointers/1` check regardless of how many `:this_pointer` fields exist; skipped when `addr: nil`
- ✅ `memoize: true` field opt — postlude injects `MemoTable.check_struct_with_meta/1`; no-op when `meta.memo` is `nil`
- ✅ `in_module: true` field opt — informational; no rekto enforcement, readable via schema introspection by upstream code
- ✅ `Rekto.MemoTable` — supervision-tree-compatible GenServer (owns ETS table); **not** in rekto's own supervision tree; caller creates instances and passes ETS tid via `meta.memo`
- ✅ `Rekto.Schema.Metadata` extended with `:memo` field — rekto stays pure; memoize enforcement only fires when caller provides a table
- ✅ `Rekto.Schema.Sanity` — shared sanity check implementations for DSL-injected checks

**Design notes:**
- All new types are implemented as postlude-injected sanity checks, not runtime hooks
- `memoize` global ETS singleton rejected — rekto is a pure serialization library; memo state belongs to the process managing actual memory reads (Scanner, Repo). The `meta.memo` field threads the table reference in at deserialization time
- Upstream idiom for opts like `in_module`/`in_heap` whose enforcement is implementation-dependent:

```elixir
# Consumer project wraps rekto.Schema with its own sanity checks
defmodule MyProject.Schema do
  def module_before_heap(%{} = struct) do
    struct
    |> Helpers.get_fields_with_opt(:in_heap)
    # |> validate against Scanner's known heap regions...
    :ok
  end

  defmacro __using__(_) do
    use Rekto.Schema
    sanity MyProject.Schema, :module_before_heap
  end
end
```

We want to make the ability to operate on "arbitrary" field opts explicit in our docs as part of the API.

### 1.3 GIGA-Consolidation

- [X] `Cancex.HexDump` rolled into `LibkrebsEx`
- [X] `libkrebs_cpp_integration_test` rolled into `libkrebs/examples`
- [X] `assault_krebs` rolled into `libkrebs/examples`
- [X] `yeet_engine` rolled into `libkrebs`
- [X] `Cancex.Repo` rolled into `LibkrebsEx`

### 1.4 Nomenclature confusion

Do we want to rename some things? **YES**

I might rename the elixir prokect `LibkrebsEx` to just `Krebs`
(submodules of `Libkrebs` and `LibkrebsEx` both go here),
and the NIF to `libkrebs_nif`.
makes sense to make the CLI the eponymous project,
since it has all the core integrated functionality which the GUI will be a frontend of

**OK WE DID THAT**

### 1.5 Final passes

- [X] A lot of the `Krebs` API functions with ex. multiple default parameters on both sides are very awkward and may benefit from replacing some positional arguments with options
- [X] More pervasive logging capacity on all layers of the stack. and the logs will eventually be MCP and GUI accessible
- [X] Read over everything with fresh eyes. Better test suite, better documentation, comments. Make this suitable for developer use

## Phase 2: GUI Development

### 2.1 Technology Choice
**Priority: High** | **Decision Required**

**Options:**
- **Scenic** - Pure Elixir, desktop-native
- **Rust UI framework** - There are a lot of these...

**NOT using:** Phoenix LiveView (wrong model for local-heavy state)

### 2.2 Three-Tab Interface
**Priority: High** | **Complexity: High**

**Tab 1: Scanner** (Cheat Engine parity)
- [X] Value search (primitives + arrays + loaded schemas, also show byte pattern)
- [-] Live results table with addresses
- [X] Saved addresses list
- [X] Click address → jump to Inspector

**Tab 2: Inspector** (Novel feature)
- [X] HexDump output with visual annotations
- [X] Click byte range → type dropdown (u32, f32, CppString, etc.)
- [X] Tentative field annotations stored in UI state
- [X] "Generate Schema" → produces `.exs` file in code editor
- [X] "Validate Schema" → run Rekto parsing
- [X] "Load Schema" → schema becomes available in scanner dropdown

**Tab 3: Schemas** (Schema browser)
- [X] Load existing `.exs` files
- [X] Display field list, constraints, metadata
- [X] "Run Query" → results table
- [ ] Click result → Inspector view with struct overlay

**Tab 4: IEx / MCP integration**
- [X] The UI should be "backed" by an IEx console (w/ useful helpers and aliases loaded) and run its schemas & cheat tables in the same VM as IEx
- [X] LLM clients should (via the extant MCP) have first-class access to the same data as the UI

It is ESSENTIAL that the interface visually resemble Cheat Engine and IDA, and that a screenshot of this is the first thing people see. Most prospective employers have experience using those programs

### 2.3 Schema Generation
**Priority: Medium** | **Complexity: Medium**

Interactive hex annotation → working schema.

**Flow:**
1. User finds address via Scanner
2. Inspector shows HexDump with inferred types
3. User clicks ranges, assigns field names + types
4. System tracks selections, validates offsets
5. Generate button produces:
   ```elixir
   defmodule MyGame.PlayerHealth do
     @moduledoc """
     Auto-generated from scan at 0x12345678
     Inferred from memory layout on 2026-02-26
     """
     
     use Rekto.Schema
     
     schema do
       field :current_hp, :f32, constraints: [range: {0.0, 1000.0}]
       field :max_hp, :f32, offset: 0x04
       # ... with inference comments
     end
   end
   ```

**Features:**
- Constraint inference from user corrections
- Offset calculation from selections
- Gap detection (garbage bytes)
- Educational comments explaining choices

## Phase 3: Polish & Documentation

### 3.1 README Overhaul
**Priority: High** | **Complexity: Low**

Replace self-deprecating tone with professional confidence.

**Structure:**
```markdown
# Rekto: Query Engine for Binary Memory Analysis

[Screenshot: Query in action]

## Features
- Declarative schemas with composable constraints
- Query compilation to optimized patterns
- Automatic type inference and validation

## Quick Start
[Terminal example]

## Documentation
[Links to detailed docs]
```

**Screenshots needed:**
- Terminal session showing query → result
- HexDump with type inference
- Schema definition (vs manual approach)
- (Future) GUI mockup

**Tone:** Cheeky but professional. "Educational purposes" wink-wink.

### 3.2 Performance Instrumentation
**Priority: Low** | **Complexity: Medium**

Add Telemetry integration for profiling.

**Metrics:**
- Scan time vs pattern complexity
- NIF call overhead breakdown
- Schema validation bottlenecks
- Memory read bandwidth

**Purpose:** Demonstrate you measured before optimizing (resume value)

### 3.3 Error Recovery Documentation
**Priority: Low** | **Complexity: Low**

Document common failure modes and solutions.

**Examples:**
- Constraint violations with suggested fixes
- Sanity check failures with debugging hints
- Schema compilation errors with user-friendly explanations

## Phase 4: Advanced Features (Future)

### 4.1 Memory Modification
- [ ] `Repo.update(struct, changes)` with validation
- [ ] Freeze/unfreeze values
- [ ] Batch updates
- [ ] Verify Linux write path — test `write_bytes` on a controlled process; explore `process_vm_writev` as alternative to `/proc/mem` writes if needed

### 4.2 Scripting Interface
- [X] `.exs` cheat tables that users can write
- [X] Hot reload support
- [ ] Template library

### 4.3 Cross-Platform Support
- [ ] Linux window enumeration (X11/Wayland) — `ProcFs::list_windows` / `window_names` stub
- [ ] macOS support (maybe)

## Non-Goals

Things we explicitly decided NOT to do:

- ❌ Phoenix LiveView GUI (wrong abstraction)
- ❌ Polymorphic STL types (fighting reality)
- ❌ Over-engineered special types beyond core patterns
- ❌ Mandatory scripting for basic use
- ❌ Debugger/disassembler features (out of scope)
- ❌ Production-grade tool (portfolio piece is sufficient)

## Timeline Estimates

**Phase 1:** ~done (error refactor + libkrebs_ex refactor + schema registry + MCP complete; special types + stdlib modules remain)
**Phase 2:** 1 month (GUI scaffold + core features)
**Phase 3:** 1 week (documentation + screenshots)
**Phase 4:** Future/optional

**Total to "demo-ready":** ~6-8 weeks part-time

## Success Criteria

Portfolio-ready means:
- ✅ Clean README with screenshots
- ✅ Working GUI demonstrating workflow
- ✅ At least one complete cheat table example
- ✅ Professional error messages
- ✅ Resume entry written
- ✅ Can demo in <5 minutes to non-technical audience

Not needed for success:
- Complete feature parity with Cheat Engine
- Production stability
- Wide platform support
- Active user base
