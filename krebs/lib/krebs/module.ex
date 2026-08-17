defmodule Krebs.Module do
  @moduledoc "A loaded module (shared library or executable segment) in the target process."

  defstruct [:name, :range_start, :range_end]

  @type t :: %__MODULE__{
          name: String.t(),
          range_start: non_neg_integer,
          range_end: non_neg_integer
        }

  def to_range(%Krebs.Module{range_start: start, range_end: rend}), do: start..rend

  defimpl Inspect do
    import Inspect.Algebra

    def inspect(%Krebs.Module{} = m, opts) do
      hex = fn n -> "0x" <> Integer.to_string(n, 16) end

      concat([
        "#Krebs.Module<",
        to_doc(m.name, opts),
        ", ",
        hex.(m.range_start),
        "..",
        hex.(m.range_end),
        ">"
      ])
    end
  end
end
