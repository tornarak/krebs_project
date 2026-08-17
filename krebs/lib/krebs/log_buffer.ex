defmodule Krebs.LogBuffer do
  use GenServer

  @max_entries 500

  @typedoc "A single log entry."
  @type log_entry :: %{
          ts: String.t(),
          level: :error | :warning | :info | :debug | :trace,
          source: :elixir | :rust,
          module: String.t(),
          message: String.t()
        }

  @moduledoc """
  Ring buffer for log entries from both Elixir Logger and the Rust NIF layer.

  Stores the last #{@max_entries} entries in an ETS table. Entries are
  inserted asynchronously via `GenServer.cast` — no caller blocks.

  Receives `{:krebs_log, level, target, message}` messages from the Rust
  NIF logger bridge (see `Krebs.Nif.nif_log_init/1`).

  The Elixir side feeds through `Krebs.LogBuffer.Backend` (Logger backend).
  """

  # ── Public API ─────────────────────────────────────────────────────────────

  @spec start_link(keyword) :: GenServer.on_start()
  def start_link(opts \\ []) do
    name = Keyword.get(opts, :name, __MODULE__)
    GenServer.start_link(__MODULE__, :ok, name: name)
  end

  @doc "Async-insert a log entry into the ring buffer."
  @spec insert(log_entry, GenServer.server()) :: :ok
  def insert(entry, server \\ __MODULE__) when is_map(entry) do
    GenServer.cast(server, {:insert, entry})
  end

  @doc "Return the most recent `n` entries (oldest first), capped at #{@max_entries}."
  @spec get_recent(pos_integer, GenServer.server()) :: [log_entry]
  def get_recent(n \\ 50, server \\ __MODULE__) do
    GenServer.call(server, {:get_recent, n})
  end

  @doc "Clear all stored entries."
  @spec clear(GenServer.server()) :: :ok
  def clear(server \\ __MODULE__) do
    GenServer.cast(server, :clear)
  end

  # ── GenServer callbacks ────────────────────────────────────────────────────

  @impl GenServer
  def init(:ok) do
    # Use an anonymous table so multiple instances don't conflict.
    table = :ets.new(:krebs_log_buffer, [:ordered_set, :protected])
    {:ok, %{table: table, counter: 0}}
  end

  @impl GenServer
  def handle_cast({:insert, entry}, %{table: table, counter: counter} = state) do
    n = counter + 1
    :ets.insert(table, {n, entry})

    # Trim oldest entry once the buffer is full
    if n > @max_entries do
      :ets.delete(table, n - @max_entries)
    end

    {:noreply, %{state | counter: n}}
  end

  def handle_cast(:clear, %{table: table} = state) do
    :ets.delete_all_objects(table)
    {:noreply, %{state | counter: 0}}
  end

  @impl GenServer
  def handle_call({:get_recent, n}, _from, %{table: table, counter: counter} = state) do
    start_key = max(1, counter - n + 1)
    entries = :ets.select(table, [{{:"$1", :"$2"}, [{:>=, :"$1", start_key}], [:"$2"]}])
    {:reply, entries, state}
  end

  # ── Rust NIF bridge ────────────────────────────────────────────────────────

  # Receives {:krebs_log, level, target, message} from nif_logger.rs.
  @impl GenServer
  def handle_info({:krebs_log, level, target, message}, state) do
    entry = %{
      ts: DateTime.utc_now() |> DateTime.to_iso8601(),
      level: level,
      source: :rust,
      module: target,
      message: message
    }

    handle_cast({:insert, entry}, state)
  end

  def handle_info(_, state), do: {:noreply, state}
end
