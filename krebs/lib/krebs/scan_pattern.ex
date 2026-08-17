defmodule Krebs.ScanPattern do
  @moduledoc """
  Elixir wrapper around the Libkrebs `ScanPattern` trait.

  ## Patterns

  Patterns consist of a series of bytes (the value) and an optional parallel
  series of bytes (the mask).

  If there is no mask, the value is compared directly to the data being scanned.
  If there is a mask, (value & mask) is compared to (data & mask).

  Libkrebs masks are "byte-wise"; you can only control comparisons on a byte-by-byte
  basis. All non-zero bytes in the mask will be changed to `0xFF` automatically.
  """
  alias Krebs.Nif
  alias Krebs.ScanPattern

  @enforce_keys [:pattern_ref, :pattern_type, :bytes]
  defstruct [:pattern_ref, :pattern_type, :bytes, :mask]

  @typedoc """
  A pattern's type is determined automatically based on its length.
  There are three types of patterns, each with a different backing store:

  * `:byte` - Represented a series of bytes. Can be of any length.
  * `:word` - Represented as a series of words. Length must be a multiple
  of the word size (this depends on the machine). Noticeably faster
  than `:byte` patterns.
  * `:simd` - Represented as a 32-byte SIMD vector. All patterns under 32
  bytes are automatically padded to fit into this vector. The fastest type-
  much faster than `:byte` and `:word` patterns.
  """
  @type pattern_type :: :byte | :word | :simd
  @type t :: %__MODULE__{
          pattern_ref: reference,
          pattern_type: pattern_type,
          bytes: binary,
          mask: binary | nil
        }

  # Lifts a function to operate on nullable values (nil passes through unchanged).
  defp monad(f) do
    &if &1 do
      f.(&1)
    else
      &1
    end
  end

  # Binary data is returned from Rust as a binary (Erlang String type) but must
  # be passed as a charlist (Vec<u8> on the Rust side) to avoid UTF-8 validation.

  @doc false
  def to_ffi(%ScanPattern{} = struct) do
    struct
    |> Map.update!(:bytes, &:erlang.binary_to_list/1)
    |> Map.update!(:mask, monad(&:erlang.binary_to_list/1))
  end

  @doc false
  def from_ffi(%ScanPattern{} = struct) do
    struct
    |> Map.update!(:bytes, &:erlang.list_to_binary/1)
    |> Map.update!(:mask, monad(&:erlang.list_to_binary/1))
  end

  @doc """
  Creates a new scan pattern with the given value and mask. The mask must either be nil
  or a binary with the same length as the value. See the Libkrebs Rust documentation for
  more detail.

  The pattern is automatically optimized into a `:byte`, `:word`, or `:simd` pattern depending
  on its size.

  ## Examples

      iex> alias Krebs.ScanPattern
      Krebs.ScanPattern
      iex> ScanPattern.new(<<1, 2, 3, 4>>)
      #Krebs.ScanPattern<:simd, bytes: <<1, 2, 3, 4>>, mask: nil>
      iex> ScanPattern.new(<<1, 2, 3, 4>>, <<0, 0, 1, 1>>)
      #Krebs.ScanPattern<:simd, bytes: <<1, 2, 3, 4>>, mask: ??XX>
      iex> ScanPattern.new(String.duplicate(<<1>>, 33))
      #Krebs.ScanPattern<:byte, bytes: <<1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1>>, mask: nil>
      iex> ScanPattern.new(String.duplicate(<<1, 2, 3, 4, 5, 6, 7, 8>>, 5))
      #Krebs.ScanPattern<:word, bytes: <<1, 2, 3, 4, 5, 6, 7, 8, 1, 2, 3, 4, 5, 6, 7, 8, 1, 2, 3, 4, 5, 6, 7, 8, 1, 2, 3, 4, 5, 6, 7, 8, 1, 2, 3, 4, 5, 6, 7, 8>>, mask: nil>
      iex> ScanPattern.new(String.duplicate(<<1>>, 33) <> <<2>>)
      #Krebs.ScanPattern<:byte, bytes: <<1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 2>>, mask: nil>
  """
  @spec new(binary, binary | nil) :: ScanPattern.t()
  def new(pattern, mask \\ nil) when is_binary(pattern) and (is_binary(mask) or is_nil(mask)) do
    Nif.new_scan_pattern(pattern, mask)
    |> from_ffi
  end

  @doc """
  Concatenates the scan patterns.

  The concatenated pattern is optimized into the optimal type,
  as in `new/2`.

  ## Examples

  Note: `|` is an Elixir special form and cannot be used in `iex>` doctests.
  Use it via `import` in your own code:

      import Krebs.ScanPattern, only: [{:|, 2}]
      pattern1 = ScanPattern.new(<<1, 2, 3, 4>>)
      pattern2 = ScanPattern.new(<<5, 6, 7, 8>>, <<255, 0, 0, 255>>)
      pattern1 | pattern2
      #=> #Krebs.ScanPattern<:simd, bytes: <<1, 2, 3, 4, 5, 6, 7, 8>>, mask: XXXXX??X>

  """
  @spec t() | t() :: t()
  def %ScanPattern{} = p0 | %ScanPattern{} = p1 do
    Nif.concat_scan_patterns(to_ffi(p0), to_ffi(p1))
    |> from_ffi
  end

  defimpl Inspect do
    import Inspect.Algebra

    defp print_mask(nil), do: "nil"

    defp print_mask(mask) do
      mask
      |> :erlang.binary_to_list()
      |> Enum.map(fn val -> if val != 0, do: ?X, else: ?? end)
      |> :erlang.list_to_binary()
    end

    def inspect(%Krebs.ScanPattern{pattern_type: type, bytes: bytes, mask: mask}, opts) do
      concat([
        "#Krebs.ScanPattern<",
        to_doc(type, opts),
        ", bytes: ",
        to_doc(bytes, %{opts | binaries: :as_binaries}),
        ", mask: ",
        print_mask(mask),
        ">"
      ])
    end
  end
end
