defmodule Krebs.Scanner.ScanSet do
  @moduledoc """
  Iterative memory search, à la Cheat Engine's "Next Scan".

  Acts as the scan recipient: start a `ScanSet`, pass its pid to repeated
  `Scanner.scan/4` calls, and after each scan the result set is intersected
  with the previous one, narrowing down candidates.

      {:ok, ss} = ScanSet.start_link()

      # First scan — seeds the set with all matching addresses
      ScanSet.scan(ss, Krebs.Scanner, ScanPattern.new(<<100>>, nil), :heap)
      ScanSet.await(ss)
      ScanSet.count(ss)  # => maybe 400

      # Take damage, health is now 75 — second scan intersects
      ScanSet.scan(ss, Krebs.Scanner, ScanPattern.new(<<75>>, nil), :heap)
      ScanSet.await(ss)
      ScanSet.count(ss)  # => maybe 3
  """

  use GenServer

  import Bitwise, only: [band: 2]

  require Logger

  alias Krebs.{Scanner, ScanMatch, ScanPattern}

  @type mem_type :: :heap | :module

  # ── Public API ─────────────────────────────────────────────────────────────

  @spec start_link(keyword) :: GenServer.on_start()
  def start_link(opts \\ []) do
    name = Keyword.fetch!(opts, :name)
    GenServer.start_link(__MODULE__, name, opts)
  end

  @spec start(keyword) :: GenServer.on_start()
  def start(opts \\ []) do
    name = Keyword.fetch!(opts, :name)
    GenServer.start(__MODULE__, name, opts)
  end

  @doc """
  Trigger a scan. Results stream to this ScanSet; on completion the batch is
  intersected with the stored set (or seeds it on the first scan).

  Returns immediately — use `await/1` or `count/1` to know when it's done.
  """
  @spec scan(pid, GenServer.server(), ScanPattern.t(), keyword) :: :ok
  def scan(pid, scanner, %ScanPattern{} = pattern, opts \\ []) do
    GenServer.cast(pid, {:scan, scanner, pattern, opts[:mem_type] || :heap})
  end

  @doc "Block until the in-progress scan (if any) is committed to the set."
  @spec await(pid, timeout) :: :ok | {:error, term}
  def await(pid, timeout \\ :infinity) do
    GenServer.call(pid, :await, timeout)
  end

  @doc "Return the current result set as a `MapSet` of addresses."
  @spec results(pid) :: MapSet.t()
  def results(pid), do: GenServer.call(pid, :results)

  @doc "Number of addresses currently in the set."
  @spec count(pid) :: non_neg_integer
  def count(pid), do: GenServer.call(pid, :count)

  @doc """
  Re-read bytes at each address in the current set and keep only those that still match
  `pattern`. Unlike `scan/4`, this does NOT do a full memory scan — it reads only the
  known candidate addresses. Returns immediately; use `await/1` to wait for completion.

  No-ops with a warning if called before the first `scan/4` (no current set).
  """
  @spec next_scan(pid, GenServer.server(), ScanPattern.t()) :: :ok
  def next_scan(pid, scanner, %ScanPattern{} = pattern) do
    GenServer.cast(pid, {:next_scan, scanner, pattern})
  end

  @doc "Clear all results and start fresh."
  @spec reset(pid) :: :ok
  def reset(pid), do: GenServer.call(pid, :reset)

  # ── GenServer callbacks ────────────────────────────────────────────────────

  @impl GenServer
  def init(name) do
    Krebs.ScanSetRegistry.register(name, self())
    {:ok, %{name: name, current: nil, pending: MapSet.new(), scanning: false, waiters: []}}
  end

  @impl GenServer
  def terminate(_reason, %{name: name}) do
    Krebs.ScanSetRegistry.unregister(name)
  end

  # Accumulate matches into pending
  @impl GenServer
  def handle_info(%ScanMatch{addr: addr}, state) do
    {:noreply, %{state | pending: MapSet.put(state.pending, addr)}}
  end

  # Scan complete: intersect (or seed) then notify waiters
  @impl GenServer
  def handle_info({:done, _count}, state) do
    current =
      case state.current do
        nil -> state.pending
        existing -> MapSet.intersection(existing, state.pending)
      end

    Logger.info("[ScanSet:#{state.name}] scan done — set size=#{MapSet.size(current)}")
    Enum.each(state.waiters, &GenServer.reply(&1, :ok))

    {:noreply, %{state | current: current, pending: MapSet.new(), scanning: false, waiters: []}}
  end

  # Scan error: discard pending batch, don't touch current set, propagate error to waiters
  @impl GenServer
  def handle_info({:error, reason}, state) do
    Logger.warning("[ScanSet:#{state.name}] scan error: #{inspect(reason)}")
    Enum.each(state.waiters, &GenServer.reply(&1, {:error, reason}))
    {:noreply, %{state | pending: MapSet.new(), scanning: false, waiters: []}}
  end

  def handle_info({:next_scan_done, matching}, state) do
    Logger.info("[ScanSet:#{state.name}] next scan done — set size=#{MapSet.size(matching)}")
    Enum.each(state.waiters, &GenServer.reply(&1, :ok))
    {:noreply, %{state | current: matching, scanning: false, waiters: []}}
  end

  @impl GenServer
  def handle_call(:await, _from, %{scanning: false} = state) do
    {:reply, :ok, state}
  end

  def handle_call(:await, from, state) do
    {:noreply, %{state | waiters: [from | state.waiters]}}
  end

  def handle_call(:results, _from, state) do
    {:reply, state.current || MapSet.new(), state}
  end

  def handle_call(:count, _from, state) do
    {:reply, MapSet.size(state.current || MapSet.new()), state}
  end

  def handle_call(:reset, _from, state) do
    {:reply, :ok, %{state | current: nil, pending: MapSet.new(), scanning: false, waiters: []}}
  end

  @impl GenServer
  def handle_cast({:scan, scanner, pattern, mem_type}, %{scanning: false} = state) do
    pid = self()
    Task.start(fn -> Scanner.scan(scanner, pattern, pid, mem_type: mem_type) end)
    {:noreply, %{state | scanning: true, pending: MapSet.new()}}
  end

  def handle_cast({:scan, _scanner, _pattern, _mem_type}, %{name: name, scanning: true} = state) do
    Logger.warning(
      "[ScanSet:#{name}] double-scan discarded — await the current scan before starting another"
    )

    {:noreply, state}
  end

  def handle_cast({:next_scan, scanner, pattern}, %{scanning: false, current: current} = state)
      when not is_nil(current) do
    pid = self()
    size = byte_size(pattern.bytes)

    Task.start(fn ->
      matching =
        current
        |> Enum.filter(fn addr ->
          case Scanner.read(scanner, addr, size) do
            {:ok, data} -> matches_pattern?(data, pattern)
            {:error, _} -> false
          end
        end)
        |> MapSet.new()

      send(pid, {:next_scan_done, matching})
    end)

    {:noreply, %{state | scanning: true}}
  end

  def handle_cast({:next_scan, _scanner, _pattern}, %{name: name, current: nil} = state) do
    Logger.warning("[ScanSet:#{name}] next_scan called before initial scan — ignoring")
    {:noreply, state}
  end

  def handle_cast({:next_scan, _scanner, _pattern}, %{name: name, scanning: true} = state) do
    Logger.warning(
      "[ScanSet:#{name}] next_scan discarded — await the current scan before starting another"
    )

    {:noreply, state}
  end

  defp matches_pattern?(data, %ScanPattern{bytes: bytes, mask: nil}), do: data == bytes

  defp matches_pattern?(data, %ScanPattern{bytes: bytes, mask: mask}) do
    apply_mask = fn bin ->
      Enum.zip(:binary.bin_to_list(bin), :binary.bin_to_list(mask))
      |> Enum.map(fn {b, m} -> band(b, m) end)
      |> :binary.list_to_bin()
    end

    apply_mask.(data) == apply_mask.(bytes)
  end
end
