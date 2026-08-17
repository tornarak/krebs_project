defmodule SanityTest do
  use ExUnit.Case, async: true

  defmodule HasThisPtr do
    use Rekto.Schema

    schema do
      field(:self_ptr, :this_pointer)
      field(:data, :u32)
    end
  end

  describe "this_pointer sanity check" do
    test "passes when field value matches struct address" do
      addr = 0x1234
      # self_ptr=0x1234 matches addr=0x1234
      bin = <<0x34, 0x12, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00>>
      assert {:ok, _} = HasThisPtr.from_binary(bin, %{addr: addr, assoc_type: :none})
    end

    test "fails when field value does not match struct address" do
      addr = 0x1234
      # self_ptr=0x5678 does not match addr=0x1234
      bin = <<0x78, 0x56, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00>>

      # Sanity check errors are wrapped as {{mod, func}, inner_errors}
      assert {:error, [{{Rekto.Schema.Sanity, :check_this_pointers}, inner}]} =
               HasThisPtr.from_binary(bin, %{addr: addr, assoc_type: :none})

      assert inner == [self_ptr: {:this_pointer_mismatch, 0x1234, 0, 0x5678}]
    end

    test "skips check when addr is nil" do
      # any self_ptr value is accepted when addr is unknown
      bin = <<0x78, 0x56, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00>>
      assert {:ok, _} = HasThisPtr.from_binary(bin, %{addr: nil, assoc_type: :none})
    end
  end
end
