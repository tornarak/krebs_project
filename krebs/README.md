# Krebs

Elixir bindings for [Libkrebs](https://github.com/tornarak/krebs_project/tree/master/libkrebs) —
a `Scanner` GenServer, `Krebs.Repo` (a [Rekto](https://github.com/tornarak/krebs_project/tree/master/rekto)
`Repo` implementation), a `HexDump` inspector, and an MCP server exposing 20+ tools for
attach/scan/query/eval against a live process's memory.

Part *dos* of my horrifying master plan.

## Installation

```elixir
def deps do
  [
    {:krebs, path: "../krebs"}   # includes rekto as a transitive path dep
  ]
end
```

Builds a Rust NIF (`native/libkrebs_nif/`) via [`rustler`](https://github.com/rusterlium/rustler)
as part of `mix compile` — you need a C toolchain (`gcc`/`clang`) and **Rust nightly**
installed. The repo's `rust-toolchain.toml` pins nightly automatically for any `cargo`/NIF
build run inside this tree.

On Linux, attaching to a process you didn't spawn yourself requires relaxing
`kernel.yama.ptrace_scope` first — see
[the root README](https://github.com/tornarak/krebs_project#getting-started) if `attach`
returns a permission error.

## Usage

```bash
mix deps.get
mix compile

# Attach on startup and drop into iex with the scanner already running
iex -S mix krebs.attach --name SomeGame   # or: --pid 1234
```

This also starts the MCP server (`Bandit` on `http://localhost:4040` by default, override
via `config :krebs, :mcp_port, N`), so an MCP-speaking client (e.g. Claude Code) can attach
to the same process at the same time as the `iex` session.

Without the mix task, start a scanner manually from any `iex -S mix` session:

```elixir
{:ok, _pid} = Krebs.Scanner.start_link(pid: 1234)
Krebs.Scanner.regions()
Krebs.Scanner.read(0x12345678, 32)
```
