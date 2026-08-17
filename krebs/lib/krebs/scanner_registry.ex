defmodule Krebs.ScannerRegistry do
  @moduledoc """
  ETS-backed registry of active scanner instances.

  Scanners are registered automatically when `Krebs.Scanner.start_link/1` succeeds
  and unregistered on shutdown.
  """

  @doc """
  Starts the scanner registry.

  Returns `:ignore` since ETS table is persistent.
  """
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

  @doc """
  Registers an active scanner by name.

  Called by `Krebs.Scanner.init/1` after the scanner starts.
  """
  @spec register(atom, pid) :: true
  def register(name, pid) do
    :ets.insert(__MODULE__, {name, pid})
  end

  @doc """
  Unregisters a scanner by name.

  Called by `Krebs.Scanner.terminate/2` on shutdown.
  """
  @spec unregister(atom) :: true
  def unregister(name) do
    :ets.delete(__MODULE__, name)
  end

  @doc """
  Gets the PID of a registered scanner, or `nil` if not found or dead.
  """
  @spec get(atom) :: pid | nil
  def get(name) do
    case :ets.lookup(__MODULE__, name) do
      [{^name, pid}] -> if Process.alive?(pid), do: pid, else: nil
      [] -> nil
    end
  end

  @doc """
  Returns all registered scanners as a map: `%{name => pid, ...}`.

  Filters out dead processes.
  """
  @spec all :: %{atom => pid}
  def all do
    :ets.tab2list(__MODULE__)
    |> Enum.filter(fn {_name, pid} -> Process.alive?(pid) end)
    |> Map.new()
  end

  @doc """
  Returns a list of active scanner names.
  """
  @spec list_names :: [atom]
  def list_names do
    all() |> Map.keys()
  end
end
