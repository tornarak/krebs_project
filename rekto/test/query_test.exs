defmodule QueryTest do
  use ExUnit.Case, async: true

  alias Rekto.Query

  defmodule SimpleSchema do
    use Rekto.Schema

    schema do
      field(:hp, :u32)
      field(:mana, :u16)
      points_to(:next, __MODULE__)
    end
  end

  describe "Query.from/2" do
    test "creates a valid query" do
      q = Query.from(SimpleSchema, where: [hp: 100])
      assert q.schema == SimpleSchema
      assert q.where == [hp: 100]
      assert q.filter == nil
      assert q.preload == nil
    end

    test "accepts multiple where fields" do
      q = Query.from(SimpleSchema, where: [hp: 100, mana: 50])
      assert q.where == [hp: 100, mana: 50]
    end

    test "accepts preload option" do
      q = Query.from(SimpleSchema, where: [hp: 1], preload: [:next])
      assert q.preload == [:next]
    end

    test "accepts filter option" do
      filter = fn s -> s.hp > 50 end
      q = Query.from(SimpleSchema, where: [hp: 1], filter: filter)
      assert q.filter == filter
    end

    test "raises when :where is missing" do
      assert_raise ArgumentError, fn ->
        Query.from(SimpleSchema, filter: fn _ -> true end)
      end
    end

    test "raises when :where is empty" do
      assert_raise ArgumentError, fn ->
        Query.from(SimpleSchema, where: [])
      end
    end

    test "raises on unknown option" do
      assert_raise ArgumentError, fn ->
        Query.from(SimpleSchema, where: [hp: 1], bogus: :opt)
      end
    end

    test "raises when where field is not in schema" do
      assert_raise ArgumentError, fn ->
        Query.from(SimpleSchema, where: [nonexistent: 0])
      end
    end

    test "raises when preload field is not a pointer" do
      assert_raise ArgumentError, fn ->
        Query.from(SimpleSchema, where: [hp: 1], preload: [:hp])
      end
    end
  end
end
