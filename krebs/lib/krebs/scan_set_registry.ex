defmodule Krebs.ScanSetRegistry do
  @moduledoc """
  ETS-backed registry of active `ScanSet` instances.

  ScanSets register automatically on startup and unregister on shutdown.
  """

  @doc false
  def start_link(_opts) do
    :ets.new(__MODULE__, [:named_table, :public])
    :ignore
  end

  @doc false
  def child_spec(opts) do
    %{
      id: __MODULE__,
      start: {__MODULE__, :start_link, [opts]},
      restart: :permanent,
      shutdown: :brutal_kill,
      type: :worker
    }
  end

  @doc "Registers a ScanSet by name. Called automatically by `ScanSet.init/1`."
  @spec register(atom, pid) :: true
  def register(name, pid), do: :ets.insert(__MODULE__, {name, pid})

  @doc "Unregisters a ScanSet by name. Called automatically by `ScanSet.terminate/2`."
  @spec unregister(atom) :: true
  def unregister(name), do: :ets.delete(__MODULE__, name)

  @doc "Gets the PID of a registered ScanSet, or `nil` if not found or dead."
  @spec get(atom) :: pid | nil
  def get(name) do
    case :ets.lookup(__MODULE__, name) do
      [{^name, pid}] -> if Process.alive?(pid), do: pid, else: nil
      [] -> nil
    end
  end

  @doc "Returns all registered ScanSets as `%{name => pid}`. Filters out dead processes."
  @spec all :: %{atom => pid}
  def all do
    :ets.tab2list(__MODULE__)
    |> Enum.filter(fn {_name, pid} -> Process.alive?(pid) end)
    |> Map.new()
  end

  @doc "Returns a list of active ScanSet names."
  @spec list_names :: [atom]
  def list_names, do: all() |> Map.keys()
end
