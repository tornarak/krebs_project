# Neoplasm

New
Extensible
Optimized
Paradigm for
Loading,
Accessing, and
Searching
Memory

## Installation

Add `neoplasm` as a path dependency in `mix.exs` (it depends on `krebs` and `rekto`,
also as path deps within this project):

```elixir
def deps do
  [
    {:neoplasm, path: "../neoplasm"}
  ]
end
```

Builds an `egui`/`eframe` Rust NIF (`native/neoplasm_nif/`) via `rustler` as part of `mix
compile` — you need a C toolchain and **Rust nightly** installed (the repo's
`rust-toolchain.toml` pins this automatically for any build run inside the tree), plus
whatever native windowing/GL libs `eframe` needs for your platform (on Linux: X11 or
Wayland dev headers, e.g. `libxkbcommon-dev`, `libgl1-mesa-dev`).

## Usage

```bash
mix deps.get
mix compile
iex -S mix   # opens the egui window and starts Neoplasm's own MCP server
```

The GUI, `iex`, and MCP clients all drive the same underlying `Krebs.Scanner` instance —
attach from any of them and the others see it immediately. Click "IEx" in the toolbar to
open a REPL pointed at the running session without leaving the GUI.

On Linux, attaching to a process you didn't spawn yourself requires relaxing
`kernel.yama.ptrace_scope` first — see
[the root README](https://github.com/tornarak/krebs_project#getting-started) if attaching
returns a permission error.

Documentation can be generated locally with [ExDoc](https://github.com/elixir-lang/ex_doc).

