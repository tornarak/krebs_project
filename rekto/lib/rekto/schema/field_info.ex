defmodule Rekto.Schema.FieldInfo do
  @moduledoc "Metadata about a single field in a Rekto schema."

  @type t :: %__MODULE__{
          name: atom,
          data_type: Rekto.Serialization.datatype(),
          offset: non_neg_integer,
          size: pos_integer,
          points_to: module | nil,
          opts: Keyword.t()
        }

  defstruct [:name, :data_type, :offset, :size, :points_to, :opts]
end
