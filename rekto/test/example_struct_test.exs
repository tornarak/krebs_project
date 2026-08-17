defmodule ExampleStructTest do
  use ExUnit.Case
  doctest Rekto.ExampleStruct
  doctest Rekto.Schema, except: [points_to: 3, sanity: 1]
  doctest Rekto.Schema.Constraints, import: true

  alias Rekto.Association
  alias Rekto.ExampleStruct
  alias Rekto.ExampleStruct.SmolStruct
  alias Rekto.Schema.Helpers
  alias Rekto.Schema.Metadata

  @filler_byte 0xCC

  @default_props %{
    __meta__: %Metadata{
      addr: nil,
      assoc_type: :none
    },
    vftable: %Association.NotLoaded{
      __meta__: %Rekto.Schema.Metadata{
        addr: 01,
        assoc_type: :pointer
      },
      data_type: :u32
    },
    epicness: 0,
    even_thing: 20,
    lil_boy: %SmolStruct{
      __meta__: %Metadata{
        addr: nil,
        assoc_type: :embed
      },
      int1: 30,
      int2: 40
    },
    parent: %Association.NotLoaded{
      __meta__: %Rekto.Schema.Metadata{
        addr: 50,
        assoc_type: :pointer
      },
      data_type: ExampleStruct
    }
  }

  @difficult_props %{
    __meta__: %Metadata{
      addr: nil,
      assoc_type: :none
    },
    vftable: %Association.Primitive{
      __meta__: %Metadata{
        addr: 01,
        assoc_type: :pointer
      },
      data_type: :u32,
      value: 32
    },
    epicness: 0,
    even_thing: 20,
    lil_boy: %SmolStruct{
      __meta__: %Metadata{
        addr: nil,
        assoc_type: :embed
      },
      int1: 30,
      int2: 40
    },
    parent: %ExampleStruct{
      __meta__: %Metadata{
        addr: 50,
        assoc_type: :pointer
      },
      vftable: %Association.NotLoaded{
        __meta__: %Metadata{
          addr: 01,
          assoc_type: :pointer
        },
        data_type: :u32
      },
      epicness: 0,
      even_thing: 20,
      lil_boy: %SmolStruct{
        __meta__: %Metadata{
          addr: nil,
          assoc_type: :embed
        },
        int1: 30,
        int2: 40
      },
      parent: %Association.NotLoaded{
        __meta__: %Metadata{
          addr: 50,
          assoc_type: :pointer
        },
        data_type: ExampleStruct
      }
    }
  }

  @default_as_bin <<01, 00, 00, 00>> <>
                    <<00, 00, 00, 00, 00, 00, 00, 00>> <>
                    <<20, 00, 00, 00, 00, 00, 00, 00>> <>
                    <<30, 00, 00, 00>> <>
                    <<40, 00, 00, 00>> <>
                    (@filler_byte |> List.duplicate(52) |> :erlang.list_to_binary()) <>
                    <<50, 00, 00, 00>>

  def struct_fixture(props \\ %{}) do
    struct(ExampleStruct, props |> Enum.into(@default_props))
  end

  test "generates struct" do
    assert struct_fixture()
  end

  test "from_binary/1" do
    assert ExampleStruct.from_binary(@default_as_bin) == {:ok, struct_fixture()}
  end

  test "to_binary/1" do
    assert struct_fixture() |> ExampleStruct.to_binary() == @default_as_bin
  end

  test "difficult to_binary/1" do
    assert struct_fixture(@difficult_props) |> ExampleStruct.to_binary() == @default_as_bin
  end

  test "rejects constraint violations" do
    vibe_check =
      struct_fixture(%{epicness: 69})
      |> Helpers.check_field_constraints()

    assert vibe_check == [epicness: {:unexpected_const, 69}]
  end

  test "sanity checks work" do
    # Set pointers to int values; sanity checks occur before assocs are created
    uber_vibe_check =
      struct_fixture(%{vftable: 100, parent: 50})
      |> Helpers.check_sanity()

    assert uber_vibe_check == [
             {{ExampleStruct, :exe_before_heap},
              "Largest executable pointer (0x00000064) > smallest heap pointer (0x00000032)"}
           ]
  end
end
