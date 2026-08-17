defmodule Neoplasm.Window do
  @moduledoc """
  Owns the NIF lifecycle and the egui window thread.
  Single point of contact with neoplasm_nif — no other process touches the NIF directly.
  Receives GUI events from the NIF and routes them to the appropriate owner process.
  """

  use GenServer
  require Logger

  alias Neoplasm.{Helpers, Nif}

  def start_link(opts \\ []) do
    GenServer.start_link(__MODULE__, :ok, Keyword.put_new(opts, :name, __MODULE__))
  end

  def open do
    GenServer.call(__MODULE__, :open_window)
  end

  @spec query_reattach() :: pid() | nil
  def query_reattach do
    GenServer.call(__MODULE__, :query_reattach)
  end

  @impl true
  def init(:ok) do
    Nif.open_window(self())
    Nif.set_schemas(build_schema_list())
    Nif.set_word_size(Rekto.Serialization.get_word_size())
    {:ok, %{iex_open: false, iex_ref: nil}}
  end

  @impl true
  def handle_call(:open_window, _sender, state) do
    {:reply, Nif.open_window(self()), state}
  end

  def handle_call(:query_reattach, _sender, %{reattach: reattach} = state) do
    {:reply, reattach, state}
  end

  def handle_call(:query_reattach, _sender, %{} = state) do
    {:reply, nil, state}
  end

  @impl true
  def handle_info({:neoplasm, :list_processes_requested}, state) do
    processes = Krebs.Nif.list_processes()
    Nif.set_processes(processes)
    {:noreply, state}
  end

  def handle_info({:neoplasm, :attach_requested, pid}, state) do
    Neoplasm.Scanner.attach(pid)

    {:noreply, Map.put(state, :reattach, pid)}
  end

  def handle_info({:neoplasm, :detach_requested}, state) do
    Neoplasm.Scanner.detach()

    {:noreply, Map.delete(state, :reattach)}
  end

  # Raw hex bytes scan (bytes mode) — pattern is already a hex string
  def handle_info({:neoplasm, :scan_requested, pattern, region, next}, state) do
    GenServer.cast(Neoplasm.Scanner, {:scan, pattern, region, next})
    {:noreply, state}
  end

  # Bytes (hex) scan — routed through scan_primitive for consistency
  def handle_info({:neoplasm, :primitive_scan_requested, "bytes", value, region, next}, state) do
    GenServer.cast(Neoplasm.Scanner, {:scan, value, region, next})
    {:noreply, state}
  end

  # UTF-8 text scan — value string IS the byte pattern
  def handle_info({:neoplasm, :primitive_scan_requested, "text", value, region, next}, state) do
    GenServer.cast(Neoplasm.Scanner, {:scan_text, value, region, next})
    {:noreply, state}
  end

  # Array scan — type_name is "array/{elem_type}/{count}"
  def handle_info(
        {:neoplasm, :primitive_scan_requested, "array/" <> _ = type_name, value, region, next},
        state
      ) do
    GenServer.cast(Neoplasm.Scanner, {:scan_array, type_name, value, region, next})
    {:noreply, state}
  end

  # Typed primitive scan — Elixir does the serialization
  def handle_info({:neoplasm, :primitive_scan_requested, type_name, value, region, next}, state) do
    GenServer.cast(Neoplasm.Scanner, {:scan_primitive, type_name, value, region, next})
    {:noreply, state}
  end

  def handle_info({:neoplasm, :query_requested, schema, fields}, state) do
    GenServer.cast(Neoplasm.Scanner, {:query, schema, fields})
    {:noreply, state}
  end

  # Live byte preview — bytes (hex) mode: parse via MCP format for consistency
  def handle_info({:neoplasm, :primitive_preview_requested, "bytes", value}, state) do
    hex =
      case Krebs.MCP.parse_hex_pattern(value) do
        {:ok, {bytes, _mask}} ->
          bytes
          |> :erlang.binary_to_list()
          |> Enum.map(
            &(Integer.to_string(&1, 16)
              |> String.pad_leading(2, "0")
              |> String.upcase())
          )
          |> Enum.join(" ")

        _ ->
          ""
      end

    Nif.set_scan_pattern(hex)
    {:noreply, state}
  end

  # Live byte preview — UTF-8 text: show raw UTF-8 bytes
  def handle_info({:neoplasm, :primitive_preview_requested, "text", value}, state) do
    hex = bytes_to_hex_string(:binary.bin_to_list(value))
    Nif.set_scan_pattern(hex)
    {:noreply, state}
  end

  # Live byte preview — array: serialize each space-separated element
  def handle_info(
        {:neoplasm, :primitive_preview_requested, "array/" <> _ = type_name, value},
        state
      ) do
    hex =
      case Helpers.serialize_array(type_name, value) do
        {:ok, bytes} -> bytes |> :binary.bin_to_list() |> bytes_to_hex_string()
        :error -> ""
      end

    Nif.set_scan_pattern(hex)
    {:noreply, state}
  end

  # Live byte preview for a typed primitive value
  def handle_info({:neoplasm, :primitive_preview_requested, type_name, value}, state) do
    hex =
      try do
        type = String.to_existing_atom(type_name)

        case Neoplasm.Scanner.parse_value(value, type) do
          nil ->
            ""

          v ->
            v
            |> Rekto.Serialization.to_bytes(type)
            |> :erlang.binary_to_list()
            |> bytes_to_hex_string()
        end
      rescue
        _ -> ""
      end

    Nif.set_scan_pattern(hex)
    {:noreply, state}
  end

  # Live byte preview for a schema query
  def handle_info({:neoplasm, :schema_preview_requested, schema_name, fields}, state) do
    {hex, errors} =
      case Helpers.build_schema_query(schema_name, fields) do
        {:ok, query} -> {Krebs.Repo.pattern_hex_for_query(query), []}
        {:errors, rows} -> {"", rows}
        _ -> {"", []}
      end

    inferred = Helpers.infer_schema_fields(schema_name, fields)

    Nif.set_scan_pattern(hex)
    Nif.set_scan_errors(errors)
    Nif.set_inferred_fields(inferred)
    {:noreply, state}
  end

  def handle_info({:neoplasm, :iex_requested}, %{iex_open: false} = state) do
    case spawn_iex() do
      {:ok, ref} -> {:noreply, Map.merge(state, %{iex_open: true, iex_ref: ref})}
      :error -> {:noreply, state}
    end
  end

  def handle_info({:neoplasm, :iex_requested}, %{iex_open: true} = state) do
    # bring focus to IEx?
    # todo...
    {:noreply, state}
  end

  def handle_info({:DOWN, down_ref, :port, port, reason}, %{iex_open: true, iex_ref: ref} = state)
      when down_ref == ref do
    Logger.info("IEx window closed: #{inspect(reason)}")
    unless is_nil(Port.info(port)), do: Port.close(port)
    {:noreply, Map.merge(state, %{iex_open: false, iex_ref: nil})}
  end

  def handle_info({:neoplasm, :cast_preview_requested, addr, type}, state) do
    Nif.set_cast_preview(Helpers.read_value_at(addr, type))
    {:noreply, state}
  end

  def handle_info({:neoplasm, :hex_dump_requested, addr}, state) do
    word_size = Rekto.Serialization.get_word_size()

    case Krebs.HexDump.hex_dump_rows(addr, 64 * word_size) do
      {:ok, rows} ->
        Nif.set_hex_dump(rows)

      {:error, reason} ->
        Nif.set_error("Hex dump failed at 0x#{Integer.to_string(addr, 16)}: #{inspect(reason)}")
        Nif.set_hex_dump([])
    end

    {:noreply, state}
  end

  def handle_info({:neoplasm, :memory_layout_requested}, state) do
    Helpers.push_memory_layout()
    {:noreply, state}
  end

  def handle_info({:neoplasm, :watch_add_requested, addr, type}, state) do
    Neoplasm.Scanner.add_watch(addr, type)
    {:noreply, state}
  end

  def handle_info({:neoplasm, :watch_remove_requested, addr}, state) do
    Neoplasm.Scanner.remove_watch(addr)
    {:noreply, state}
  end

  def handle_info({:neoplasm, :watch_info_changed, addr, label, auto_refresh, bare_map}, state) do
    Neoplasm.Scanner.update_watch_info(addr, label, auto_refresh, bare_map)
    {:noreply, state}
  end

  # Schema builder events
  def handle_info({:neoplasm, :schema_validate_requested, module_name, fields}, state) do
    errors = Krebs.SchemaGen.validate(module_name, fields)
    Nif.set_schema_validation(errors)
    {:noreply, state}
  end

  def handle_info({:neoplasm, :schema_generate_requested, module_name, base_addr, fields}, state) do
    case Krebs.SchemaGen.generate(module_name, base_addr, fields) do
      {:ok, code} -> Nif.set_schema_generated({true, code})
      {:error, msg} -> Nif.set_schema_generated({false, msg})
    end

    {:noreply, state}
  end

  def handle_info({:neoplasm, :schema_load_requested, code}, state) do
    case Krebs.SchemaGen.load(code) do
      {:ok, msg} ->
        Nif.set_schemas(build_schema_list())
        Nif.set_schema_load_result(msg)

      {:error, msg} ->
        Nif.set_schema_load_result("Error: " <> msg)
    end

    {:noreply, state}
  end

  def handle_info({:neoplasm, :schema_save_requested, code, path}, state) do
    case Krebs.SchemaGen.save(code, path) do
      :ok -> Nif.set_schema_load_result("Saved to #{path}")
      {:error, reason} -> Nif.set_schema_load_result("Save failed: #{reason}")
    end

    {:noreply, state}
  end

  def handle_info({:neoplasm, :window_closed}, state) do
    Logger.info("Neoplasm.Window: window closed")
    {:stop, :normal, state}
  end

  def handle_info({:neoplasm, event}, state) do
    Logger.warning("Neoplasm.Window: unhandled event #{inspect(event)}")
    {:noreply, state}
  end

  def handle_info({:neoplasm, event, _}, state) do
    Logger.warning("Neoplasm.Window: unhandled event #{inspect(event)}")
    {:noreply, state}
  end

  @impl true
  def terminate(_reason, _state) do
    Nif.close_window()
  end

  defp build_schema_list do
    Rekto.SchemaRegistry.all()
    |> Enum.map(fn mod -> {mod, Rekto.Schema.get_schema(mod)} end)
  end

  defp bytes_to_hex_string(byte_list) do
    byte_list
    |> Enum.map(&(Integer.to_string(&1, 16) |> String.pad_leading(2, "0") |> String.upcase()))
    |> Enum.join(" ")
  end

  defp spawn_iex do
    node = Node.self() |> to_string()
    iex = System.find_executable("iex") || "iex"

    case find_terminal(iex, node) do
      {exe, args} ->
        port = Port.open({:spawn_executable, exe}, args: args)
        {:ok, Port.monitor(port)}

      nil ->
        Logger.error("Neoplasm.Window: no terminal emulator found to launch IEx")
        :error
    end
  end

  # Returns {exe_path, args} for the first available terminal emulator, or nil.
  # Each candidate is {name, args} where args are passed directly to the executable.
  defp find_terminal(iex, node) do
    candidates =
      case :os.type() do
        {:win32, _} ->
          # Windows Terminal opens a new tab; cmd+start opens a new window
          [
            {"wt", ["new-tab", "--", iex, "--remsh", node]},
            {"cmd", ["/c", "start", "cmd", "/k", iex, "--remsh", node]}
          ]

        {:unix, _} ->
          # Ordered by preference: distro default → common desktop → modern → minimal
          [
            {"x-terminal-emulator", ["-e", iex, "--remsh", node]},
            {"gnome-terminal", ["--", iex, "--remsh", node]},
            {"konsole", ["-e", iex, "--remsh", node]},
            {"alacritty", ["-e", iex, "--remsh", node]},
            {"kitty", [iex, "--remsh", node]},
            {"xfce4-terminal", ["-e", "#{iex} --remsh #{node}"]},
            {"xterm", ["-e", iex, "--remsh", node]}
          ]
      end

    Enum.find_value(candidates, fn {name, args} ->
      case System.find_executable(name) do
        nil -> nil
        path -> {path, args}
      end
    end)
  end
end
