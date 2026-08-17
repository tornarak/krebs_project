defmodule Krebs.ProcessRef do
  @moduledoc false
  # Elixir counterpart of the Rust `ProcessRef` NifStruct.
  # Holds a reference-counted pointer to an open process handle.
  # Created by `Krebs.Nif.attach/2`; garbage collected when no references remain.
  @type t :: %__MODULE__{resource: reference()}
  defstruct [:resource]
end
