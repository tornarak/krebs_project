defmodule StringKiddy do
  use Rekto.Schema

  # This is my OC, teh epik Son of String!!!
  # DO NOT STEAL!!!
  schema extends: Rekto.VCPP.StdString.Short do
    field(:my_epic_int, :u32)
  end
end

defmodule InheritanceTest do
  use ExUnit.Case

  alias Rekto.VCPP
  alias Rekto.Serialization
  alias Rekto.Schema.{Helpers, Metadata}

  alias StringKiddy

  @as_bytes "Hello" <>
              String.duplicate(<<0>>, 11) <>
              <<5, 0, 0, 0>> <>
              <<15, 0, 0, 0>> <>
              <<69, 0, 0, 0>>

  @as_struct %StringKiddy{
    __meta__: %Metadata{
      addr: nil,
      assoc_type: :none
    },
    buffer: "Hello" <> String.duplicate(<<0>>, 11),
    length: 5,
    capacity: 15,
    my_epic_int: 69
  }

  test "right size" do
    assert Serialization.get_type_size!(StringKiddy) == 28
  end

  test "right offsets" do
    assert Helpers.get_field_offset!(StringKiddy, :my_epic_int) == 24
  end

  test "to_binary/1" do
    assert StringKiddy.to_binary(@as_struct) == @as_bytes
  end

  test "parent to_binary/1" do
    assert VCPP.StdString.Short.to_binary(@as_struct) == binary_part(@as_bytes, 0, 24)
  end
end
