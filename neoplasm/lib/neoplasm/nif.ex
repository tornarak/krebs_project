defmodule Neoplasm.Nif do
  use Rustler, otp_app: :neoplasm, crate: :neoplasm_nif

  def open_window(_recipient), do: :erlang.nif_error(:nif_not_loaded)
  def close_window(), do: :erlang.nif_error(:nif_not_loaded)

  def set_attached(_attached, _reason), do: :erlang.nif_error(:nif_not_loaded)
  def set_scan_pattern(_hex), do: :erlang.nif_error(:nif_not_loaded)
  def set_scan_errors(_errors), do: :erlang.nif_error(:nif_not_loaded)
  def set_processes(_processes), do: :erlang.nif_error(:nif_not_loaded)
  def set_schemas(_schemas), do: :erlang.nif_error(:nif_not_loaded)
  def set_scan_status(_status), do: :erlang.nif_error(:nif_not_loaded)
  def push_results(_addrs), do: :erlang.nif_error(:nif_not_loaded)
  def clear_results(), do: :erlang.nif_error(:nif_not_loaded)

  # Hex dump inspector
  def set_hex_dump(_rows), do: :erlang.nif_error(:nif_not_loaded)
  # regions: [{base :: u64, size :: u64, kind :: :heap | :module | :other}]
  def set_memory_layout(_regions), do: :erlang.nif_error(:nif_not_loaded)

  # Display an error modal in the GUI.
  def set_error(_message), do: :erlang.nif_error(:nif_not_loaded)

  # Push the result of a cast preview: {:ok, value_string} | {:error, reason_string}
  def set_cast_preview(_result), do: :erlang.nif_error(:nif_not_loaded)

  # Push the full watched address list (with read results) to the GUI.
  # entries: [{addr :: u64, label :: binary, type_name :: binary, auto_refresh :: bool, bare_map :: bool, {:ok, value} | {:error, reason}}]
  def set_watch_entries(_entries), do: :erlang.nif_error(:nif_not_loaded)

  # Push inferred field values to the GUI (schema mode only).
  # fields: [{field_name :: binary, display_value :: binary}]
  def set_inferred_fields(_fields), do: :erlang.nif_error(:nif_not_loaded)

  # Word size (pushed once at init)
  def set_word_size(_size), do: :erlang.nif_error(:nif_not_loaded)

  # Schema builder results
  def set_schema_validation(_errors), do: :erlang.nif_error(:nif_not_loaded)
  def set_schema_generated(_result), do: :erlang.nif_error(:nif_not_loaded)
  def set_schema_load_result(_msg), do: :erlang.nif_error(:nif_not_loaded)

  # Reset scan-specific state (results, status, pattern, errors).
  def reset_scan_state(), do: :erlang.nif_error(:nif_not_loaded)

  # Reset all process-specific state (detach + scan state). Leaves schemas intact.
  def reset_process_state(), do: :erlang.nif_error(:nif_not_loaded)

  # Resolve a pending call from the GUI thread.
  def nif_reply(_token, _response), do: :erlang.nif_error(:nif_not_loaded)
end
