defmodule Rekto.Serialization do
  @moduledoc """
  Facilities for converting Elixir values to bytes.

  ## Notes

  `:byte` is a special case. An individual `:byte` will be converted to a number in
  the range [0, 255], but an array of `:byte`s will be converted to a binary.

  Erlang/Elixir has no NaN value; attempting to convert an NaN
  either way will result in an `ArgumentError` being thrown.

  `:word` resolves to the configured word type (`:word_type` in application config,
  defaults to `:u32`). Its byte size is computed at compile time.
  """
  alias Rekto.Association
  alias Rekto.Schema
  alias Rekto.Schema.Metadata

  @word_type Application.compile_env(:rekto, :word_type, :u32)

  @type byte_primitive :: :bool | :byte | :u8 | :i8
  @type short_primitive :: :u16 | :i16
  @type dword_primitive :: :u32 | :i32 | :f32
  @type qword_primitive :: :u64 | :i64 | :f64
  @type native_primitive :: :word
  @type primitive ::
          byte_primitive
          | short_primitive
          | native_primitive
          | dword_primitive
          | qword_primitive
          | {:string_buffer, pos_integer}
  @type datatype ::
          primitive | {datatype, pos_integer} | Schema.t()

  @type u8 :: 0x00..0xFF
  @u8_range 0x00..0xFF
  @type i8 :: -0x80..0x7F
  @i8_range -0x80..0x7F

  @type u16 :: 0x0000..0xFFFF
  @u16_range 0x0000..0xFFFF
  @type i16 :: -0x8000..0x7FFF
  @i16_range -0x8000..0x7FFF

  @type u32 :: 0x00000000..0xFFFFFFFF
  @u32_range 0x00000000..0xFFFFFFFF
  @type i32 :: -0x80000000..0x7FFFFFFF
  @i32_range -0x80000000..0x7FFFFFFF

  @type u64 :: 0x0000000000000000..0xFFFFFFFFFFFFFFFF
  @u64_range 0x0000000000000000..0xFFFFFFFFFFFFFFFF
  @type i64 :: -0x8000000000000000..0x7FFFFFFFFFFFFFFF
  @i64_range -0x8000000000000000..0x7FFFFFFFFFFFFFFF

  @type serializable_bytes :: <<_::8>> | <<_::16>> | <<_::32>> | <<_::64>> | <<_::_*8>>
  @type serializable_type ::
          u8
          | i8
          | u16
          | i16
          | u32
          | i32
          | u64
          | i64
          | float
          | boolean
          | binary
          | Schema.schema_struct()
          | [serializable_type]

  @all_primitives [:bool, :byte, :u8, :i8, :u16, :i16, :u32, :i32, :f32, :u64, :i64, :f64, :word]

  def primitives, do: @all_primitives

  defguard(
    is_primitive(x)
    when x in @all_primitives or
           (is_tuple(x) and
              tuple_size(x) == 2 and
              elem(x, 0) == :string_buffer and
              is_integer(elem(x, 1)) and elem(x, 1) > 0) or
           (is_tuple(x) and
              tuple_size(x) == 2 and
              elem(x, 0) in @all_primitives and
              is_integer(elem(x, 1)) and elem(x, 1) > 0)
  )

  def datatype?(x) when is_primitive(x), do: true

  def datatype?({x, n})
      when is_integer(n) and n > 0 and is_primitive(x), do: true

  def datatype?(x), do: Schema.has_schema?(x)

  @doc false
  defmacro check_mod(module, do: block) do
    quote do
      mod = unquote(module)

      cond do
        not String.starts_with?(to_string(mod), "Elixir.") ->
          raise ArgumentError,
                "Atom #{inspect(mod)} is not a module (were you trying to convert a primitive or array?)"

        not Schema.has_schema?(mod) ->
          raise(ArgumentError, "Module #{inspect(mod)} has no schema")

        true ->
          unquote(block)
      end
    end
  end

  # The :u64 and fallthrough branches are unreachable when word_type is :u32 (the default).
  # This is by design — the unused branch serves as a compile-time guard for misconfiguration.
  @doc false
  defmacro word_size do
    case @word_type do
      :u32 -> 4
      :u64 -> 8
      other -> raise "Invalid word_type config: #{inspect(other)}"
    end
  end

  @doc """
  Gets the native word size in bytes (platform-specific, configured at compile time).

  Reads from `:rekto` config key `:word_type`. Defaults to `:u32` (4 bytes).

  ## Examples

      iex> get_word_size()
      4
  """
  @spec get_word_size() :: pos_integer
  def get_word_size, do: word_size()

  @doc """
  Gets the size of a given type in bytes. Works on primitives, arrays, and schemas.

  This is the canonical way to get the byte size of a schema.

  ## Examples

      iex> get_type_size!(:bool)
      1

      iex> get_type_size!(:word)
      4

      iex> get_type_size!(:u32)
      4

      iex> get_type_size!(:f64)
      8

      iex> get_type_size!({:byte, 20})
      20

      iex> get_type_size!(Rekto.VCPP.StdString.Short)
      24

  """
  @spec get_type_size!(datatype) :: pos_integer
  def get_type_size!(:word), do: word_size()
  def get_type_size!(:bool), do: 1
  def get_type_size!(:byte), do: 1
  def get_type_size!(:u8), do: 1
  def get_type_size!(:i8), do: 1

  def get_type_size!(:u16), do: 2
  def get_type_size!(:i16), do: 2

  def get_type_size!(:u32), do: 4
  def get_type_size!(:i32), do: 4
  def get_type_size!(:f32), do: 4

  def get_type_size!(:u64), do: 8
  def get_type_size!(:i64), do: 8
  def get_type_size!(:f64), do: 8

  def get_type_size!({:word, count}) when is_integer(count),
    do: count * word_size()

  def get_type_size!({:string_buffer, n}) when is_integer(n) and n > 0, do: n

  def get_type_size!({subtype, count}) when is_integer(count),
    do: count * get_type_size!(subtype)

  def get_type_size!(mod) when is_atom(mod) do
    cond do
      not String.starts_with?(to_string(mod), "Elixir.") ->
        raise ArgumentError,
              "Atom #{inspect(mod)} is not a module (were you trying to convert a primitive or array?)"

      true ->
        mod.__size__()
    end
  end

  def get_type_size!(unk),
    do: raise(ArgumentError, "#{inspect(unk)} is not a valid field type")

  # Layer 09: Protocol

  @doc """
  Converts bytes to the given datatype.
  Throws an error if the conversion is not possible.

      iex> from_bytes(<<0>>, :bool)
      false
      iex> from_bytes(<<1>>, :bool)
      true
      iex> (from_bytes(<<0x78, 0x56, 0x34, 0x12>>, :u32)
        |> Integer.to_string(16)
        |>  String.replace_prefix("", "0x"))
      "0x12345678"
      iex> from_bytes(<<0xFF, 0xFF, 0x7F, 0xFF>>, :f32)
      -3.4028234663852886e38
      iex> from_bytes(<<0xFF, 0xFF, 0xFF, 0xFF>>, :f32)
      ** (ArgumentError) NaN

  """
  @spec from_bytes(bytes :: binary, type :: datatype) :: {:ok, serializable_type} | {:error, any}
  def from_bytes(<<value::unsigned-native-integer-size(8)>>, :bool), do: {:ok, value != 0}
  def from_bytes(<<value::unsigned-native-integer-size(1)>>, :byte), do: {:ok, value}
  def from_bytes(<<value::unsigned-native-integer-size(8)>>, :u8), do: {:ok, value}
  def from_bytes(<<value::signed-native-integer-size(8)>>, :i8), do: {:ok, value}

  def from_bytes(<<value::unsigned-native-integer-size(16)>>, :u16), do: {:ok, value}
  def from_bytes(<<value::signed-native-integer-size(16)>>, :i16), do: {:ok, value}

  def from_bytes(<<value::unsigned-native-integer-size(32)>>, :u32), do: {:ok, value}
  def from_bytes(<<value::signed-native-integer-size(32)>>, :i32), do: {:ok, value}
  def from_bytes(<<value::native-float-size(32)>>, :f32), do: {:ok, value}
  def from_bytes(<<_::native-binary-size(4)>>, :f32), do: {:error, :nan}

  def from_bytes(<<value::unsigned-native-integer-size(64)>>, :u64), do: {:ok, value}
  def from_bytes(<<value::signed-native-integer-size(64)>>, :i64), do: {:ok, value}
  def from_bytes(<<value::native-float-size(64)>>, :f64), do: {:ok, value}
  def from_bytes(<<_::native-binary-size(8)>>, :f64), do: {:error, :nan}

  # Compile-time generation of :word clauses based on configured word_type
  case @word_type do
    :u32 ->
      def from_bytes(<<value::unsigned-native-integer-size(32)>>, :word), do: {:ok, value}

    :u64 ->
      def from_bytes(<<value::unsigned-native-integer-size(64)>>, :word), do: {:ok, value}

    other ->
      raise "Invalid word_type config: #{inspect(other)}"
  end

  def from_bytes(<<value::binary>>, {:byte, n}) do
    len = byte_size(value)
    unless len == n, do: raise(ArgumentError, "Expected #{n} bytes, got #{len}")

    {:ok, value}
  end

  def from_bytes(binary, {:string_buffer, n}) when byte_size(binary) == n do
    {:ok, binary}
  end

  def from_bytes(binary, {:string_buffer, n}),
    do: {:error, {:wrong_size, byte_size(binary), n}}

  def from_bytes(bytes, {subtype, count}) when is_integer(count) do
    type_size = get_type_size!(subtype)
    req_size = count * type_size
    bytes_size = byte_size(bytes)

    unless bytes_size == req_size,
      do:
        raise(
          ArgumentError,
          "Invalid size binary: #{inspect(bytes_size)} bytes when #{inspect(req_size)} bytes required}"
        )

    {:ok, arr_from_bytes(subtype, type_size, bytes, [])}
  end

  def from_bytes(bytes, prim) when is_primitive(prim),
    do: {:error, {:cannot_fit, prim, byte_size(bytes)}}

  @doc false
  @spec from_bytes(bytes :: serializable_bytes, Rekto.Schema.t(), map) ::
          {:ok, Rekto.Schema.schema_struct()} | {:error, any()}
  def from_bytes(bytes, mod, meta \\ %{}) when is_atom(mod) do
    if Rekto.Schema.has_schema?(mod) do
      new_meta = Map.merge(Rekto.Schema.Metadata.default(), meta)
      mod.from_binary(bytes, new_meta)
    else
      {:error, :no_schema}
    end
  end

  @spec arr_from_bytes(datatype, pos_integer, binary, [any]) :: [any]
  defp arr_from_bytes(element_type, element_size, bytes, acc) do
    case bytes do
      <<start::binary-size(element_size), rest::binary>> ->
        case from_bytes(start, element_type) do
          {:ok, new} ->
            arr_from_bytes(element_type, element_size, rest, [new | acc])

          {:error, _} = t ->
            t
        end

      _ ->
        acc |> Enum.reverse()
    end
  end

  # Layer 13: Ego (mine is bruised after writing this)

  @doc """
  Converts the datatype to bytes.
  Throws an error if the conversion is not possible.

      iex> to_bytes(true, :bool)
      <<1>>
      iex> to_bytes(23, :u8)
      <<23>>
      iex> to_bytes(0x12345678, :u32)
      <<120, 86, 52, 18>>
      iex> to_bytes(1, :f32)
      <<0, 0, 128, 63>>
  """
  @spec to_bytes(
          serializable_type
          | %Association.NotLoaded{}
          | %Association.Primitive{}
          | Schema.schema_struct(),
          datatype
        ) :: serializable_bytes
  def to_bytes(%Association.NotLoaded{__meta__: %Metadata{addr: addr}}, ptr_type),
    do: to_bytes(addr, ptr_type)

  def to_bytes(%Association.Primitive{__meta__: %Metadata{addr: addr}}, ptr_type),
    do: to_bytes(addr, ptr_type)

  def to_bytes(%{__meta__: %Metadata{assoc_type: :pointer, addr: addr}}, ptr_type)
      when is_atom(ptr_type) do
    to_bytes(addr, ptr_type)
  end

  def to_bytes(%{__meta__: %Metadata{}} = struct, mod) when is_atom(mod) do
    check_mod mod do
      mod.to_binary(struct)
    end
  end

  def to_bytes(true, :bool), do: <<1>>
  def to_bytes(false, :bool), do: <<0>>

  def to_bytes(value, :bool) when value in @u8_range,
    do: <<if(value != 0, do: 1, else: 0)::unsigned-native-integer-size(8)>>

  def to_bytes(value, :byte) when value in @u8_range,
    do: <<value::unsigned-native-integer-size(8)>>

  def to_bytes(value, :u8) when value in @u8_range, do: <<value::unsigned-native-integer-size(8)>>
  def to_bytes(value, :i8) when value in @i8_range, do: <<value::signed-native-integer-size(8)>>

  def to_bytes(value, :u16) when value in @u16_range,
    do: <<value::unsigned-native-integer-size(16)>>

  def to_bytes(value, :i16) when value in @i16_range,
    do: <<value::signed-native-integer-size(16)>>

  def to_bytes(value, :u32) when value in @u32_range,
    do: <<value::unsigned-native-integer-size(32)>>

  def to_bytes(value, :i32) when value in @i32_range,
    do: <<value::signed-native-integer-size(32)>>

  def to_bytes(value, :f32), do: <<value::native-float-size(32)>>

  def to_bytes(value, :u64) when value in @u64_range,
    do: <<value::unsigned-native-integer-size(64)>>

  def to_bytes(value, :i64) when value in @i64_range,
    do: <<value::signed-native-integer-size(64)>>

  def to_bytes(value, :f64), do: <<value::native-float-size(64)>>

  # Compile-time generation of :word clauses based on configured word_type
  case @word_type do
    :u32 ->
      def to_bytes(value, :word) when value in @u32_range,
        do: <<value::unsigned-native-integer-size(32)>>

    :u64 ->
      def to_bytes(value, :word) when value in @u64_range,
        do: <<value::unsigned-native-integer-size(64)>>

    other ->
      raise "Invalid word_type config: #{inspect(other)}"
  end

  def to_bytes(binary, {:byte, n}) do
    len = byte_size(binary)
    unless len == n, do: raise(ArgumentError, "Expected #{n} bytes, got #{len}")

    binary
  end

  def to_bytes(str, {:string_buffer, n}) when is_binary(str) do
    len = byte_size(str)

    if len > n do
      raise ArgumentError,
            "String (#{len} bytes) too large for {:string_buffer, #{n}}"
    else
      str <> :binary.copy(<<0>>, n - len)
    end
  end

  def to_bytes(list, {subtype, count}) when is_list(list) do
    list_len = length(list)

    unless list_len == count,
      do:
        raise(
          ArgumentError,
          "Expected list with #{count} elements, got one with #{list_len} elements"
        )

    list
    |> Enum.map(&to_bytes(&1, subtype))
    |> Enum.reduce(&(&2 <> &1))
  end

  def to_bytes(_, type) when is_atom(type) do
    check_mod type do
      raise ArgumentError, "Invalid argument #{type} (WTF?)"
    end
  end
end
