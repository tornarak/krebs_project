defmodule Krebs.Scanner do
  @moduledoc """
  GenServer wrapper around a `ProcessRef` + `ScannerRef` pair.

  Multiple named instances are supported — every public function takes an
  optional `server` argument (default: `__MODULE__`) so the bare-name API
  still works when there is only one scanner running.

  ## Quick start

      iex> Krebs.Scanner.start_link(pid: 1234)
      {:ok, #PID<...>}
      iex> Krebs.Scanner.executable_name()
      "my_process"
      iex> Krebs.Scanner.read(0x1000, 16)
      {:ok, <<...>>}

  ## Named instances

      {:ok, s} = Krebs.Scanner.start_link(pid: 1234, name: :game)
      Krebs.Scanner.executable_name(:game)
  """

  use GenServer

  require Logger

  alias Krebs.{Nif, ScanMatch, ScanPattern}

  @typedoc "A memory address in the target process."
  @type addr :: non_neg_integer

  @typedoc "The region of memory to operate on."
  @type mem_type :: :heap | :module

  @refresh_interval 60_000

  # ── Start / supervision ────────────────────────────────────────────────────

  @doc """
  Starts a scanner attached to `pid:`.

  Options:
    * `:pid` — target process ID (required)
    * `:name` — GenServer name (default: `Krebs.Scanner`)
    * `:access` — `:read` or `:read_write` (default: `:read`)
    * `:read_size` — scan buffer size in bytes (default: 1 MB)
  """
  @spec start_link(keyword) :: GenServer.on_start()
  def start_link(opts \\ []) do
    name = Keyword.get(opts, :name, __MODULE__)
    GenServer.start_link(__MODULE__, opts, name: name, timeout: 30_000)
  end

  @spec child_spec(keyword) :: Supervisor.child_spec()
  def child_spec(opts) do
    %{
      id: Keyword.get(opts, :name, __MODULE__),
      start: {__MODULE__, :start_link, [opts]},
      type: :worker
    }
  end

  # ── Static helpers (no GenServer round-trip) ───────────────────────────────

  @doc "Returns all running processes as `[{pid, exe_name}]`."
  @spec list_processes() :: [{non_neg_integer, String.t()}]
  def list_processes(), do: Nif.list_processes()

  @doc "Returns PIDs of all processes whose exe name contains `name`."
  @spec search_processes(String.t()) :: [non_neg_integer]
  def search_processes(name) when is_binary(name), do: Nif.search_processes(name)

  @doc "Returns all visible windows as `[{pid, [title, ...]}]`."
  @spec list_windows() :: [{non_neg_integer, [String.t()]}]
  def list_windows(), do: Nif.list_windows()

  @doc "Returns PIDs of processes with a window title containing `title`."
  @spec search_windows(String.t()) :: [non_neg_integer]
  def search_windows(title) when is_binary(title), do: Nif.search_windows(title)

  @doc "Returns all active scanner instances as `%{name => pid, ...}`."
  @spec list_instances() :: %{atom => pid}
  def list_instances, do: Krebs.ScannerRegistry.all()

  @doc "Returns a list of active scanner instance names."
  @spec instance_names() :: [atom]
  def instance_names, do: Krebs.ScannerRegistry.list_names()

  # ── Instance accessors ─────────────────────────────────────────────────────

  @spec pid(GenServer.server()) :: non_neg_integer
  def pid(server \\ __MODULE__), do: GenServer.call(server, :pid)

  @spec executable_name(GenServer.server()) :: String.t()
  def executable_name(server \\ __MODULE__), do: GenServer.call(server, :executable_name)

  @spec window_names(GenServer.server()) :: [String.t()]
  def window_names(server \\ __MODULE__), do: GenServer.call(server, :window_names)

  @spec access_level(GenServer.server()) :: :read | :read_write
  def access_level(server \\ __MODULE__), do: GenServer.call(server, :access_level)

  @doc "Reads `size` bytes from `addr`."
  @spec read(server :: GenServer.server(), addr(), pos_integer) ::
          {:ok, binary} | {:error, term}
  def read(server \\ __MODULE__, addr, size)

  def read(server, addr, size)
      when is_integer(addr) and addr >= 0 and is_integer(size) and size > 0,
      do: GenServer.call(server, {:read, addr, size})

  @spec read_type(GenServer.server(), addr(), Rekto.Serialization.datatype()) ::
          {:ok, any} | {:error, term}
  def read_type(server \\ __MODULE__, addr, type)
      when is_integer(addr) and addr >= 0 do
    # TODO throw in memo
    if Rekto.Serialization.datatype?(type) do
      with {:ok, bytes} <- read(server, addr, Rekto.Serialization.get_type_size!(type)) do
        if Rekto.Schema.has_schema?(type),
          do: Rekto.Serialization.from_bytes(bytes, type, %{addr: addr}),
          else: Rekto.Serialization.from_bytes(bytes, type)
      end
    else
      {:error, :no_schema}
    end
  end

  @doc "Writes `data` to `addr`."
  @spec write(server :: GenServer.server(), addr(), binary) ::
          {:ok, non_neg_integer} | {:error, term}
  def write(server \\ __MODULE__, addr, data)

  def write(server, addr, data)
      when is_integer(addr) and addr >= 0 and is_binary(data),
      do: GenServer.call(server, {:write, addr, :erlang.binary_to_list(data)})

  @doc "Returns memory regions for the target process."
  @spec regions(GenServer.server()) :: {:ok, [Krebs.Region.t()]} | {:error, term}
  def regions(server \\ __MODULE__), do: GenServer.call(server, :regions)

  @doc "Returns loaded modules for the target process."
  @spec modules(GenServer.server()) :: {:ok, [Krebs.Module.t()]} | {:error, term}
  def modules(server \\ __MODULE__), do: GenServer.call(server, :modules)

  @doc """
  Scans memory for `pattern`, streaming `ScanMatch` structs to `recipient`.

  Sends `{:done, count}` when the scan is complete or `{:error, reason}` on failure.

  Options:
    * `:mem_type` — `:heap` (default) or `:module`
  """
  @spec scan(ScanPattern.t(), pid, keyword) :: :ok
  @spec scan(GenServer.server(), ScanPattern.t(), pid, keyword) :: :ok

  def scan(%ScanPattern{} = pattern, recipient) when is_pid(recipient),
    do: do_scan(__MODULE__, pattern, recipient, [])

  def scan(%ScanPattern{} = pattern, recipient, opts)
      when is_pid(recipient) and is_list(opts),
      do: do_scan(__MODULE__, pattern, recipient, opts)

  def scan(server, %ScanPattern{} = pattern, recipient) when is_pid(recipient),
    do: do_scan(server, pattern, recipient, [])

  def scan(server, %ScanPattern{} = pattern, recipient, opts)
      when is_pid(recipient) and is_list(opts),
      do: do_scan(server, pattern, recipient, opts)

  defp do_scan(server, pattern, recipient, opts) do
    mem_type = opts[:mem_type] || :heap
    GenServer.cast(server, {:scan, pattern, recipient, mem_type})
  end

  defp relay_loop(pid) do
    receive do
      %ScanMatch{} = match ->
        send(pid, {:match, self(), match})
        relay_loop(pid)

      {:done, count} ->
        send(pid, {:done, self(), count})

      {:error, reason} ->
        send(pid, {:error, self(), reason})
    end
  end

  @doc """
  Like `scan/4` but returns a lazy `Stream` of `ScanMatch` structs.

  Options:
    * `:mem_type` — `:heap` (default) or `:module`
  """
  @spec scan_stream(ScanPattern.t()) :: Enumerable.t()
  @spec scan_stream(ScanPattern.t(), keyword) :: Enumerable.t()
  @spec scan_stream(GenServer.server(), ScanPattern.t()) :: Enumerable.t()
  @spec scan_stream(GenServer.server(), ScanPattern.t(), keyword) :: Enumerable.t()

  def scan_stream(%ScanPattern{} = pattern),
    do: do_scan_stream(__MODULE__, pattern, [])

  def scan_stream(%ScanPattern{} = pattern, opts) when is_list(opts),
    do: do_scan_stream(__MODULE__, pattern, opts)

  def scan_stream(server, %ScanPattern{} = pattern),
    do: do_scan_stream(server, pattern, [])

  def scan_stream(server, %ScanPattern{} = pattern, opts) when is_list(opts),
    do: do_scan_stream(server, pattern, opts)

  defp do_scan_stream(server, pattern, opts) do
    mem_type = opts[:mem_type] || :heap
    # A relay task is used to isolate the caller's mailbox: the NIF sends
    # ScanMatch messages to the relay pid; the stream then receives from the
    # relay via tagged messages, so unrelated messages in the caller's mailbox
    # are not consumed.
    Stream.resource(
      fn ->
        case GenServer.whereis(server) do
          nil ->
            {:error, {:no_scanner, server}}

          _pid ->
            recv_pid = self()
            %Task{pid: relay_pid} = task = Task.async(fn -> relay_loop(recv_pid) end)
            GenServer.cast(server, {:scan, pattern, relay_pid, mem_type})
            {:running, task}
        end
      end,
      fn
        {:error, _} = err ->
          {[err], :halted}

        :halted ->
          {:halt, :halted}

        {:running, %Task{pid: pid} = task} ->
          receive do
            {:match, ^pid, match} -> {[match], {:running, task}}
            {:done, ^pid, _count} -> {:halt, {:done, task}}
            {:error, ^pid, reason} -> {[{:error, reason}], {:halting, task}}
          end

        {:halting, task} ->
          {:halt, {:done, task}}
      end,
      fn
        :halted -> :ok
        {:error, _} -> :ok
        {_, task} -> Task.await(task)
      end
    )
  end

  @doc "Returns whether `addr` is in the given `mem_type` region."
  @spec in_memory?(GenServer.server(), addr(), mem_type) :: {:ok, boolean} | {:error, term}
  def in_memory?(server \\ __MODULE__, addr, mem_type)

  def in_memory?(server, addr, mem_type)
      when is_integer(addr) and addr >= 0 and mem_type in [:heap, :module],
      do: GenServer.call(server, {:in_memory?, addr, mem_type})

  @doc """
  Triggers an immediate memory layout refresh.

  The layout is also refreshed automatically every 60 seconds.
  """
  @spec refresh_layout(GenServer.server()) :: :ok
  def refresh_layout(server \\ __MODULE__),
    do: GenServer.cast(server, :refresh_layout)

  @doc """
  Subscribe to receive {name, :refresh} tuples
  when the layout is refreshed.
  """
  @spec subscribe(GenServer.server(), pid :: Process.dest()) ::
          :ok | {:error, :already_subscribed}
  def subscribe(server \\ __MODULE__, pid \\ self()),
    do: GenServer.call(server, {:subscribe, pid})

  @doc """
  Unsubscribe from layout updates
  """
  @spec unsubscribe(GenServer.server(), pid :: Process.dest()) :: :ok | {:error, :not_subscribed}
  def unsubscribe(server \\ __MODULE__, pid \\ self()),
    do: GenServer.call(server, {:unsubscribe, pid})

  # ── GenServer callbacks ────────────────────────────────────────────────────

  @impl GenServer
  def init(opts) do
    name = Keyword.get(opts, :name, __MODULE__)
    target_pid = Keyword.fetch!(opts, :pid)
    read_size = Keyword.get(opts, :read_size, 1 * 1024 * 1024)
    access = Keyword.get(opts, :access, :read)

    with {:ok, proc} <- Nif.attach(target_pid, access),
         {:ok, scanner} <- Nif.scanner_new(proc, read_size) do
      Logger.info("[Scanner:#{name}] registered")
      Krebs.ScannerRegistry.register(name, self())
      Process.send_after(self(), :refresh, @refresh_interval)
      {:ok, %{proc: proc, scanner: scanner, name: name, on_refresh: MapSet.new()}}
    else
      {:error, reason} -> {:stop, reason}
    end
  end

  @impl GenServer
  def handle_call(:pid, _from, %{proc: proc} = state),
    do: {:reply, Nif.process_pid(proc), state}

  def handle_call(:executable_name, _from, %{proc: proc} = state),
    do: {:reply, Nif.executable_name(proc), state}

  def handle_call(:window_names, _from, %{proc: proc} = state),
    do: {:reply, Nif.window_names(proc), state}

  def handle_call(:access_level, _from, %{proc: proc} = state),
    do: {:reply, Nif.access_level(proc), state}

  def handle_call({:read, addr, size}, _from, %{proc: proc} = state),
    do: {:reply, Nif.read(proc, addr, size), state}

  def handle_call({:write, addr, data}, _from, %{proc: proc} = state),
    do: {:reply, Nif.write(proc, addr, data), state}

  def handle_call(:regions, _from, %{proc: proc} = state),
    do: {:reply, Nif.regions(proc), state}

  def handle_call(:modules, _from, %{proc: proc} = state),
    do: {:reply, Nif.modules(proc), state}

  def handle_call({:in_memory?, addr, mem_type}, _from, %{scanner: scanner} = state),
    do: {:reply, Nif.in_memory?(scanner, addr, mem_type), state}

  def handle_call({:subscribe, from}, _from, %{on_refresh: on_refresh} = state) do
    unless MapSet.member?(on_refresh, from) do
      {:reply, :ok, %{state | on_refresh: MapSet.put(on_refresh, from)}}
    else
      {:reply, {:error, :already_subscribed}, state}
    end
  end

  def handle_call({:unsubscribe, from}, _from, %{on_refresh: on_refresh} = state) do
    if MapSet.member?(on_refresh, from) do
      {:reply, :ok, %{state | on_refresh: MapSet.delete(on_refresh, from)}}
    else
      {:reply, {:error, :not_subscribed}, state}
    end
  end

  @impl GenServer
  def handle_cast({:scan, pattern, recipient, mem_type}, %{scanner: scanner, name: name} = state) do
    Logger.debug("[Scanner:#{name}] scan mem_type=#{mem_type}")
    result = Nif.scan(scanner, ScanPattern.to_ffi(pattern), recipient, mem_type)

    case result do
      {:ok, count} -> send(recipient, {:done, count})
      {:error, _} = err -> send(recipient, err)
    end

    {:noreply, state}
  end

  def handle_cast(:refresh_layout, %{} = state) do
    {:noreply, refresh_state(state)}
  end

  @impl GenServer
  def handle_info(:refresh, %{} = state) do
    s = refresh_state(state)
    Process.send_after(self(), :refresh, @refresh_interval)
    {:noreply, s}
  end

  @impl GenServer
  def terminate(reason, %{proc: proc, name: name}) do
    Logger.info("[Scanner:#{name}] terminating reason=#{inspect(reason)}")
    Krebs.ScannerRegistry.unregister(name)
    Nif.close(proc)
  end

  defp refresh_state(%{scanner: scanner, name: name, on_refresh: on_refresh} = state) do
    Nif.refresh_layout(scanner)

    new_on_refresh =
      MapSet.filter(
        on_refresh,
        fn proc ->
          Process.alive?(proc) && send(proc, {name, :refresh})
        end
      )

    %{state | on_refresh: new_on_refresh}
  end
end
