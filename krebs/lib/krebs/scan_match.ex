defmodule Krebs.ScanMatch do
  @moduledoc """
  A series of bytes that matched a `Krebs.ScanPattern`.

  Consists of the bytes in question, and the address at which
  they were found.
  """

  @enforce_keys [:addr, :value]
  defstruct [:addr, :value]

  @type t :: %__MODULE__{
          addr: pos_integer,
          value: binary
        }

  defimpl Inspect do
    import Inspect.Algebra

    alias Krebs.ScanMatch

    defp format_addr(addr) do
      "0x#{addr |> Integer.to_string(16) |> String.pad_leading(8, "0")}"
    end

    def inspect(%ScanMatch{addr: addr, value: value}, opts) do
      concat([
        "#Krebs.ScanMatch<",
        format_addr(addr),
        ":",
        break(" "),
        to_doc(value, %{opts | binaries: :as_binaries}),
        ">"
      ])
    end
  end
end
