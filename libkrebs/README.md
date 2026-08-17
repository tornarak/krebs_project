# libkrebs

A Cheat-Engine-esque framework for safely scanning and editing other processes' memory.

Libkrebs comes with a Windows interface and Visual C++ signatures, but the general scanning interface is platform-agnostic.

For an example of Libkrebs being used to scan an actual process, check out the [C++ integration test](https://github.com/tornarak/krebs_project/tree/master/libkrebs/examples/cpp_fixture) and the [`cpp_scan` example](https://github.com/tornarak/krebs_project/blob/master/libkrebs/examples/cpp_scan.rs).

## Installation

`libkrebs` isn't published to crates.io — pull it in as a path or git dependency:

```toml
[dependencies]
libkrebs = { path = "../libkrebs" }
```

Requires **Rust nightly** (`#![feature(portable_simd)]`, `#![feature(associated_type_defaults)]`).
A `rust-toolchain.toml` at the workspace root pins this automatically for `cargo` run
anywhere inside the repo; outside this repo, add your own or invoke `cargo +nightly`.

## Usage

```bash
# From the workspace root
cargo check --workspace
cargo test --workspace -- --skip word_pattern   # one pre-existing UB crash, unrelated

# Attach to a live process and scan its heap for a pattern (see examples/cpp_scan.rs
# for a full, runnable walkthrough — builds and attaches to its own test fixture)
cargo run --example cpp_scan
cargo run --example assault_cube [PID]
```

Reading another process's memory on Linux requires relaxing `ptrace_scope` for anything
you didn't spawn yourself — see [the root README](../README.md#getting-started) for the
one-line fix if `attach` fails with a permission error.

# Patterns

The core of libkrebs is creating, and then scanning for, *scan patterns* (also sometimes known as signatures). A scan pattern is simply a vector of bytes combined with an optional parallel "mask vector" that determines whether certain bytes should be ignored.

The simplest scan pattern is the `BytePattern`, built via the `FromBytes` trait (implemented by every pattern type — `BytePattern`, `WordPattern`, `Simd32Pattern` — so these constructors work for all three, not just `BytePattern`):

```rust
use libkrebs::pattern_scan::{BytePattern, FromBytes, ScanPattern};

let bytes = vec![0x01, 0x02, 0x03, 0x04];
let pattern: BytePattern = BytePattern::new(vec![0x01, 0x02, 0x03, 0x04], None);
assert!(pattern.matches(&bytes));
```

Libkrebs includes two other kinds of patterns: the `WordPattern` and the `Simd32Pattern`. These offer additional speed, at the cost of constrained length and content.

All patterns can be matched against byte slices:

```rust
const MEM_TEST: [u8; 0xC] = [
	0x01, 0x02, 0x03, 0x04,
	0xFF, 0x00, 0xFF, 0xFF,
	0xFF, 0xFF, 0xFF, 0xFF,
];

// `scan_buffer` takes the base address the buffer was read from (used to compute match
// addresses) and returns a vector of ScanMatch structs
assert_eq!(pattern.scan_buffer(0x0, &MEM_TEST[..]).len(), 1);

// Matches on the first 4 bytes
assert!(pattern.matches(&MEM_TEST[..4]));
```

Patterns can have masks applied to only search for certain bytes:

```rust
let cool_bytes = vec![0x01, 0xFF, 0x03, 0xFF];
// ignore the second and fourth byte
// the mask is an option, because a pattern can have no mask
let mask = Some(vec![true, false, true, false]);
// `str_mask` builds a bare mask from a string; '?' means "ignore"
assert_eq!(mask, Some(str_mask("X?X?")));
let masked_pattern = BytePattern::with_mask(&pattern, mask);
assert!(masked_pattern.matches(&cool_bytes));
```

There are many ways to construct a pattern.

```rust
// Assuming little-endian integer encoding
let pattern_as_int: u32 = 0x04030201;
assert_eq!(pattern, BytePattern::from_primitive(pattern_as_int, None));

// [0x04, 0x03, 0x02, 0x01, 0x01, 0x02, 0x03, 0x04]
let glued_together = BytePattern::from_primitive(0x01020304u32, None)
    + BytePattern::from_primitive(0x04030201u32, None);
```
