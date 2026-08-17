defmodule Krebs.ScannerRef do
  @moduledoc false
  # Elixir counterpart of the Rust `ScannerRef` NifStruct.
  # Holds a scanner resource that shares the underlying process Arc with a ProcessRef.
  # Created by `Krebs.Nif.scanner_new/2`; garbage collected when no references remain.
  @type t :: %__MODULE__{resource: reference()}
  defstruct [:resource]
end
