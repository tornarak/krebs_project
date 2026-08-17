defmodule Rekto.ExampleStruct.SmolStruct do
  @moduledoc false

  use Rekto.Schema

  schema do
    field(:int1, :u32)
    field(:int2, :u32)
  end
end

defmodule Rekto.ExampleStruct do
  @moduledoc false

  use Rekto.Schema

  alias Rekto.ExampleStruct
  alias Rekto.ExampleStruct.SmolStruct
  alias Rekto.Schema.Helpers

  # Sanity checks check the whole struct, as opposed to constraints,
  # which restrict the values of individual fields.
  #
  # Any function from any module can be used as a sanity check, provided
  # that it takes a single map/struct as input.
  #
  # This code is identical to `sanity __MODULE__, :exe_before_heap`.
  sanity(:exe_before_heap)

  def even(num), do: rem(num, 2) == 0

  schema do
    points_to(:vftable, :u32, in_exe: true)
    # `epicness` must be in [0, 69) ++ (69, 100]
    field(:epicness, :u64, constraints: [range: {0, 100}, not_const: 69])
    # The `func` constraint accepts a named function.
    # Due to Elixir's limitations, it will not work with anonymous functions.
    field(:even_thing, :u64, constraints: [func: &__MODULE__.even/1])
    # You can also embed other Rektos
    field(:lil_boy, SmolStruct)
    # Fields are usually arranged one after the other.
    # If an offset is explicitly specified, the byte representation will be expanded to fit all fields.
    # Any data outside of a field is ignored.
    points_to(:parent, __MODULE__, offset: 0x50, in_heap: true)
  end

  # The struct is defined by the `schema` macro.
  # (This totally screws up Dialyzer)
  # Trying to define a struct method prior to invoking the macro will result in a compile-time error.
  def johnny_johnny(%ExampleStruct{parent: papa}) do
    "Yes, #{inspect(papa)}?"
    # "Strangling prostitutes?"
    # "No, #{inspect papa}."
    # "Telling lies?"
    # "... No. #{inspect papa}..."
    # "Open the crawlspace. Now."
    # *gunshot*
    # "Ha Ha Ha!"
  end

  # This library was initially developed as part of a high-level abstraction for game hacking.
  # The structs that make up your average game state usually contain pointers to both the heap
  # (most things allocated at runtime) and the executable image (vftables and global variables).
  #
  # In the Windows memory layout, the executable image is in one contiguous block near the beginning
  # of the beginning, while thread stacks and heaps are allocated later on. For this reason, it is
  # almost always an invariant that all image addresses will be lower than heap addresses.
  #
  # This sanity check compares all struct fields with the custom options `:in_exe` and `:in_heap`.
  # There is no library support for these options; they are implemented solely in this module.
  @spec exe_before_heap(Rekto.Schema.schema_struct()) :: :ok | {:error, String.t()}
  def exe_before_heap(%{__struct__: _mod} = struct) do
    # descending order
    exe_ptrs =
      struct
      |> Helpers.get_fields_with_opt(:in_exe)
      |> Enum.map(&elem(&1, 2))
      |> Enum.sort(&Kernel.>=/2)

    # ascending order
    heap_ptrs =
      struct
      |> Helpers.get_fields_with_opt(:in_heap)
      |> Enum.map(&elem(&1, 2))
      |> Enum.sort(&Kernel.<=/2)

    case {exe_ptrs, heap_ptrs} do
      {[biggest_exe | _t0], [smallest_heap | _t1]} ->
        if biggest_exe <= smallest_heap,
          do: :ok,
          else:
            {:error,
             "Largest executable pointer (#{Helpers.print_ptr(biggest_exe)}) > smallest heap pointer (#{
               Helpers.print_ptr(smallest_heap)
             })"}

      _ ->
        # One or both lists are empty
        :ok
    end
  end
end
