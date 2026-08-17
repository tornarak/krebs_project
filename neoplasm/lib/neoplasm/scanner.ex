defmodule Neoplasm.Scanner do
  @moduledoc """
  Owns attach state and drives Krebs.Scanner on behalf of the GUI.
  Translates GUI events into Krebs API calls and streams results to the NIF.
  """

  use GenServer
  require Logger

  alias Neoplasm.{Helpers, Nif}

  @krebs_scanner Krebs.Scanner

  def attach(pid) do
    GenServer.call(__MODULE__, {:attach, pid})
  end

  def detach do
    GenServer.call(__MODULE__, :detach)
  end

  def attached?, do: GenServer.call(__MODULE__, :attached?)

  @doc "Parse a string value into the given Rekto primitive type. Returns nil on failure."
  def parse_value(str, type), do: parse_for_type(str, type)

  def add_watch(addr, type_name),
    do: GenServer.cast(__MODULE__, {:add_watch, addr, type_name})

  def remove_watch(addr),
    do: GenServer.cast(__MODULE__, {:remove_watch, addr})

  def update_watch_info(addr, label, auto_refresh, bare_map),
    do: GenServer.cast(__MODULE__, {:update_watch_info, addr, label, auto_refresh, bare_map})

  def list_watches, do: GenServer.call(__MODULE__, :list_watches)
  def list_results, do: GenServer.call(__MODULE__, :list_results)

  defdelegate push_memory_layout, to: Helpers

  # GenServer

  def start_link(opts \\ []) do
    GenServer.start_link(__MODULE__, :ok, Keyword.put_new(opts, :name, __MODULE__))
  end

  @impl true
  def init(:ok) do
    Process.send_after(self(), :watch_tick, 1500)
    init_state = %{attached: false, scan_set: nil, watches: %{}}
    reattach = Process.whereis(Neoplasm.Window) && Neoplasm.Window.query_reattach()

    state =
      if reattach do
        Nif.reset_scan_state()

        case attach_state(init_state, reattach) do
          {:ok, _scanner, new_state} ->
            Logger.info("Neoplasm.Scanner: reattaching after crash...")
            new_state

          {:error, _} ->
            Logger.warning("Neoplasm.Scanner: failed to reattach on startup")
            init_state
        end
      else
        init_state
      end

    {:ok, state}
  end

  defp attach_state(state, pid) do
    case Krebs.Scanner.start_link(pid: pid, name: @krebs_scanner) do
      {:ok, scanner} ->
        Logger.info("Neoplasm.Scanner: attached to pid #{pid}")
        Krebs.Scanner.subscribe(@krebs_scanner)
        ss = start_scan_set(state.scan_set)
        Nif.set_attached({pid, Krebs.Scanner.executable_name(scanner)}, nil)
        {:ok, scanner, %{state | attached: true, scan_set: ss}}

      {:error, reason} = e ->
        Nif.set_attached(nil, to_string(reason))
        Logger.error("Neoplasm.Scanner: attach failed: #{inspect(reason)}")
        e
    end
  end

  @impl true
  def handle_call({:attach, pid}, _sender, state) do
    case attach_state(state, pid) do
      {:ok, scanner_pid, new_state} ->
        {:reply, {:ok, scanner_pid}, new_state}

      {:error, _reason} = e ->
        {:reply, e, state}
    end
  end

  def handle_call(:detach, _sender, state) do
    res =
      case Process.whereis(@krebs_scanner) do
        nil ->
          Nif.set_attached(nil, "Not attached in the first place...?\nSomething has gone wrong.")
          {:error, :not_attached}

        pid ->
          GenServer.stop(pid)
          Nif.reset_process_state()
          :ok
      end

    if state.scan_set, do: GenServer.stop(state.scan_set)
    {:reply, res, %{state | attached: false, scan_set: nil}}
  end

  def handle_call(:attached?, _sender, state) do
    case Process.whereis(@krebs_scanner) do
      nil ->
        {:reply, false, state}

      _pid ->
        {:reply, true, state}
    end
  end

  def handle_call(:list_watches, _sender, state) do
    entries =
      Enum.map(state.watches, fn {addr, w} ->
        %{
          addr: addr,
          label: w.label,
          type: w.type,
          auto_refresh: w.auto_refresh,
          bare_map: w.bare_map,
          value: w.last_result || {:ok, ""}
        }
      end)

    {:reply, entries, state}
  end

  def handle_call(:list_results, _sender, state) do
    addrs =
      case state.scan_set && Process.alive?(state.scan_set) do
        true -> Krebs.Scanner.ScanSet.results(state.scan_set) |> MapSet.to_list()
        _ -> []
      end

    {:reply, addrs, state}
  end

  @impl true
  def handle_cast({:query, schema_name, field_values}, state) do
    self_pid = self()
    Nif.clear_results()
    Nif.set_scan_status(:scanning)

    Task.start(fn ->
      addrs = run_schema_query(schema_name, field_values)
      send(self_pid, {:query_done, addrs})
    end)

    {:noreply, state}
  end

  def handle_cast({:scan_primitive, type_name, value_str, region, next}, state) do
    Logger.debug("Scanning for primitive: #{type_name} (#{value_str})")

    try do
      type = String.to_existing_atom(type_name)

      case parse_for_type(value_str, type) do
        nil ->
          Logger.error("Neoplasm.Scanner: could not parse #{inspect(value_str)} as #{type_name}")
          Nif.set_scan_status(:idle)
          {:noreply, state}

        value ->
          bytes = Rekto.Serialization.to_bytes(value, type)
          {:noreply, start_scan(Krebs.ScanPattern.new(bytes, nil), region, next, state)}
      end
    rescue
      _ ->
        Logger.error("Neoplasm.Scanner: unknown type #{inspect(type_name)}")
        Nif.set_scan_status(:idle)
        {:noreply, state}
    end
  end

  def handle_cast({:scan_text, value, region, next}, state) do
    Logger.debug("Scanning for text string: \"#{value}\"")

    {:noreply, start_scan(Krebs.ScanPattern.new(value, nil), region, next, state)}
  end

  def handle_cast({:scan_array, type_name, value, region, next}, state) do
    Logger.debug("Scanning for array: #{type_name} [#{value}]")

    case Helpers.serialize_array(type_name, value) do
      {:ok, bytes} ->
        {:noreply, start_scan(Krebs.ScanPattern.new(bytes, nil), region, next, state)}

      :error ->
        Logger.error(
          "Neoplasm.Scanner: invalid array scan — #{inspect(type_name)} #{inspect(value)}"
        )

        Nif.set_scan_status(:idle)
        {:noreply, state}
    end
  end

  def handle_cast({:add_watch, addr, type}, state) do
    cond do
      not Rekto.Serialization.datatype?(type) ->
        Logger.debug(
          "Attempt to watch unrecognized type #{inspect(type)} at #{Integer.to_string(addr, 16)}"
        )

        {:noreply, state}

      Map.has_key?(state.watches, addr) ->
        Logger.debug("Redundant attempt to add watch at #{Integer.to_string(addr, 16)}")
        {:noreply, state}

      true ->
        Logger.debug("Added watch at #{Integer.to_string(addr, 16)}: #{inspect(type)}")

        new_watch = %{
          label: "",
          type: type,
          auto_refresh: true,
          bare_map: false,
          last_result: nil
        }

        watches = refresh_watches(Map.put(state.watches, addr, new_watch))
        {:noreply, %{state | watches: watches}}
    end
  end

  def handle_cast({:remove_watch, addr}, state) do
    Logger.debug("Removed watch at #{Integer.to_string(addr, 16)}")
    watches = Map.delete(state.watches, addr)
    push_watch_entries(watches)
    {:noreply, %{state | watches: watches}}
  end

  def handle_cast({:update_watch_info, addr, label, auto_refresh, bare_map}, state) do
    watches =
      Map.update(state.watches, addr, nil, fn w ->
        %{w | label: label, auto_refresh: auto_refresh, bare_map: bare_map}
      end)

    watches = refresh_watches(watches)
    {:noreply, %{state | watches: watches}}
  end

  def handle_cast({:scan, pattern_str, region, next}, state) do
    case Krebs.MCP.parse_hex_pattern(pattern_str) do
      {:error, reason} ->
        Logger.error("Neoplasm.Scanner: invalid pattern #{inspect(reason)}")
        Nif.set_scan_status(:idle)
        {:noreply, state}

      {:ok, {bytes, mask}} ->
        {:noreply, start_scan(Krebs.ScanPattern.new(bytes, mask), region, next, state)}
    end
  end

  # Scan committed by ScanSet — push final result set to NIF
  @impl true
  def handle_info(:scan_committed, %{scan_set: ss} = state) do
    addrs = Krebs.Scanner.ScanSet.results(ss) |> MapSet.to_list()
    Nif.clear_results()
    Nif.push_results(addrs)
    Nif.set_scan_status({:done, length(addrs)})
    {:noreply, state}
  end

  def handle_info({:scan_error, reason}, state) do
    Logger.error("Neoplasm.Scanner: scan error #{inspect(reason)}")
    Nif.set_scan_status(:idle)
    {:noreply, state}
  end

  def handle_info({:query_done, addrs}, state) do
    Nif.push_results(addrs)
    Nif.set_scan_status({:done, length(addrs)})
    {:noreply, state}
  end

  def handle_info(:watch_tick, state) do
    watches =
      if map_size(state.watches) > 0 do
        updated =
          Map.new(state.watches, fn {addr, w} ->
            result =
              if w.auto_refresh,
                do: read_watch_value(addr, w.type, w.bare_map),
                else: w.last_result || {:ok, ""}

            {addr, %{w | last_result: result}}
          end)

        push_watch_entries(updated)
        updated
      else
        state.watches
      end

    Process.send_after(self(), :watch_tick, 1500)
    {:noreply, %{state | watches: watches}}
  end

  def handle_info({@krebs_scanner, :refresh}, state) do
    # Logger.debug("[Scanner:#{@krebs_scanner}] periodic layout refresh")
    push_memory_layout()

    {:noreply, state}
  end

  defp start_scan_set(_existing) do
    case Process.whereis(:neoplasm_scan_set) do
      nil -> :ok
      pid -> GenServer.stop(pid)
    end

    {:ok, ss} = Krebs.Scanner.ScanSet.start(name: :neoplasm_scan_set)
    ss
  end

  defp start_scan(_pattern, _mem_type, _next, %{scan_set: nil} = state) do
    Logger.error("Neoplasm.Scanner: scan attempted before attach")
    state
  end

  defp start_scan(pattern, mem_type, next, %{scan_set: ss} = state) do
    Nif.set_scan_status(:scanning)
    self_pid = self()

    if next do
      Krebs.Scanner.ScanSet.next_scan(ss, @krebs_scanner, pattern)
    else
      Krebs.Scanner.ScanSet.reset(ss)
      Nif.clear_results()
      Krebs.Scanner.ScanSet.scan(ss, @krebs_scanner, pattern, mem_type: mem_type)
    end

    Task.start(fn ->
      case Krebs.Scanner.ScanSet.await(ss, 60_000) do
        :ok -> send(self_pid, :scan_committed)
        {:error, reason} -> send(self_pid, {:scan_error, reason})
      end
    end)

    state
  end

  # Reads all watches unconditionally, updates last_result, pushes to NIF.
  # Returns the updated watches map.
  defp refresh_watches(watches) do
    updated =
      Map.new(watches, fn {addr, w} ->
        result = read_watch_value(addr, w.type, w.bare_map)
        {addr, %{w | last_result: result}}
      end)

    push_watch_entries(updated)
    updated
  end

  # Pushes the current watch state to the NIF without re-reading values.
  defp push_watch_entries(watches) do
    entries =
      Enum.map(watches, fn {addr, w} ->
        {addr, w.label, w.type, w.auto_refresh, w.bare_map, w.last_result || {:ok, ""}}
      end)

    Nif.set_watch_entries(entries)
  end

  defp read_watch_value(addr, type, bare_map) do
    Helpers.read_value_at(addr, type, bare_map: bare_map)
  end

  defp run_schema_query(schema_name, field_values) do
    case Helpers.build_schema_query(schema_name, field_values) do
      {:ok, query} ->
        Nif.set_scan_errors([])

        case Krebs.Repo.all(query, scanner: @krebs_scanner) do
          {:ok, results} -> Enum.map(results, & &1.__meta__.addr)
          _ -> []
        end

      {:errors, rows} ->
        Logger.error("Neoplasm.Scanner: invalid query for #{schema_name}: #{inspect(rows)}")
        Nif.set_scan_errors(rows)
        Nif.set_scan_status(:idle)
        []

      :no_fields ->
        []

      :unknown_schema ->
        Logger.warning("Neoplasm.Scanner: unknown schema #{schema_name}")
        []
    end
  end

  @int_types [:u8, :i8, :u16, :i16, :u32, :i32, :u64, :i64, :word, :byte]
  @float_types [:f32, :f64]

  defp parse_for_type("true", :bool), do: true
  defp parse_for_type("false", :bool), do: false
  defp parse_for_type("1", :bool), do: true
  defp parse_for_type("0", :bool), do: false
  defp parse_for_type(_, :bool), do: nil

  defp parse_for_type(str, dt) when dt in @int_types do
    case Integer.parse(str) do
      {n, ""} -> n
      _ -> nil
    end
  end

  defp parse_for_type(str, dt) when dt in @float_types do
    case Float.parse(str) do
      {f, ""} -> f
      _ -> nil
    end
  end

  defp parse_for_type(str, {:string_buffer, _n}) when is_binary(str), do: str

  defp parse_for_type(_, _), do: nil
end
