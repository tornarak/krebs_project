defmodule Rekto.Schema.Metadata do
  @enforce_keys [:addr, :assoc_type]
  defstruct [:addr, :assoc_type, :memo]

  @type t :: %__MODULE__{
          addr: non_neg_integer | nil,
          assoc_type: Rekto.Schema.assoc_type(),
          memo: any
        }

  defimpl Inspect do
    import Inspect.Algebra
    alias Rekto.Schema.Helpers

    def inspect(%Rekto.Schema.Metadata{addr: addr, assoc_type: assoc_type}, opts) do
      concat([
        "#Rekto.Schema.Metadata<",
        if(addr, do: Helpers.print_ptr(addr), else: "nil"),
        ", ",
        to_doc(assoc_type, opts),
        ">"
      ])
    end
  end

  def default, do: %{addr: nil, assoc_type: :none}
end
