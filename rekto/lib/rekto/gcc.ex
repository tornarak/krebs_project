defmodule Rekto.GCC do
  defmodule StdString do
    @moduledoc """
    GCC [std::string](https://www.cplusplus.com/reference/string/string/).
    """

    defmodule Short do
      @moduledoc """
      GCC [std::string](https://www.cplusplus.com/reference/string/string/)
      with a length under 16 characters. The string data is stored in an internal buffer.

      Layout (32-bit):
      - offset 0x00: c_str (word) — pointer to the internal string buffer (self-referential)
      - offset 0x08: length (word)
      - offset 0x10: buffer ([u8; 16])
      """

      use Rekto.Schema
      require Rekto.Serialization

      alias Rekto.GCC.StdString.Short

      sanity(Rekto.VCPP.StdString.Validation, :check_short_buffer)

      @type t :: %__MODULE__{
              c_str: non_neg_integer,
              length: 1..15,
              buffer: binary
            }

      schema do
        field(:c_str, :this_pointer,
          pointer_offset: :buffer,
          constraints: [non_null: true]
        )

        field(:length, :word, constraints: [non_null: true, range: {1, 15}])
        field(:buffer, {:string_buffer, 16})
      end

      @impl Schema
      def custom_mask(%{length: len}) do
        word_size = Rekto.Serialization.get_word_size()

        String.duplicate(<<0xFF>>, word_size) <>
          String.duplicate(<<0xFF>>, word_size) <>
          String.duplicate(<<0xFF>>, len) <>
          String.duplicate(<<0x00>>, 16 - len)
      end

      @impl Schema
      def custom_mask(%{}) do
        String.duplicate(<<0xFF>>, Rekto.Serialization.get_type_size!(__MODULE__))
      end

      @impl Schema
      def pattern_inference(%{buffer: buf}) when is_binary(buf) do
        %{length: byte_size(buf)}
      end

      def pattern_inference(%{}), do: %{}

      defimpl Inspect do
        import Inspect.Algebra

        def inspect(%Rekto.GCC.StdString.Short{__meta__: meta} = str, opts) do
          concat([
            "#Rekto.GCC.StdString.Short<",
            to_doc(to_string(str), opts),
            " | __meta__: ",
            to_doc(meta, opts),
            ">"
          ])
        end
      end

      defimpl List.Chars do
        @spec to_charlist(Rekto.GCC.StdString.Short.t()) :: charlist
        def to_charlist(%{} = str) do
          str |> Kernel.to_string() |> :erlang.binary_to_list()
        end
      end

      defimpl String.Chars do
        def to_string(%{buffer: str_bin, length: len}) do
          str_bin |> binary_part(0, len) |> Kernel.to_string()
        end
      end
    end

    defmodule Long do
      @moduledoc """
      GCC [std::string](https://www.cplusplus.com/reference/string/string/)
      with a length greater than or equal to 16 characters. The string data is stored
      on the heap, and the `std::string` itself contains a pointer to this data.

      Layout (32-bit):
      - offset 0x00: c_str (word) — pointer to external string buffer on heap
      - offset 0x08: length (word)
      - offset 0x10: capacity (word) — allocation size (not including null terminator)
      - offset 0x18: unk ([u8; 8]) — unknown, possibly refcount structure
      """

      use Rekto.Schema

      alias Rekto.GCC.StdString.Long

      @type t :: %__MODULE__{
              buffer: Rekto.Association.NotLoaded.t() | Rekto.Association.Primitive.t(),
              length: 16..16_000_000,
              capacity: non_neg_integer,
              unk: binary
            }

      sanity(:buffer_pointer_not_null)
      sanity(:capacity_not_less_than_length)
      transform(:buffer_length)

      schema do
        points_to(:buffer, Rekto.Void, constraints: [non_null: true])
        field(:length, :word, constraints: [range: {16, 16_000_000}])
        field(:capacity, :word)
        field(:unk, {:byte, 8})
      end

      @doc """
      A transformation.

      Changes the buffer association into an array of `{:byte, length}`.
      """
      def buffer_length(
            %Long{length: len, buffer: %Rekto.Association.NotLoaded{} = buf_assoc} = str
          ) do
        # Update the `buffer` association to be the correct length
        %Long{
          str
          | buffer: %Rekto.Association.NotLoaded{
              buf_assoc
              | data_type: {:string_buffer, len}
            }
        }
      end

      @doc """
      A sanity check.

      Ensures that the string buffer pointer is not null.
      Replaces the libkrebs check: `if self.c_str == 0 { Err(NullPtr) }`.
      """
      @spec buffer_pointer_not_null(%Long{}) ::
              :ok | {:error, :null_buffer_pointer}
      def buffer_pointer_not_null(%Long{
            buffer: %Rekto.Association.NotLoaded{__meta__: %{addr: addr}}
          }) do
        if addr == 0 do
          {:error, :null_buffer_pointer}
        else
          :ok
        end
      end

      def buffer_pointer_not_null(%Long{}), do: :ok

      @doc """
      A sanity check.

      Ensures that capacity >= length.
      Replaces the libkrebs check: `if self.capacity < self.length { Err(CapacityTooSmall) }`.
      """
      @spec capacity_not_less_than_length(%Long{}) ::
              :ok | {:error, {:capacity_too_small, non_neg_integer, non_neg_integer}}
      def capacity_not_less_than_length(%Long{length: len, capacity: cap}) do
        if cap < len do
          {:error, {:capacity_too_small, cap, len}}
        else
          :ok
        end
      end

      defimpl Inspect do
        import Inspect.Algebra

        def inspect(%Rekto.GCC.StdString.Long{buffer: buf, __meta__: meta}, opts) do
          content =
            case buf do
              %Rekto.Association.Primitive{value: str} -> to_doc(str, opts)
              _ -> "<not loaded>"
            end

          concat([
            "#Rekto.GCC.StdString.Long<",
            content,
            " | __meta__: ",
            to_doc(meta, opts),
            ">"
          ])
        end
      end

      defimpl List.Chars do
        def to_charlist(%Long{} = long_str) do
          long_str |> to_string() |> :erlang.binary_to_list()
        end
      end

      defimpl String.Chars do
        def to_string(%Long{buffer: %Rekto.Association.Primitive{value: str_buf}}) do
          str_buf
        end
      end
    end

    use Rekto.Schema

    alias Rekto.GCC
    alias Rekto.GCC.StdString.{Short, Long}

    transform(:determine_type)

    schema do
      field(:c_str, :word, constraints: [non_null: true])
      field(:length, :word, constraints: [range: {1, 16_000_000}])
      field(:unk, {:byte, 16})
    end

    @doc """
    A transformation.

    Converts the string to a `Rekto.GCC.StdString.Short` if its length is less than 16.
    Otherwise, converts it to a `Rekto.GCC.StdString.Long`.
    """
    @spec determine_type(%GCC.StdString{}) :: Short.t() | Long.t() | {:error, any}
    def determine_type(%GCC.StdString{length: len} = str) do
      str_result =
        if len < 16 do
          str
          |> GCC.StdString.to_binary()
          |> Short.from_binary(str.__meta__)
        else
          str
          |> GCC.StdString.to_binary()
          |> Long.from_binary(str.__meta__)
        end

      with {:ok, str} <- str_result do
        str
      end
    end
  end
end
