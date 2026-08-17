defmodule Krebs.Region do
  @moduledoc "A memory region in the target process."

  defstruct [:base, :range_start, :range_end, :perms, :mem_type, :state]

  @type t :: %__MODULE__{
          base: non_neg_integer,
          range_start: non_neg_integer,
          range_end: non_neg_integer,
          perms: String.t(),
          mem_type: String.t(),
          state: String.t()
        }

  def to_range(%Krebs.Region{range_start: start, range_end: rend}), do: start..rend

  defimpl Inspect do
    import Inspect.Algebra

    def inspect(%Krebs.Region{} = r, opts) do
      hex = fn n -> "0x" <> Integer.to_string(n, 16) end

      concat([
        "#Krebs.Region<",
        hex.(r.base),
        ", ",
        hex.(r.range_start),
        "..",
        hex.(r.range_end),
        ", perms=",
        to_doc(r.perms, opts),
        ", type=",
        to_doc(r.mem_type, opts),
        ", state=",
        to_doc(r.state, opts),
        ">"
      ])
    end
  end
end
