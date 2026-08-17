defmodule EatMe do
  use Rekto.Schema

  schema do
    field(:hi, :u32)
    field(:wee, :u16)
  end
end

defmodule Embedder do
  use Rekto.Schema

  schema do
    field(:hey_there, :u32)
    points_to(:nom, EatMe, offset: 0x08)
  end
end

defmodule PrimPtr do
  use Rekto.Schema

  schema do
    points_to(:int, :u32)
  end
end

defmodule RepoTest do
  use ExUnit.Case

  alias Rekto.VCPP
  alias Rekto.Repo

  alias Rekto.Association.{Primitive, NotLoaded}
  alias Rekto.Schema.Metadata

  @correct_string %VCPP.StdString.Long{
    __meta__: %Metadata{
      addr: 0x04,
      assoc_type: :none
    },
    buffer: %NotLoaded{
      __meta__: %Metadata{
        addr: 0x24,
        assoc_type: :pointer
      },
      data_type: {:string_buffer, 20}
    },
    length: 20,
    capacity: 31
  }

  @correct_embedder %Embedder{
    __meta__: %Metadata{
      addr: 0x38,
      assoc_type: :none
    },
    hey_there: 5,
    nom: %EatMe{
      __meta__: %Metadata{
        addr: 0x44,
        assoc_type: :pointer
      },
      hi: 69,
      wee: 10
    }
  }

  @correct_primptr %PrimPtr{
    __meta__: %Metadata{
      addr: 0x4C,
      assoc_type: :none
    },
    int: %Primitive{
      __meta__: %Metadata{
        addr: 0x50,
        assoc_type: :pointer
      },
      data_type: :u32,
      value: 3
    }
  }

  defmodule TestRepo do
    use Rekto.Repo

    @twelve_nulls String.duplicate(<<0>>, 12)
    @charlist List.duplicate(?a, 20)

    @embedder_bytes <<5, 0, 0, 0>> <>
                      <<0, 0, 0, 0>> <>
                      <<0x44, 0, 0, 0>>

    @eatme_bytes <<69, 0, 0, 0>> <>
                   <<10, 0>>

    @string_memory <<0, 0, 0, 0>> <>
                     <<0x24, 0, 0, 0>> <>
                     @twelve_nulls <>
                     <<20, 0, 0, 0>> <>
                     <<31, 0, 0, 0>> <>
                     <<0, 0, 0, 0, 0, 0, 0, 0>> <>
                     :erlang.list_to_binary(@charlist) <>
                     @embedder_bytes <>
                     @eatme_bytes <>
                     <<0, 0>> <>
                     <<0x50, 0, 0, 0>> <>
                     <<3, 0, 0, 0>>

    @impl Rekto.Repo
    def read_bytes(addr, num_bytes, _opts) do
      if addr + num_bytes <= byte_size(@string_memory) do
        {:ok,
         @string_memory
         |> binary_part(addr, num_bytes)}
      else
        {:error, "Invalid Address"}
      end
    end

    @impl Rekto.Repo
    def execute_query(_query, _opts \\ []), do: {:ok, []}
  end

  test "get/3" do
    assert {:ok, @correct_string} == Rekto.Repo.get(TestRepo, Rekto.VCPP.StdString, 0x04)
  end

  test "preload/3" do
    {:ok, %VCPP.StdString.Long{} = struct} = Repo.get(TestRepo, Rekto.VCPP.StdString, 0x04)

    %VCPP.StdString.Long{buffer: %Primitive{value: chars}} =
      Repo.preload(TestRepo, struct, :buffer)

    assert chars == String.duplicate("a", 20)
  end

  test "struct as assoc" do
    {:ok, vorer} = Repo.get(TestRepo, Embedder, 0x38, preload: :nom)

    assert vorer == @correct_embedder
  end

  test "primitive pointers" do
    {:ok, judgemental} = Repo.get(TestRepo, PrimPtr, 0x4C, preload: :int)

    assert judgemental == @correct_primptr
  end

  test "errors" do
    assert_raise MatchError, fn -> {:ok, _nothing} = Repo.get(TestRepo, Embedder, 0x104) end
  end
end
