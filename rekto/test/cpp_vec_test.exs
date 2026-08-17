defmodule CppVecTest do
  use ExUnit.Case, async: true

  alias Rekto.VCPP
  alias Rekto.Schema.Metadata

  # arr_start=0x1000, last_addr=0x1010 (4 u32s), arr_end=0x1020
  @valid_vec_bin <<0x00, 0x10, 0x00, 0x00, 0x10, 0x10, 0x00, 0x00, 0x20, 0x10, 0x00, 0x00>>

  @correct_vec %VCPP.StdVector{
    __meta__: %Metadata{addr: nil, assoc_type: :none},
    arr_start: 0x1000,
    last_addr: 0x1010,
    arr_end: 0x1020
  }

  test "from_binary/1 parses a valid vector" do
    assert {:ok, @correct_vec} == VCPP.StdVector.from_binary(@valid_vec_bin)
  end

  test "to_binary/1 round-trips" do
    assert VCPP.StdVector.to_binary(@correct_vec) == @valid_vec_bin
  end

  test "rejects null arr_start" do
    # arr_start == 0 violates non_null constraint
    bin = <<0x00, 0x00, 0x00, 0x00, 0x10, 0x10, 0x00, 0x00, 0x20, 0x10, 0x00, 0x00>>
    assert {:error, _} = VCPP.StdVector.from_binary(bin)
  end

  test "rejects null last_addr" do
    # last_addr == 0 violates non_null constraint
    bin = <<0x00, 0x10, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x20, 0x10, 0x00, 0x00>>
    assert {:error, _} = VCPP.StdVector.from_binary(bin)
  end

  test "rejects out-of-order addresses (arr_start >= last_addr)" do
    # arr_start=0x1010, last_addr=0x1000 — violates array_order sanity check
    bin = <<0x10, 0x10, 0x00, 0x00, 0x00, 0x10, 0x00, 0x00, 0x20, 0x10, 0x00, 0x00>>
    assert {:error, _} = VCPP.StdVector.from_binary(bin)
  end

  test "get_byte_size/1 returns difference between last_addr and arr_start" do
    assert VCPP.StdVector.get_byte_size(@correct_vec) == 0x10
  end

  test "get_length/2 returns element count for a given type" do
    # 0x10 bytes / 4 bytes per u32 = 4 elements
    assert VCPP.StdVector.get_length(@correct_vec, :u32) == 4.0
  end

  test "Inspect renders address range" do
    str = inspect(@correct_vec)
    assert str =~ "Rekto.VCPP.StdVector"
    assert str =~ "0x00001000"
  end
end
