defmodule Krebs.MCP do
  @moduledoc """
  MCP (Model Context Protocol) server tools for Krebs.

  Each submodule implements a single tool exposed over the Vancouver MCP adapter.
  `Krebs.MCP.tools/0` returns the list of all tool modules for registration with the router.

  Shared helpers (`resolve_server/1`, `parse_name/2`, `parse_addr/1`,
  `parse_hex_pattern/1`, `collect_scan/2`) are defined on this module and called
  from tool `run/2` callbacks.
  """

  alias Krebs.{Scanner, ScanPattern, ScanMatch}

  require Logger

  def tools do
    base = [
      __MODULE__.CalculateSum,
      __MODULE__.ListScanners,
      __MODULE__.ListProcesses,
      __MODULE__.SearchProcesses,
      __MODULE__.Attach,
      __MODULE__.Detach,
      __MODULE__.Regions,
      __MODULE__.Modules,
      __MODULE__.ReadBytes,
      __MODULE__.WriteBytes,
      __MODULE__.HexDump,
      __MODULE__.Scan,
      __MODULE__.ScanSetNew,
      __MODULE__.ScanSetScan,
      __MODULE__.ScanSetResults,
      __MODULE__.ScanSetReset,
      __MODULE__.ScanSetDelete,
      __MODULE__.ScanSetList,
      __MODULE__.ReadType,
      __MODULE__.ListSchemas,
      __MODULE__.SchemaInfo,
      __MODULE__.Eval,
      __MODULE__.GetLogs
    ]

    extra = Application.get_env(:krebs, :extra_mcp_tools, [])
    base ++ extra
  end

  # ── Shared helpers ─────────────────────────────────────────────────────────

  def resolve_server(params) do
    parse_name(Map.get(params, "scanner"), Scanner)
  end

  def parse_name(nil, default), do: default
  def parse_name("", default), do: default

  def parse_name(name, _default) when is_binary(name) do
    try do
      String.to_existing_atom(name)
    rescue
      ArgumentError -> String.to_atom(name)
    end
  end

  def parse_addr("0x" <> rest), do: String.to_integer(rest, 16)
  def parse_addr("0X" <> rest), do: String.to_integer(rest, 16)
  def parse_addr(s), do: String.to_integer(s, 16)

  def parse_hex_pattern(str) do
    tokens = String.split(String.trim(str))

    result =
      Enum.reduce_while(tokens, {[], []}, fn token, {bytes, mask} ->
        cond do
          token in ["?", "??"] ->
            {:cont, {[0 | bytes], [0 | mask]}}

          match?(<<"?", _::binary-size(1)>>, token) ->
            <<_q, byte>> = token
            {:cont, {[byte | bytes], [0xFF | mask]}}

          String.match?(token, ~r/^[0-9A-Fa-f]{1,2}$/) ->
            case Integer.parse(token, 16) do
              {byte, ""} -> {:cont, {[byte | bytes], [0xFF | mask]}}
              _ -> {:halt, {:error, {:invalid_token, token}}}
            end

          true ->
            {:halt, {:error, {:invalid_token, token}}}
        end
      end)

    case result do
      {:error, _} = err ->
        err

      {bytes, mask} ->
        bytes_bin = bytes |> Enum.reverse() |> :erlang.list_to_binary()
        mask_list = Enum.reverse(mask)

        mask_bin =
          if Enum.all?(mask_list, &(&1 == 0xFF)),
            do: nil,
            else: :erlang.list_to_binary(mask_list)

        {:ok, {bytes_bin, mask_bin}}
    end
  end

  def collect_scan(0, acc), do: Enum.reverse(acc)

  def collect_scan(limit, acc) do
    receive do
      %ScanMatch{addr: addr} ->
        collect_scan(limit - 1, [addr | acc])

      {:done, _count} ->
        Enum.reverse(acc)

      {:error, _reason} ->
        Enum.reverse(acc)
    after
      60_000 ->
        Enum.reverse(acc)
    end
  end

  # ── Tools ──────────────────────────────────────────────────────────────────

  defmodule CalculateSum do
    @moduledoc false
    use Vancouver.Tool

    def name, do: "calculate_sum"
    def description, do: "Add two numbers together"

    def input_schema do
      %{
        "type" => "object",
        "properties" => %{
          "a" => %{"type" => "number"},
          "b" => %{"type" => "number"}
        },
        "required" => ["a", "b"]
      }
    end

    def run(conn, %{"a" => a, "b" => b}), do: send_text(conn, "#{a + b}")
  end

  defmodule ListScanners do
    @moduledoc false
    use Vancouver.Tool

    def name, do: "list_scanners"

    def description,
      do: "List all active scanner instances (attached processes). Returns atom names."

    def input_schema, do: %{"type" => "object", "properties" => %{}}

    def run(conn, _params) do
      names = Krebs.ScannerRegistry.list_names() |> Enum.map(&to_string/1)
      send_json(conn, names)
    end
  end

  defmodule ListProcesses do
    @moduledoc false
    use Vancouver.Tool

    def name, do: "list_processes"
    def description, do: "List all running OS processes. Returns [[pid, exe_name], ...]."

    def input_schema, do: %{"type" => "object", "properties" => %{}}

    def run(conn, _params) do
      procs =
        Krebs.Scanner.list_processes()
        |> Enum.map(fn {pid, name} -> [pid, name] end)

      send_json(conn, procs)
    end
  end

  defmodule SearchProcesses do
    @moduledoc false
    use Vancouver.Tool

    def name, do: "search_processes"

    def description,
      do: "Find OS processes whose executable name contains a substring. Returns [pid, ...]."

    def input_schema do
      %{
        "type" => "object",
        "properties" => %{"name" => %{"type" => "string"}},
        "required" => ["name"]
      }
    end

    def run(conn, %{"name" => name}) do
      send_json(conn, Krebs.Scanner.search_processes(name))
    end
  end

  defmodule Attach do
    @moduledoc false
    use Vancouver.Tool

    def name, do: "attach"

    def description,
      do: """
      Attach a scanner to a running process. scanner_name defaults to 'Krebs.Scanner'.
      access: 'read' (default) or 'read_write'. Pass scanner_name to other tools after attaching.
      """

    def input_schema do
      %{
        "type" => "object",
        "properties" => %{
          "pid" => %{"type" => "integer"},
          "scanner_name" => %{"type" => "string"},
          "access" => %{"type" => "string"}
        },
        "required" => ["pid"]
      }
    end

    def run(conn, %{"pid" => pid} = params) do
      name = Krebs.MCP.parse_name(Map.get(params, "scanner_name"), Krebs.Scanner)
      access = if Map.get(params, "access") == "read_write", do: :read_write, else: :read

      case Krebs.Scanner.start_link(pid: pid, name: name, access: access) do
        {:ok, _} -> send_text(conn, "attached '#{name}' to PID #{pid}")
        {:error, {:already_started, _}} -> send_text(conn, "error: '#{name}' already running")
        {:error, reason} -> send_text(conn, "error: #{inspect(reason)}")
      end
    end
  end

  defmodule Detach do
    @moduledoc false
    use Vancouver.Tool

    def name, do: "detach"
    def description, do: "Stop and detach a scanner from its process."

    def input_schema do
      %{
        "type" => "object",
        "properties" => %{"scanner" => %{"type" => "string"}}
      }
    end

    def run(conn, params) do
      server = Krebs.MCP.resolve_server(params)

      case GenServer.whereis(server) do
        nil ->
          send_text(conn, "error: '#{server}' not found")

        pid ->
          GenServer.stop(pid, :normal)
          send_text(conn, "detached '#{server}'")
      end
    end
  end

  defmodule Regions do
    @moduledoc false
    use Vancouver.Tool

    def name, do: "regions"

    def description,
      do: "List memory regions of the target process: base address, range, permissions, type."

    def input_schema do
      %{
        "type" => "object",
        "properties" => %{"scanner" => %{"type" => "string"}}
      }
    end

    def run(conn, params) do
      server = Krebs.MCP.resolve_server(params)

      case Krebs.Scanner.regions(server) do
        {:ok, regions} -> send_text(conn, inspect(regions))
        {:error, reason} -> send_text(conn, "error: #{inspect(reason)}")
      end
    end
  end

  defmodule Modules do
    @moduledoc false
    use Vancouver.Tool

    def name, do: "modules"

    def description,
      do: "List loaded modules (DLLs / shared objects) with name, start, and end addresses."

    def input_schema do
      %{
        "type" => "object",
        "properties" => %{"scanner" => %{"type" => "string"}}
      }
    end

    def run(conn, params) do
      server = Krebs.MCP.resolve_server(params)

      case Krebs.Scanner.modules(server) do
        {:ok, mods} -> send_text(conn, inspect(mods))
        {:error, reason} -> send_text(conn, "error: #{inspect(reason)}")
      end
    end
  end

  defmodule ReadBytes do
    @moduledoc false
    use Vancouver.Tool

    def name, do: "read_bytes"
    def description, do: "Read raw bytes from a memory address. Returns uppercase hex string."

    def input_schema do
      %{
        "type" => "object",
        "properties" => %{
          "addr" => %{"type" => "string", "description" => "hex address, e.g. '0x7FFF1234'"},
          "size" => %{"type" => "integer"},
          "scanner" => %{"type" => "string"}
        },
        "required" => ["addr", "size"]
      }
    end

    def run(conn, %{"addr" => addr_str, "size" => size} = params) do
      server = Krebs.MCP.resolve_server(params)
      addr = Krebs.MCP.parse_addr(addr_str)

      case Krebs.Scanner.read(server, addr, size) do
        {:ok, bytes} -> send_text(conn, Base.encode16(bytes))
        {:error, reason} -> send_text(conn, "error: #{inspect(reason)}")
      end
    end
  end

  defmodule WriteBytes do
    @moduledoc false
    use Vancouver.Tool

    def name, do: "write_bytes"

    def description,
      do: "Write bytes to a memory address. data is an uppercase hex string (e.g. 'DEADBEEF')."

    def input_schema do
      %{
        "type" => "object",
        "properties" => %{
          "addr" => %{"type" => "string", "description" => "hex address, e.g. '0x7FFF1234'"},
          "data" => %{"type" => "string"},
          "scanner" => %{"type" => "string"}
        },
        "required" => ["addr", "data"]
      }
    end

    def run(conn, %{"addr" => addr_str, "data" => hex} = params) do
      addr = Krebs.MCP.parse_addr(addr_str)
      server = Krebs.MCP.resolve_server(params)

      case Base.decode16(hex, case: :mixed) do
        {:ok, bytes} ->
          case Krebs.Scanner.write(server, addr, bytes) do
            {:ok, _} -> send_text(conn, "wrote #{byte_size(bytes)} bytes")
            {:error, reason} -> send_text(conn, "error: #{inspect(reason)}")
          end

        :error ->
          send_text(conn, "error: invalid hex string")
      end
    end
  end

  defmodule HexDump do
    @moduledoc false
    use Vancouver.Tool

    def name, do: "hex_dump"

    def description,
      do: """
      Hex dump with type inference: address, hex bytes, ASCII, and inferred types per row.
      word_size controls row width in bytes: 4 for 32-bit targets (default), 8 for 64-bit.
      size must be a multiple of word_size.
      """

    def input_schema do
      %{
        "type" => "object",
        "properties" => %{
          "addr" => %{"type" => "string", "description" => "hex address, e.g. '0x7FFF1234'"},
          "size" => %{"type" => "integer"},
          "word_size" => %{"type" => "integer"},
          "scanner" => %{"type" => "string"}
        },
        "required" => ["addr", "size"]
      }
    end

    def run(conn, %{"addr" => addr_str, "size" => size} = params) do
      server = Krebs.MCP.resolve_server(params)
      addr = Krebs.MCP.parse_addr(addr_str)
      word_size = Map.get(params, "word_size") || 4
      send_text(conn, Krebs.HexDump.hex_dump_to_string(server, addr, size, word_size: word_size))
    end
  end

  defmodule Scan do
    @moduledoc false
    use Vancouver.Tool

    def name, do: "scan"

    def description,
      do: """
      Scan memory for a byte pattern. Pattern uses space-separated hex bytes with '??' as wildcard,
      e.g. '48 8B ?? FF 00'. mem_type: 'heap' (default) or 'module'.
      Returns matching addresses as hex strings. limit caps result count (default 500).
      """

    def input_schema do
      %{
        "type" => "object",
        "properties" => %{
          "pattern" => %{"type" => "string"},
          "mem_type" => %{"type" => "string"},
          "limit" => %{"type" => "integer"},
          "scanner" => %{"type" => "string"}
        },
        "required" => ["pattern"]
      }
    end

    def run(conn, %{"pattern" => pattern_str} = params) do
      server = Krebs.MCP.resolve_server(params)

      mem_type = if Map.get(params, "mem_type") == "module", do: :module, else: :heap
      limit = Map.get(params, "limit") || 500

      case Krebs.MCP.parse_hex_pattern(pattern_str) do
        {:error, reason} ->
          send_text(conn, "error: #{inspect(reason)}")

        {:ok, {bytes, mask}} ->
          pattern = ScanPattern.new(bytes, mask)
          me = self()
          Task.start(fn -> Scanner.scan(server, pattern, me, mem_type: mem_type) end)
          addrs = Krebs.MCP.collect_scan(limit, [])
          result = Enum.map(addrs, &"0x#{String.pad_leading(Integer.to_string(&1, 16), 8, "0")}")
          send_json(conn, result)
      end
    end
  end

  defmodule ListSchemas do
    @moduledoc false
    use Vancouver.Tool

    def name, do: "list_schemas"
    def description, do: "List all compiled Rekto schema modules."

    def input_schema, do: %{"type" => "object", "properties" => %{}}

    def run(conn, _params) do
      schemas = Rekto.SchemaRegistry.all() |> Enum.map(&to_string/1)
      send_json(conn, schemas)
    end
  end

  defmodule SchemaInfo do
    @moduledoc false
    use Vancouver.Tool

    def name, do: "schema_info"

    def description,
      do: """
      Get field definitions for a Rekto schema: field names, types, byte offsets, sizes,
      pointer targets, and constraints (non_null, in_heap, in_module, range).
      """

    def input_schema do
      %{
        "type" => "object",
        "properties" => %{"module" => %{"type" => "string"}},
        "required" => ["module"]
      }
    end

    def run(conn, %{"module" => mod_str}) do
      mod = Module.concat([mod_str])

      if Rekto.Schema.has_schema?(mod) do
        info = Rekto.Schema.get_schema(mod)
        constraints_map = Map.new(info.field_constraints, fn {name, c} -> {name, c} end)

        field_lines =
          Enum.map(info.fields, fn f ->
            constraint_str =
              case Map.get(constraints_map, f.name) do
                nil -> ""
                c -> " [#{inspect(c)}]"
              end

            ptr_str = if f.points_to, do: " -> #{inspect(f.points_to)}", else: ""

            "  +0x#{String.pad_leading(Integer.to_string(f.offset, 16), 3, "0")}" <>
              " [#{f.size}b] :#{f.name} :: #{inspect(f.data_type)}" <>
              ptr_str <>
              constraint_str
          end)

        send_text(
          conn,
          "Module: #{mod_str}\nSize: #{info.size} bytes\nFields:\n#{Enum.join(field_lines, "\n")}"
        )
      else
        send_text(conn, "error: #{mod_str} is not a Rekto schema")
      end
    end
  end

  defmodule ScanSetNew do
    @moduledoc false
    use Vancouver.Tool

    def name, do: "scan_set_new"

    def description,
      do: "Create a named result set for iterative scanning (Cheat Engine-style next scan)."

    def input_schema do
      %{
        "type" => "object",
        "properties" => %{"name" => %{"type" => "string"}},
        "required" => ["name"]
      }
    end

    def run(conn, %{"name" => name_str}) do
      name = Krebs.MCP.parse_name(name_str, nil)

      case Krebs.Scanner.ScanSet.start_link(name: name) do
        {:ok, _} -> send_text(conn, "created scan set '#{name_str}'")
        {:error, {:already_started, _}} -> send_text(conn, "error: '#{name_str}' already exists")
        {:error, reason} -> send_text(conn, "error: #{inspect(reason)}")
      end
    end
  end

  defmodule ScanSetScan do
    @moduledoc false
    use Vancouver.Tool

    def name, do: "scan_set_scan"

    def description,
      do: """
      Scan memory and intersect results into a named result set.
      On the first scan the set is seeded; subsequent scans narrow it down.
      Pattern uses space-separated hex bytes with '??' as wildcard.
      Blocks until the scan completes.
      """

    def input_schema do
      %{
        "type" => "object",
        "properties" => %{
          "set" => %{"type" => "string"},
          "pattern" => %{"type" => "string"},
          "mem_type" => %{"type" => "string"},
          "scanner" => %{"type" => "string"}
        },
        "required" => ["set", "pattern"]
      }
    end

    def run(conn, %{"set" => set_name, "pattern" => pattern_str} = params) do
      scanner = Krebs.MCP.resolve_server(params)
      set = Krebs.MCP.parse_name(set_name, nil)
      mem_type = if Map.get(params, "mem_type") == "module", do: :module, else: :heap

      pid = Krebs.ScanSetRegistry.get(set)

      if is_nil(pid) do
        send_text(conn, "error: no scan set '#{set_name}'")
      else
        case Krebs.MCP.parse_hex_pattern(pattern_str) do
          {:error, reason} ->
            send_text(conn, "error: #{inspect(reason)}")

          {:ok, {bytes, mask}} ->
            pattern = Krebs.ScanPattern.new(bytes, mask)
            Krebs.Scanner.ScanSet.scan(pid, scanner, pattern, mem_type: mem_type)
            Krebs.Scanner.ScanSet.await(pid)
            count = Krebs.Scanner.ScanSet.count(pid)

            send_text(
              conn,
              "scan complete — #{count} address#{if count == 1, do: "", else: "es"} remaining"
            )
        end
      end
    end
  end

  defmodule ScanSetResults do
    @moduledoc false
    use Vancouver.Tool

    def name, do: "scan_set_results"

    def description,
      do: "Return addresses in a named result set. limit caps output (default 100)."

    def input_schema do
      %{
        "type" => "object",
        "properties" => %{
          "set" => %{"type" => "string"},
          "limit" => %{"type" => "integer"}
        },
        "required" => ["set"]
      }
    end

    def run(conn, %{"set" => set_name} = params) do
      set = Krebs.MCP.parse_name(set_name, nil)
      pid = Krebs.ScanSetRegistry.get(set)

      if is_nil(pid) do
        send_text(conn, "error: no scan set '#{set_name}'")
      else
        limit = Map.get(params, "limit") || 100
        results = Krebs.Scanner.ScanSet.results(pid)
        addrs = results |> Enum.sort() |> Enum.take(limit)
        total = MapSet.size(results)

        hex_addrs = Enum.map(addrs, &"0x#{String.pad_leading(Integer.to_string(&1, 16), 8, "0")}")
        suffix = if total > limit, do: "\n(showing #{limit} of #{total})", else: ""
        send_json(conn, %{"count" => total, "addresses" => hex_addrs, "note" => suffix})
      end
    end
  end

  defmodule ScanSetReset do
    @moduledoc false
    use Vancouver.Tool

    def name, do: "scan_set_reset"
    def description, do: "Clear a named result set so the next scan seeds it fresh."

    def input_schema do
      %{
        "type" => "object",
        "properties" => %{"set" => %{"type" => "string"}},
        "required" => ["set"]
      }
    end

    def run(conn, %{"set" => set_name}) do
      set = Krebs.MCP.parse_name(set_name, nil)
      pid = Krebs.ScanSetRegistry.get(set)

      if is_nil(pid) do
        send_text(conn, "error: no scan set '#{set_name}'")
      else
        Krebs.Scanner.ScanSet.reset(pid)
        send_text(conn, "cleared '#{set_name}'")
      end
    end
  end

  defmodule ScanSetDelete do
    @moduledoc false
    use Vancouver.Tool

    def name, do: "scan_set_delete"
    def description, do: "Stop and remove a named result set."

    def input_schema do
      %{
        "type" => "object",
        "properties" => %{"set" => %{"type" => "string"}},
        "required" => ["set"]
      }
    end

    def run(conn, %{"set" => set_name}) do
      set = Krebs.MCP.parse_name(set_name, nil)
      pid = Krebs.ScanSetRegistry.get(set)

      if is_nil(pid) do
        send_text(conn, "error: no scan set '#{set_name}'")
      else
        GenServer.stop(pid, :normal)
        send_text(conn, "deleted '#{set_name}'")
      end
    end
  end

  defmodule ScanSetList do
    @moduledoc false
    use Vancouver.Tool

    def name, do: "scan_set_list"
    def description, do: "List all active named result sets with their current address counts."

    def input_schema, do: %{"type" => "object", "properties" => %{}}

    def run(conn, _params) do
      sets =
        Krebs.ScanSetRegistry.all()
        |> Enum.map(fn {name, pid} ->
          count = Krebs.Scanner.ScanSet.count(pid)
          %{"name" => to_string(name), "count" => count}
        end)

      send_json(conn, sets)
    end
  end

  defmodule ReadType do
    @moduledoc false
    use Vancouver.Tool

    def name, do: "read_type"

    def description,
      do: """
      Deserialize a Rekto type from memory. type is any Rekto datatype expression:
      a primitive (`:u32`, `:f32`, `:word`), a schema module (`Rekto.GCC.StdString`),
      an array (`{:array, :u8, 4}`), or raw bytes (`{:byte, 16}`).
      Returns inspect of the deserialized value.
      """

    def input_schema do
      %{
        "type" => "object",
        "properties" => %{
          "addr" => %{"type" => "string", "description" => "hex address, e.g. '0x7FFF1234'"},
          "type" => %{
            "type" => "string",
            "description" => "Rekto type expression, e.g. ':u32' or 'Rekto.GCC.StdString'"
          },
          "scanner" => %{"type" => "string"}
        },
        "required" => ["addr", "type"]
      }
    end

    def run(conn, %{"addr" => addr_str, "type" => type_str} = params) do
      server = Krebs.MCP.resolve_server(params)
      addr = Krebs.MCP.parse_addr(addr_str)

      try do
        {type, _} = Code.eval_string(type_str)

        case Krebs.Scanner.read_type(server, addr, type) do
          {:ok, result} -> send_text(conn, inspect(result))
          {:error, reason} -> send_text(conn, "error: #{inspect(reason)}")
        end
      rescue
        e -> send_text(conn, "error: #{Exception.message(e)}")
      end
    end
  end

  defmodule Eval do
    @moduledoc false
    use Vancouver.Tool

    def name, do: "eval"

    def description,
      do: """
      Evaluate arbitrary Elixir code. Fallback for anything not covered by other tools.
      Binding `scanner` holds the resolved server (default Krebs.Scanner).
      All Krebs.* and Rekto.* modules are available. Returns inspect(result).

      ## Schema DSL Guide

      ### Define a schema:
          defmodule MyApp.Player do
            use Rekto.Schema

            field :hp, :u32
            field :name, {:array, :u8}, size: 32
            field :weapon, MyApp.Weapon, points_to: true
          end

      ### Field types:
          :u8, :i8, :u16, :i16, :u32, :i32, :f32, :u64, :i64, :f64, :word
          {:array, elem_type}  — fixed-size array
          :word, points_to: OtherSchema  — pointer to another struct

      ### Field options:
          size: n           — byte size (required for arrays)
          memoize: true     — enforce single value via ETS
          in_heap: true     — informational: expect in heap memory
          in_module: true   — informational: expect in executable

      ### Query by field values:
          query = Rekto.Query.from(MyApp.Player, where: [hp: 100])
      """

    def input_schema do
      %{
        "type" => "object",
        "properties" => %{
          "code" => %{"type" => "string"},
          "scanner" => %{"type" => "string"}
        },
        "required" => ["code"]
      }
    end

    def run(conn, %{"code" => code} = params) do
      server = Krebs.MCP.resolve_server(params)

      try do
        Logger.info("[MCP] eval\n#{code}")
        {result, _} = Code.eval_string(code, [scanner: server], __ENV__)
        send_text(conn, inspect(result))
      rescue
        e -> send_text(conn, "error: #{Exception.message(e)}")
      end
    end
  end

  defmodule GetLogs do
    @moduledoc false
    use Vancouver.Tool

    def name, do: "get_logs"

    def description,
      do: "Fetch recent log entries from the ring buffer (Elixir + Rust). Ordered oldest-first."

    def input_schema do
      %{
        "type" => "object",
        "properties" => %{
          "limit" => %{"type" => "integer", "description" => "Max entries to return (default 50)"},
          "level" => %{
            "type" => "string",
            "description" => "Filter by level: trace/debug/info/warning/error"
          }
        }
      }
    end

    def run(conn, params) do
      limit = Map.get(params, "limit") || 50
      level_filter = Map.get(params, "level") && String.to_atom(params["level"])

      entries =
        Krebs.LogBuffer.get_recent(limit)
        |> then(fn es ->
          if level_filter, do: Enum.filter(es, &(&1.level == level_filter)), else: es
        end)

      lines =
        Enum.map(entries, fn e ->
          level = e.level |> to_string() |> String.upcase() |> String.pad_trailing(7)
          source = e.source |> to_string() |> String.pad_trailing(6)
          "#{e.ts} #{level} [#{source}] #{e.module} — #{e.message}"
        end)

      send_text(conn, Enum.join(lines, "\n"))
    end
  end
end
