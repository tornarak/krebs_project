defmodule MemoTableTest do
  use ExUnit.Case, async: true

  alias Rekto.MemoTable
  alias Rekto.Schema.Metadata

  setup do
    {:ok, server} = MemoTable.start_link()
    tid = MemoTable.tid(server)
    {:ok, tid: tid}
  end

  # Schema used for struct-level tests — defined once at module level.
  defmodule MemoSchema do
    use Rekto.Schema

    schema do
      field(:vtable, :u32, memoize: true)
      field(:other, :u32)
    end
  end

  describe "check_and_set/4" do
    test "stores value on first encounter and returns :ok", %{tid: tid} do
      assert :ok == MemoTable.check_and_set(tid, SomeMod, :field, 42)
    end

    test "returns :ok when value matches stored value", %{tid: tid} do
      MemoTable.check_and_set(tid, SomeMod, :field, 42)
      assert :ok == MemoTable.check_and_set(tid, SomeMod, :field, 42)
    end

    test "returns memoize_conflict error on mismatch", %{tid: tid} do
      MemoTable.check_and_set(tid, SomeMod, :field, 42)

      assert {:error, {:memoize_conflict, 42, 99}} =
               MemoTable.check_and_set(tid, SomeMod, :field, 99)
    end

    test "tracks keys per (module, field) pair independently", %{tid: tid} do
      assert :ok == MemoTable.check_and_set(tid, ModA, :field, 1)
      assert :ok == MemoTable.check_and_set(tid, ModB, :field, 2)
      assert :ok == MemoTable.check_and_set(tid, ModA, :field, 1)
      assert {:error, _} = MemoTable.check_and_set(tid, ModA, :field, 99)
    end
  end

  describe "clear/1 and clear/3" do
    test "clear/1 removes all entries, allowing new values", %{tid: tid} do
      MemoTable.check_and_set(tid, SomeMod, :a, 1)
      MemoTable.check_and_set(tid, SomeMod, :b, 2)
      MemoTable.clear(tid)
      assert :ok == MemoTable.check_and_set(tid, SomeMod, :a, 99)
    end

    test "clear/3 removes a specific field, leaving others intact", %{tid: tid} do
      MemoTable.check_and_set(tid, SomeMod, :a, 1)
      MemoTable.check_and_set(tid, SomeMod, :b, 2)
      MemoTable.clear(tid, SomeMod, :a)
      assert :ok == MemoTable.check_and_set(tid, SomeMod, :a, 99)
      assert {:error, _} = MemoTable.check_and_set(tid, SomeMod, :b, 99)
    end
  end

  describe "get_all/1" do
    test "returns all stored entries as {{mod, field}, value} tuples", %{tid: tid} do
      MemoTable.check_and_set(tid, SomeMod, :x, 7)
      MemoTable.check_and_set(tid, SomeMod, :y, 8)
      all = MemoTable.get_all(tid) |> Enum.sort()
      assert all == [{{SomeMod, :x}, 7}, {{SomeMod, :y}, 8}]
    end
  end

  describe "check_struct_with_meta/1" do
    test "no-op (returns :ok) when meta.memo is nil" do
      {:ok, struct} = MemoSchema.from_binary(<<1, 0, 0, 0, 2, 0, 0, 0>>)
      assert :ok == MemoTable.check_struct_with_meta(struct)
    end

    test "stores memoized fields on first call via from_binary", %{tid: tid} do
      meta = %{addr: nil, assoc_type: :none, memo: tid}
      assert {:ok, _} = MemoSchema.from_binary(<<1, 0, 0, 0, 99, 0, 0, 0>>, meta)
      # vtable=1 is now stored; a second from_binary with same vtable also passes
      assert {:ok, _} = MemoSchema.from_binary(<<1, 0, 0, 0, 100, 0, 0, 0>>, meta)
    end

    test "from_binary rejects a conflicting memoized value", %{tid: tid} do
      meta = %{addr: nil, assoc_type: :none, memo: tid}
      {:ok, _} = MemoSchema.from_binary(<<1, 0, 0, 0, 99, 0, 0, 0>>, meta)
      # vtable=2 conflicts with stored vtable=1 — from_binary returns an error
      assert {:error, _} = MemoSchema.from_binary(<<2, 0, 0, 0, 100, 0, 0, 0>>, meta)
    end

    test "check_struct/2 returns conflict error on directly-constructed struct", %{tid: tid} do
      # Seed the table with vtable=1
      MemoTable.check_and_set(tid, MemoSchema, :vtable, 1)
      # Build a struct that bypasses from_binary, giving it vtable=2
      meta = struct(Metadata, %{addr: nil, assoc_type: :none, memo: tid})
      conflict_struct = struct(MemoSchema, %{__meta__: meta, vtable: 2, other: 0})

      assert {:error, [{:vtable, {:memoize_conflict, 1, 2}}]} =
               MemoTable.check_struct(tid, conflict_struct)
    end
  end
end
