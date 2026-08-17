defmodule CppStringTest do
  use ExUnit.Case

  alias Rekto.Schema.Metadata
  alias Rekto.Association.NotLoaded
  alias Rekto.VCPP

  @moduletag :capture_log

  @str_bin "Hello"
  @str_charlist ~c"Hello"
  @str_len 5
  @str_cap 15

  #         | H   e    l    l    o    \0
  @str_data <<72, 101, 108, 108, 111, 0>> <>
              (List.duplicate(0, 10) |> :erlang.list_to_binary()) <>
              <<5, 0, 0, 0>> <>
              <<15, 0, 0, 0>>
  @correct_struct %VCPP.StdString.Short{
    __meta__: %Metadata{
      addr: nil,
      assoc_type: :none
    },
    buffer: @str_bin <> String.duplicate(<<0>>, 11),
    length: @str_len,
    capacity: @str_cap
  }

  @filler_byte 0xFF
  @long_str_data <<0x78, 0x56, 0x34, 0x12>> <>
                   (List.duplicate(@filler_byte, 12) |> :erlang.list_to_binary()) <>
                   <<23, 0, 0, 0>> <>
                   <<31, 0, 0, 0>>
  @correct_long %VCPP.StdString.Long{
    __meta__: %Rekto.Schema.Metadata{
      addr: nil,
      assoc_type: :none
    },
    buffer: %NotLoaded{
      __meta__: %Rekto.Schema.Metadata{
        addr: 0x12345678,
        assoc_type: :pointer
      },
      data_type: {:string_buffer, 23}
    },
    length: 23,
    capacity: 31
  }

  def string_fixture do
    @str_data |> VCPP.StdString.from_binary() |> elem(1)
  end

  def long_string_fixture do
    @long_str_data |> VCPP.StdString.from_binary() |> elem(1)
  end

  test "string creation" do
    assert string_fixture() == @correct_struct
  end

  test "string to charlist" do
    assert string_fixture() |> to_charlist() == @str_charlist
  end

  test "string to binary" do
    assert string_fixture() |> to_string() == @str_bin
  end

  test "long string" do
    long = long_string_fixture()

    assert long == @correct_long
  end

  test "failed transformation" do
    fail_str =
      (String.duplicate(<<0>>, 16) <> <<5, 0, 0, 0>> <> <<15, 0, 0, 0>>)
      |> VCPP.StdString.from_binary()

    assert {:error, [{{VCPP.StdString, :determine_type}, err_type}]} = fail_str

    assert err_type == [
             {{VCPP.StdString.Validation, :check_short_buffer}, {:embedded_null_bytes, 5}}
           ]
  end
end
