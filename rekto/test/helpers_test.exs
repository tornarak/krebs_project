defmodule HelpersTest do
  use ExUnit.Case, async: true
  doctest Rekto.Schema.Helpers, import: true

  alias Rekto.Schema.Helpers
  alias Rekto.ExampleStruct
  alias Rekto.ExampleStruct.SmolStruct

  defp dup_bytes(b, n) do
    List.duplicate(b, n) |> :erlang.list_to_binary()
  end

  defp nulls(n) do
    dup_bytes(0x00, n)
  end

  defp ex_fields_mask do
    dup_bytes(0xFF, 4) <> nulls(8) <> dup_bytes(0xFF, 8) <> nulls(ExampleStruct.__size__() - 20)
  end

  test "fields_mask/2" do
    assert Helpers.mask_fields(ExampleStruct, [:vftable, :even_thing]) == ex_fields_mask()
  end

  test "patch_binary/3" do
    assert Helpers.patch_binary(<<1, 2, 3, 4>>, <<0, 0>>, 0) == <<0, 0, 3, 4>>
    assert Helpers.patch_binary(<<1, 2, 3, 4>>, <<0, 0>>, 1) == <<1, 0, 0, 4>>
    assert Helpers.patch_binary(<<1, 2, 3, 4>>, <<0, 0>>, 2) == <<1, 2, 0, 0>>

    assert Helpers.patch_binary(<<1, 2, 3, 4, 5, 6, 7, 8>>, <<2, 3, 6, 8>>, 0x04) ==
             <<1, 2, 3, 4, 2, 3, 6, 8>>
  end

  test "to_masked_bytes/2" do
    assert Helpers.to_masked_bytes(SmolStruct, int1: 3) ==
             {<<3, 0, 0, 0>> <> nulls(4), dup_bytes(0xFF, 4) <> nulls(4)}

    # Now this is what I call a "pro gamer move"
    assert Helpers.to_masked_bytes(ExampleStruct,
             vftable: 0xFFFFFFFF,
             even_thing: 0xFFFFFFFFFFFFFFFF
           ) ==
             {ex_fields_mask(), ex_fields_mask()}
  end
end
