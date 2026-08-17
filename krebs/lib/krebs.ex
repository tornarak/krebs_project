defmodule Krebs do
  @moduledoc """
  Elixir wrapper for libkrebs.
  """

  alias Krebs.Nif

  @typedoc "A platform process ID."
  @type process_id :: pos_integer

  @doc """
  Returns the PID of the first process whose window title contains `name`, or `nil`.

  On Linux, window enumeration is not yet implemented and this always returns `nil`.
  """
  @spec get_pid(String.t()) :: process_id | nil
  def get_pid(name) when is_binary(name) do
    case Nif.search_windows(name) do
      [pid | _] ->
        pid

      [] ->
        case Nif.search_processes(name) do
          [pid | _] -> pid
          [] -> nil
        end
    end
  end

  @doc """
  Returns all active scanner instances as a map: `%{name => pid, ...}`.

  Filters out dead processes.
  """
  @spec list_scanners :: %{atom => pid}
  def list_scanners do
    Krebs.ScannerRegistry.all()
  end

  @doc """
  Returns a list of active scanner instance names.
  """
  @spec scanner_names :: [atom]
  def scanner_names do
    Krebs.ScannerRegistry.list_names()
  end

  @doc """
  Gets the PID of a registered scanner, or `nil` if not found or dead.
  """
  @spec get_scanner(atom) :: pid | nil
  def get_scanner(name) do
    Krebs.ScannerRegistry.get(name)
  end
end
