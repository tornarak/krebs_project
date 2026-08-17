defmodule Rekto.MemoTable do
  require Logger

  @moduledoc """
  GenServer that owns an ETS table for asserting field value invariance across schema instances.

  Can be added directly to a supervision tree. The ETS table is `:public` so check operations
  bypass the GenServer process (no bottleneck on hot deserialization paths). The GenServer
  just owns the table, giving it proper OTP lifecycle.

  Pass the tid from `tid/1` in `meta.memo` when deserializing:

      # In your supervisor
      children = [{Rekto.MemoTable, name: :scanner_memo}]

      # Before deserializing
      tid = Rekto.MemoTable.tid(:scanner_memo)
      MySchema.from_binary(bytes, %{addr: 0x1234, assoc_type: :pointer, memo: tid})

  When `meta.memo` is `nil` (the default), the memoize check is a no-op.
  """

  use GenServer

  # — Supervision API —

  def start_link(opts \\ []) do
    {gen_opts, _} = Keyword.split(opts, [:name])
    GenServer.start_link(__MODULE__, nil, gen_opts)
  end

  @impl GenServer
  def init(_) do
    tid = :ets.new(:memo_table, [:set, :public])
    {:ok, tid}
  end

  @impl GenServer
  def handle_call(:tid, _from, tid), do: {:reply, tid, tid}

  @doc "Returns the ETS tid for the given MemoTable server. Store this in `meta.memo`."
  def tid(server \\ __MODULE__), do: GenServer.call(server, :tid)

  # — Check API (operates directly on tid, no GenServer roundtrip) —

  @doc """
  Called as an injected sanity check. Reads the memo table tid from `struct.__meta__.memo`;
  skips when `nil`.
  """
  def check_struct_with_meta(%{__meta__: %{memo: tid}} = struct), do: check_struct(tid, struct)
  def check_struct_with_meta(%{__meta__: %{}}), do: :ok

  @doc "Checks all `memoize: true` fields of a struct against the stored values in `tid`."
  def check_struct(nil, %{__struct__: _mod}), do: :ok

  def check_struct(tid, %{__struct__: mod} = struct) do
    errors =
      mod.__schema__().fields
      |> Enum.filter(fn %{opts: opts} -> Keyword.get(opts, :memoize, false) end)
      |> Enum.flat_map(fn %{name: name} ->
        case check_and_set(tid, mod, name, Map.get(struct, name)) do
          :ok -> []
          {:error, reason} -> [{name, reason}]
        end
      end)

    if errors == [], do: :ok, else: {:error, errors}
  end

  @doc """
  Checks a single field against the stored value in `tid`.

  Stores the value on first encounter; returns an error on mismatch.
  """
  def check_and_set(tid, mod, field, value) do
    key = {mod, field}

    case :ets.lookup(tid, key) do
      [] ->
        :ets.insert(tid, {key, value})
        :ok

      [{^key, ^value}] ->
        :ok

      [{^key, existing}] ->
        Logger.warning(
          "[Rekto.MemoTable] #{inspect(mod)}.#{field} memoize conflict: expected #{inspect(existing)}, got #{inspect(value)}"
        )

        {:error, {:memoize_conflict, existing, value}}
    end
  end

  @doc "Clears all entries from the given memo table."
  def clear(tid), do: :ets.delete_all_objects(tid)

  @doc "Clears the entry for a specific field."
  def clear(tid, mod, field), do: :ets.delete(tid, {mod, field})

  @doc "Returns all stored entries as `{{mod, field}, value}` tuples."
  def get_all(tid), do: :ets.tab2list(tid)
end
