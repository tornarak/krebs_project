defmodule Krebs.Nif do
  @moduledoc false
  # Raw NIF stubs — use Krebs.Scanner for the ergonomic API.
  # Calling these directly with bad arguments can crash the BEAM.

  use Rustler, otp_app: :krebs, crate: "libkrebs_nif"

  # ── Patterns ────────────────────────────────────────────────────────────────

  @spec new_scan_pattern(list, list | nil) :: Krebs.ScanPattern.t()
  def new_scan_pattern(_bytes, _mask), do: :erlang.nif_error(:nif_not_loaded)

  @spec concat_scan_patterns(Krebs.ScanPattern.t(), Krebs.ScanPattern.t()) ::
          Krebs.ScanPattern.t()
  def concat_scan_patterns(_p0, _p1), do: :erlang.nif_error(:nif_not_loaded)

  # ── Process enumeration (static — no open handle needed) ──────────────────

  @spec list_processes() :: [{non_neg_integer, String.t()}]
  def list_processes(), do: :erlang.nif_error(:nif_not_loaded)

  @spec search_processes(String.t()) :: [non_neg_integer]
  def search_processes(_name), do: :erlang.nif_error(:nif_not_loaded)

  @spec list_windows() :: [{non_neg_integer, [String.t()]}]
  def list_windows(), do: :erlang.nif_error(:nif_not_loaded)

  @spec search_windows(String.t()) :: [non_neg_integer]
  def search_windows(_title), do: :erlang.nif_error(:nif_not_loaded)

  # ── Process attachment + accessors ────────────────────────────────────────

  @spec attach(non_neg_integer, :read | :read_write) ::
          {:ok, Krebs.ProcessRef.t()} | {:error, term}
  def attach(_pid, _access), do: :erlang.nif_error(:nif_not_loaded)

  @spec process_pid(Krebs.ProcessRef.t()) :: non_neg_integer
  def process_pid(_proc), do: :erlang.nif_error(:nif_not_loaded)

  @spec executable_name(Krebs.ProcessRef.t()) :: String.t()
  def executable_name(_proc), do: :erlang.nif_error(:nif_not_loaded)

  @spec window_names(Krebs.ProcessRef.t()) :: [String.t()]
  def window_names(_proc), do: :erlang.nif_error(:nif_not_loaded)

  @spec access_level(Krebs.ProcessRef.t()) :: :read | :read_write
  def access_level(_proc), do: :erlang.nif_error(:nif_not_loaded)

  @spec close(Krebs.ProcessRef.t()) :: :ok
  def close(_proc), do: :erlang.nif_error(:nif_not_loaded)

  # ── Memory read/write ─────────────────────────────────────────────────────

  @spec read(Krebs.ProcessRef.t(), non_neg_integer, pos_integer) ::
          {:ok, binary} | {:error, term}
  def read(_proc, _addr, _size), do: :erlang.nif_error(:nif_not_loaded)

  @spec write(Krebs.ProcessRef.t(), non_neg_integer, list) ::
          {:ok, non_neg_integer} | {:error, term}
  def write(_proc, _addr, _data), do: :erlang.nif_error(:nif_not_loaded)

  # ── Process map ──────────────────────────────────────────────────────────

  @spec regions(Krebs.ProcessRef.t()) :: {:ok, [Krebs.Region.t()]} | {:error, term}
  def regions(_proc), do: :erlang.nif_error(:nif_not_loaded)

  @spec modules(Krebs.ProcessRef.t()) :: {:ok, [Krebs.Module.t()]} | {:error, term}
  def modules(_proc), do: :erlang.nif_error(:nif_not_loaded)

  # ── Scanner lifecycle ────────────────────────────────────────────────────

  @spec scanner_new(Krebs.ProcessRef.t(), pos_integer) ::
          {:ok, Krebs.ScannerRef.t()} | {:error, term}
  def scanner_new(_proc, _read_size), do: :erlang.nif_error(:nif_not_loaded)

  @spec refresh_layout(Krebs.ScannerRef.t()) :: :ok
  def refresh_layout(_scanner), do: :erlang.nif_error(:nif_not_loaded)

  # ── Scanner operations ───────────────────────────────────────────────────

  @spec is_in_memory(Krebs.ScannerRef.t(), non_neg_integer, :heap | :module) ::
          {:ok, boolean} | {:error, term}
  defp is_in_memory(_scanner, _addr, _mem_type), do: :erlang.nif_error(:nif_not_loaded)

  @spec in_memory?(Krebs.ScannerRef.t(), non_neg_integer, :heap | :module) ::
          {:ok, boolean} | {:error, term}
  def in_memory?(scanner, addr, mem_type), do: is_in_memory(scanner, addr, mem_type)

  @spec scan(Krebs.ScannerRef.t(), Krebs.ScanPattern.t(), pid, :heap | :module) ::
          {:ok, non_neg_integer} | {:error, term}
  def scan(_scanner, _pattern, _recipient, _mem_type), do: :erlang.nif_error(:nif_not_loaded)

  # ── Log bridge ───────────────────────────────────────────────────────────

  @spec nif_log_init(pid) :: :ok
  def nif_log_init(_pid), do: :erlang.nif_error(:nif_not_loaded)
end
