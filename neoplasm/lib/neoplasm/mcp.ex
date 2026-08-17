defmodule Neoplasm.MCP do
  @moduledoc """
  MCP tools for neoplasm — extends krebs's MCP server with GUI-shared state access.
  Registered via `:extra_mcp_tools` config so they appear alongside krebs's tools.
  """

  # ── Watch tools ─────────────────────────────────────────────────────────────

  defmodule ListWatches do
    @moduledoc false
    use Vancouver.Tool

    def name, do: "list_watches"

    def description,
      do:
        "List all watched memory addresses. Returns addr, label, type, value for each watch. " <>
          "Watches are shared with the GUI — changes from either side are visible to both."

    def input_schema, do: %{"type" => "object", "properties" => %{}}

    def run(conn, _params) do
      entries = Neoplasm.Scanner.list_watches()

      formatted =
        Enum.map(entries, fn w ->
          value_str =
            case w.value do
              {:ok, v} -> v
              {:error, e} -> "error: #{e}"
            end

          %{
            addr: "0x" <> String.upcase(Integer.to_string(w.addr, 16)),
            label: w.label,
            type: inspect(w.type),
            auto_refresh: w.auto_refresh,
            bare_map: w.bare_map,
            value: value_str
          }
        end)

      send_json(conn, formatted)
    end
  end

  defmodule AddWatch do
    @moduledoc false
    use Vancouver.Tool

    def name, do: "add_watch"

    def description,
      do:
        "Watch a memory address with a given type. The value is read periodically and " <>
          "visible in both the MCP (via list_watches) and the GUI hex dump tab. " <>
          "Type is a Rekto type expression: :u32, :f32, :word, {:string_buffer, 32}, etc."

    def input_schema do
      %{
        "type" => "object",
        "properties" => %{
          "addr" => %{"type" => "string", "description" => "hex address, e.g. '0x7FFF1234'"},
          "type" => %{
            "type" => "string",
            "description" => "Rekto type expression, e.g. ':u32', ':f32', '{:string_buffer, 32}'"
          }
        },
        "required" => ["addr", "type"]
      }
    end

    def run(conn, %{"addr" => addr_str, "type" => type_str}) do
      addr = Krebs.MCP.parse_addr(addr_str)

      try do
        {type, _} = Code.eval_string(type_str)
        Neoplasm.Scanner.add_watch(addr, type)
        send_text(conn, "ok")
      rescue
        e -> send_text(conn, "error: #{Exception.message(e)}")
      end
    end
  end

  defmodule RemoveWatch do
    @moduledoc false
    use Vancouver.Tool

    def name, do: "remove_watch"
    def description, do: "Stop watching a memory address. Removes it from both MCP and GUI."

    def input_schema do
      %{
        "type" => "object",
        "properties" => %{
          "addr" => %{"type" => "string", "description" => "hex address, e.g. '0x7FFF1234'"}
        },
        "required" => ["addr"]
      }
    end

    def run(conn, %{"addr" => addr_str}) do
      addr = Krebs.MCP.parse_addr(addr_str)
      Neoplasm.Scanner.remove_watch(addr)
      send_text(conn, "ok")
    end
  end

  defmodule UpdateWatch do
    @moduledoc false
    use Vancouver.Tool

    def name, do: "update_watch"

    def description,
      do:
        "Update a watch's metadata: label (display name), auto_refresh (periodic re-read), " <>
          "bare_map (strip __meta__ from schema struct display)."

    def input_schema do
      %{
        "type" => "object",
        "properties" => %{
          "addr" => %{"type" => "string", "description" => "hex address of existing watch"},
          "label" => %{"type" => "string", "description" => "display label"},
          "auto_refresh" => %{
            "type" => "boolean",
            "description" => "auto-refresh value (default true)"
          },
          "bare_map" => %{"type" => "boolean", "description" => "strip __meta__ (default false)"}
        },
        "required" => ["addr"]
      }
    end

    def run(conn, %{"addr" => addr_str} = params) do
      addr = Krebs.MCP.parse_addr(addr_str)
      label = Map.get(params, "label", "")
      auto_refresh = Map.get(params, "auto_refresh", true)
      bare_map = Map.get(params, "bare_map", false)
      Neoplasm.Scanner.update_watch_info(addr, label, auto_refresh, bare_map)
      send_text(conn, "ok")
    end
  end

  # ── Results tool ────────────────────────────────────────────────────────────

  defmodule ListResults do
    @moduledoc false
    use Vancouver.Tool

    def name, do: "list_results"

    def description,
      do:
        "List addresses from the current scan result set. These are the same results " <>
          "visible in the GUI scanner tab. Returns hex address strings."

    def input_schema, do: %{"type" => "object", "properties" => %{}}

    def run(conn, _params) do
      addrs =
        Neoplasm.Scanner.list_results()
        |> Enum.map(&("0x" <> String.upcase(Integer.to_string(&1, 16))))

      send_json(conn, addrs)
    end
  end

  # ── Schema generation tools ─────────────────────────────────────────────────

  defmodule LoadSchema do
    @moduledoc false
    use Vancouver.Tool

    def name, do: "load_schema"

    def description,
      do:
        "Compile Elixir schema source code into the running VM and register it with " <>
          "the schema registry. After loading, the schema is available to list_schemas, " <>
          "schema_info, read_type, and query operations. Also updates the GUI if running."

    def input_schema do
      %{
        "type" => "object",
        "properties" => %{
          "code" => %{
            "type" => "string",
            "description" => "Elixir source code containing a defmodule with `use Rekto.Schema`"
          }
        },
        "required" => ["code"]
      }
    end

    def run(conn, %{"code" => code}) do
      case Krebs.SchemaGen.load(code) do
        {:ok, msg} ->
          # Refresh GUI schema list if neoplasm is running
          if Code.ensure_loaded?(Neoplasm.Nif) and Process.whereis(Neoplasm.Window) do
            schemas =
              Rekto.SchemaRegistry.all()
              |> Enum.map(fn mod -> {mod, Rekto.Schema.get_schema(mod)} end)

            Neoplasm.Nif.set_schemas(schemas)
          end

          send_text(conn, msg)

        {:error, msg} ->
          send_text(conn, "error: #{msg}")
      end
    end
  end
end
