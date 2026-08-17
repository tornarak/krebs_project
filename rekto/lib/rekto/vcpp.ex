defmodule Rekto.VCPP do
  defmodule StdString do
    @moduledoc """
    Visual C++ [std::string](https://www.cplusplus.com/reference/string/string/).
    """

    defmodule Validation do
      @moduledoc """
      Shared validation logic for VCPP short and long strings.

      These sanity checks replicate the libkrebs::vcpp::string validations.
      """

      @doc """
      Validates buffer integrity for short strings.

      Checks:
      - Null terminator exists at position `length`
      - No embedded null bytes before position `length`

      Note: The buffer is always 16 bytes (fixed in schema), so we don't check
      its length. The null terminator check is both necessary and sufficient.
      Bytes after the terminator may contain debug data (0xCC in debug mode).
      """
      @spec check_short_buffer(%{buffer: binary, length: non_neg_integer}) ::
              :ok | {:error, term}
      def check_short_buffer(%{buffer: buf, length: len}) do
        cond do
          # Check null terminator at position `len`
          :binary.at(buf, len) != 0 ->
            {:error, {:no_null_terminator, len, :binary.at(buf, len)}}

          # Check for embedded null bytes before position `len`
          has_embedded_nulls?(buf, len) ->
            {:error, {:embedded_null_bytes, len}}

          true ->
            :ok
        end
      end

      @doc """
      Validates buffer integrity for long strings.

      Checks:
      - Null terminator exists at position `length` in the external buffer
      - No embedded null bytes before position `length`
      - Buffer has exactly `length + 1` bytes

      This is applied to the externally-allocated string buffer after it's loaded.
      """
      @spec check_long_buffer(binary, non_neg_integer) ::
              :ok | {:error, term}
      def check_long_buffer(buf, len) do
        expected_len = len + 1

        cond do
          byte_size(buf) != expected_len ->
            {:error, {:buffer_length_mismatch, byte_size(buf), expected_len}}

          # Check null terminator at position `len`
          :binary.at(buf, len) != 0 ->
            {:error, {:no_null_terminator, len, :binary.at(buf, len)}}

          # Check for embedded null bytes before position `len`
          has_embedded_nulls?(buf, len) ->
            {:error, {:embedded_null_bytes, len}}

          true ->
            :ok
        end
      end

      # Helper: check if a buffer has null bytes before the given position
      defp has_embedded_nulls?(buf, len) do
        buf
        |> binary_part(0, len)
        |> :binary.match(<<0>>)
        |> case do
          :nomatch -> false
          _ -> true
        end
      end
    end

    defmodule Short do
      @moduledoc """
      Visual C++ [std::string](https://www.cplusplus.com/reference/string/string/)
      with a length under 16 characters. The string data is stored in an internal buffer.
      """

      use Rekto.Schema

      alias Rekto.VCPP.StdString.Short

      sanity(Rekto.VCPP.StdString.Validation, :check_short_buffer)

      @type t :: %__MODULE__{
              buffer: binary,
              length: 1..15,
              capacity: 15
            }

      schema do
        field(:buffer, {:string_buffer, 16})
        field(:length, :word, constraints: [non_null: true, range: {1, 15}])
        field(:capacity, :word, constraints: [non_null: true, const: 15])
      end

      @impl Schema
      def custom_mask(%{length: len}) do
        String.duplicate(<<0xFF>>, len) <>
          String.duplicate(<<0x00>>, 16 - len) <>
          String.duplicate(<<0xFF>>, 2 * Rekto.Serialization.get_word_size())
      end

      @impl Schema
      def custom_mask(%{}) do
        String.duplicate(<<0xFF>>, Rekto.Serialization.get_type_size!(__MODULE__))
      end

      @impl Schema
      def pattern_inference(%{buffer: buf}) when is_binary(buf) do
        %{length: byte_size(buf), capacity: 15}
      end

      def pattern_inference(%{}), do: %{}

      defimpl Inspect do
        import Inspect.Algebra

        def inspect(%Rekto.VCPP.StdString.Short{__meta__: meta} = str, opts) do
          concat([
            "#Rekto.VCPP.StdString.Short<",
            to_doc(to_string(str), opts),
            " | __meta__: ",
            to_doc(meta, opts),
            ">"
          ])
        end
      end

      defimpl List.Chars do
        @spec to_charlist(Rekto.VCPP.StdString.Short.t()) :: charlist
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
      Visual C++ [std::string](https://www.cplusplus.com/reference/string/string/)
      with a length greater than or equal to 16 characters. The string data is stored
      on the heap, and the `std::string` itself contains a pointer to this data.
      """

      use Rekto.Schema

      alias Rekto.VCPP.StdString.Long

      @alloc_sizes [
        0x1F,
        0x2F,
        0x46,
        0x69,
        0x9D,
        0xEB,
        0x160,
        0x210,
        0x318,
        0x4A4,
        0x6F6,
        0xA71,
        0xFA9,
        0x17A4,
        0x2362,
        0x34FF,
        0x4F6B,
        0x770D,
        0xB280,
        0x10BAC,
        0x1916E,
        0x25A11,
        0x38706,
        0x54A75,
        0x7EF9C,
        0xBE756,
        0x11DAED
      ]

      @type t :: %__MODULE__{
              buffer: Rekto.Association.NotLoaded.t() | Rekto.Association.Primitive.t(),
              length: 16..0x11DAEC,
              capacity: pos_integer
            }

      sanity(:valid_capacity)
      sanity(:buffer_pointer_not_null)
      transform(:buffer_length)

      schema do
        # Rekto.Void is a special type for pointers whose types are dynamically
        # decided (more on that later). Attempting to preload one will raise an exception.
        # The default pointer type tracks the configured :word_type.
        #
        # `:length`/`:capacity` are `size_t` in the real MSVC ABI (word-sized, not a fixed
        # 4 bytes) — but they always start at byte offset 16 regardless of bitness, since
        # that offset is set by the fixed 16-byte short-string buffer/pointer union, not by
        # pointer width.
        points_to(:buffer, Rekto.Void, constraints: [non_null: true])
        field(:length, :word, offset: 16, constraints: [range: {16, List.last(@alloc_sizes) - 1}])
        field(:capacity, :word, constraints: [in: @alloc_sizes])
      end

      @doc """
      A transformation.

      Changes the buffer association into a buffer `{:string_buffer, length}`.
      """
      def buffer_length(
            %Long{length: len, buffer: %Rekto.Association.NotLoaded{} = buf_assoc} = str
          ) do
        # Update the `string_buffer` association to be the correct length
        %Long{
          str
          | buffer: %Rekto.Association.NotLoaded{
              buf_assoc
              | data_type: {:string_buffer, len}
            }
        }
      end

      @doc """
      Gets the `:capacity` value for a string of the given length.

      Returns nil if `len` is greater than or equal to `0x11DAED`.
      """
      @spec get_capacity(pos_integer) :: pos_integer | nil
      def get_capacity(len) do
        Enum.find(@alloc_sizes, nil, fn cap -> cap > len end)
      end

      @doc """
      A sanity check.

      Ensures that the capacity is valid (in the ALLOC_SIZES list and matches the length).
      """
      def valid_capacity(%Long{length: len, capacity: cap}) do
        if cap == get_capacity(len) do
          :ok
        else
          {:error, :invalid_capacity}
        end
      end

      @doc """
      A sanity check.

      Ensures that the string buffer pointer is not null.
      Replaces the libkrebs check: `if self.c_str_addr == 0 { Err(NullPtr) }`.
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

      @impl Schema
      def pattern_inference(%{length: len}) when is_integer(len) do
        case get_capacity(len) do
          nil -> %{}
          cap -> %{capacity: cap}
        end
      end

      def pattern_inference(%{}), do: %{}

      defimpl Inspect do
        import Inspect.Algebra

        def inspect(%Rekto.VCPP.StdString.Long{buffer: buf, __meta__: meta}, opts) do
          content =
            case buf do
              %Rekto.Association.Primitive{value: str} -> to_doc(str, opts)
              _ -> "<not loaded>"
            end

          concat([
            "#Rekto.VCPP.StdString.Long<",
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

    alias Rekto.VCPP
    alias Rekto.VCPP.StdString.{Short, Long}

    transform(:determine_type)

    schema do
      field(:unk, {:byte, 16})
      field(:length, :u32, offset: 16, constraints: [range: {0, 1_000_000}])
      field(:capacity, :u32, constraints: [range: {15, 1_000_000}])
    end

    @doc """
    A transformation.

    Converts the string to a `Rekto.VCPP.StdString.Short` if its length is less than 16.
    Otherwise, converts it to a `Rekto.VCPP.StdString.Long`.
    """
    @spec determine_type(%VCPP.StdString{}) :: Short.t() | Long.t() | {:error, any}
    def determine_type(%VCPP.StdString{length: len} = str) do
      str_result =
        if len < 16 do
          str
          |> VCPP.StdString.to_binary()
          |> Short.from_binary(str.__meta__)
        else
          str
          |> VCPP.StdString.to_binary()
          |> Long.from_binary(str.__meta__)
        end

      with {:ok, str} <- str_result do
        str
      end
    end
  end

  defmodule StdVector do
    use Rekto.Schema

    # alias Rekto.Association.NotLoaded
    # alias Rekto.Schema.{Helpers, Metadata}
    alias Rekto.Serialization

    sanity(:array_order)

    # transform :to_byte_array

    schema do
      field(:arr_start, :u32, constraints: [non_null: true])
      field(:last_addr, :u32, constraints: [non_null: true])
      field(:arr_end, :u32)
    end

    @type t :: %__MODULE__{
            arr_start: pos_integer(),
            last_addr: pos_integer(),
            arr_end: pos_integer()
          }

    def array_order(%{arr_start: first, last_addr: last, arr_end: end_addr})
        when first < last and last <= end_addr,
        do: :ok

    def array_order(%{arr_start: first, last_addr: last}),
      do:
        {:error,
         "End address (#{Schema.Helpers.print_ptr(last)}) >= First address (#{Schema.Helpers.print_ptr(first)})"}

    @spec get_byte_size(Rekto.VCPP.StdVector.t()) :: pos_integer
    def get_byte_size(%__MODULE__{arr_start: start, last_addr: last}) do
      last - start
    end

    def get_length(%__MODULE__{} = vec, type) do
      get_byte_size(vec) / Serialization.get_type_size!(type)
    end

    #
    # defp set_backing_type(%__MODULE__{backing_array: %NotLoaded{}=backing_arr}=vec, new_type) do
    #   %Rekto.VCPP.StdVector{vec | backing_array: %NotLoaded{backing_arr | data_type: new_type}}
    # end
    # def to_byte_array(%__MODULE__{}=vec) do
    #   set_backing_type(vec, {:byte, get_byte_size(vec)})
    # end
    #
    # def set_vector_types(%{__struct__: mod, __meta__: %Rekto.Schema.Metadata{}}=o_struct) do
    #   vec_fields = Helpers.get_fields_with_opt(o_struct, :vector_type)
    #
    #   Enum.all?(vec_fields, fn {name, _, _ } ->
    #     Helpers.get_field_type!(mod, name) == __MODULE__
    #       || raise ArgumentError, "field #{name} has option :vector_type but is not a vector"
    #   end)
    #
    #   new_vec_fields =
    #     vec_fields
    #     |> Enum.map(fn {name, type, %__MODULE__{}=vec} ->
    #       {name, set_backing_type(vec, {type, get_length(vec, type)})}
    #     end)
    #
    #   struct(o_struct, new_vec_fields)
    # end

    defimpl Inspect do
      import Inspect.Algebra
      alias Rekto.Schema.Helpers

      def inspect(%Rekto.VCPP.StdVector{arr_start: first, last_addr: last, arr_end: end_addr}, _) do
        concat([
          "#Rekto.VCPP.StdVector<",
          "[[#{Helpers.print_ptr(first)} - #{Helpers.print_ptr(last)}] - #{Helpers.print_ptr(end_addr)}]",
          ">"
        ])
      end
    end
  end
end
