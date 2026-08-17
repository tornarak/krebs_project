# Rekto

Bridging the gap between two kinds of struct.

## Motivation

The history of computing is marked by an eternal struggle: Hardware developers put a lot of work into making reliable, extremely performant computers, only for us software developers to squander it by building our applications in slower, trendier languages, and filling every nook and cranny with ads and spyware.

I wanted to create an in-memory database in Elixir, but I realized that I had no idea how to, and the Wikipedia pages on the relevant [data structures](https://en.wikipedia.org/wiki/B-tree) and [concepts](https://en.wikipedia.org/wiki/Journaling_file_system) made my head hurt. When faced with an unfamiliar problem that they have no idea how to solve, a web developer's natural impulse is to search for a good dependency - an innovative, cutting-edge solution that will inevitably be a massive drain on both runtime and programmer performance - and this is exactly what I did.

I was extremely disappointed to find that Elixir has a mature and extremely performant in-memory database built into the language - in fact, it has three of [the](http://erlang.org/doc/man/ets.html) [darn](http://erlang.org/doc/man/dets.html) [things](http://erlang.org/doc/man/mnesia.html)!

Then I had an epiphany: while I didn't know how to efficiently store data, there were other applications running on my computer whose authors probably did. Maybe I could just... repurpose their data! After all, isn't that, like, the whole point of Erlang, man? All these applications working together in harmony?

## What Is It?

Rekto is kind of like [Ecto](https://hexdocs.pm/ecto), but instead of running your queries against a database, you run them against a byte buffer. It allows you to declare "schemas" that map to both Elixir structs and byte sequences. The library is intended to be used with [libkrebs](https://github.com/tornarak/krebs_project/tree/master/libkrebs) to find structs in other processes' memory, but could probably be used for other things, like files.

The name "Rekto" is an amalgamation of the [Ecto ORM](https://hexdocs.pm/ecto/Ecto.html) (on which it is based), the Urdu word ["rekhta"](https://en.wikipedia.org/wiki/Rekhta), meaning "coarse mixture" (alluding to the structure of memory), and the gaming slang term ["get rekt"](https://www.urbandictionary.com/define.php?term=get%20rekt).

*... I should have just done a TensorFlow tutorial like every other kid in my high school.*

## Installation

`rekto` has no native dependencies — pure Elixir, no NIF to build.

```elixir
def deps do
  [
    {:rekto, path: "../rekto"}
  ]
end
```

Set the platform pointer width once in your app's config (defaults to `:u32` if omitted):

```elixir
# config/config.exs
config :rekto, :word_type, :u64   # or :u32
```

`:word_type` is read at compile time, so changing it requires a recompile
(`mix deps.compile rekto --force` if you're consuming it as a dependency).

## Usage

```bash
mix deps.get
mix compile
mix test
```

Rekto is pure — it maps binaries to structs and back, with no I/O of its own. It's meant
to be paired with something that hands it real bytes from a live process, like
[`krebs`](https://github.com/tornarak/krebs_project/tree/master/krebs)'s `Krebs.Repo`. See
[`../SCHEMAS.md`](../SCHEMAS.md) for the full field-type and constraint reference, or the
[`schema()` section below](#schemas) for a from-scratch walkthrough.

# Schemas

Rekto schemas are very similar to Ecto schemas. Here's the [Visual C++ implementation](https://shaharmike.com/cpp/std-string/) of std::string, for strings less than 16 chararters long:

```elixir
defmodule Rekto.VCPP.StdString.Short do
  @moduledoc """
  Visual C++ [std::string](https://www.cplusplus.com/reference/string/string/)
  with a length under 16 characters. The string data is stored in an internal buffer.
  """

  use Rekto.Schema

  alias Rekto.VCPP.StdString.Short

  @type t :: %__MODULE__{
          buffer: binary,
          length: 0..15,
          capacity: 15
        }

  schema do
    field(:buffer, {:string_buffer, 16})
    field(:length, :u32, constraints: [non_null: true, range: {0, 15}])
    field(:capacity, :u32, constraints: [non_null: true, const: 15])
  end

  defimpl String.Chars do
    # `:buffer` is a fixed-size raw binary, null-padded — trim it to `:length`.
    def to_string(%Short{buffer: buf, length: len}), do: binary_part(buf, 0, len)
  end
end
```

```elixir
iex> alias Rekto.VCPP.StdString.Short
Rekto.VCPP.StdString.Short

iex> bin_str = (:erlang.list_to_binary('hello')
...> <> String.duplicate(<<0>>, 11)
...> <> <<5, 0, 0, 0>>
...> <> <<15, 0, 0, 0>>)
<<104, 101, 108, 108, 111, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 5, 0, 0, 0, 15, 0,
  0, 0>>

iex> Short.from_binary(bin_str)
{:ok,
 %Rekto.VCPP.StdString.Short{
   __meta__: #Rekto.Schema.Metadata<nil, :none>,
   capacity: 15,
   length: 5,
   buffer: <<104, 101, 108, 108, 111, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0>>
 }}

iex> to_string(elem(Short.from_binary(bin_str), 1))
"hello"
```

Fields are specified by a name, type, and optional flags or constraints. Types can be split into three categories:

* A subset of [Rust primitives](https://doc.rust-lang.org/std/index.html#primitives), expressed as atoms (the precursor to Rekto was written in Rust).
* Other schemas, expressed as module names. If a schema is supplied as a regular field type, it will be embedded in the containing schema.
* Arrays, expressed as {type, size}.

```elixir
@type byte_primitive :: :bool | :byte | :u8 | :i8
@type word_primitive :: :u16 | :i16
@type dword_primitive :: :u32 | :i32 | :f32
@type qword_primitive :: :u64 | :i64 | :f64
@type primitive :: byte_primitive | word_primitive | dword_primitive | qword_primitive
@type datatype :: primitive | Schema.t() | {datatype, pos_integer}
```

Fields are arranged one after the other by default. You can specify an offset with the `:offset` option to leave empty space between that field and the previous field; the unused bytes will be discarded. Fields *must* be declared in increasing order of offset; otherwise, a compile-time error will be raised. Overlapping fields will result in an error as well.

```elixir
iex> defmodule TinyStruct do
...>   use Rekto.Schema
...>  
...>   schema do
...>     # my prized 32-bit integer. it has a lot of sentimental value.
...>     field :my_one_int, :u32
...>   end
...> end
iex> defmodule BigStruct do
...>   use Rekto.Schema
...> 
...>   schema do
...>     # mmm... structs
...>     field :i_eat_you, TinyStruct 
...>       
...>     # 12 bytes of dead space
...> 
...>     field :other_int, :u32, offset: 16
...>   end
...> end
iex> BigStruct.from_binary(<<5, 0, 0, 0>> <> String.duplicate(<<0xCC>>, 12) <> <<3, 0, 0, 0>>)
{:ok,
 %BigStruct{
   __meta__: #Rekto.Schema.Metadata<nil, :none>,
   other_int: 3,
   i_eat_you: %TinyStruct{
     __meta__: #Rekto.Schema.Metadata<nil, :embed>,
     my_one_int: 5
   }
 }}
```

Pointers are a special kind of field. Here's the implementation of std::string when the string is longer than 16 characters - the internal buffer is replaced with a pointer to an external buffer on the heap.

```elixir
defmodule Rekto.VCPP.StdString.Long do
  @moduledoc """
  Visual C++ [std::string](https://www.cplusplus.com/reference/string/string/)
  with a length greater than or equal to 16 characters. The string data is stored
  on the heap, and the `std::string` itself contains a pointer to this data.
  """

  use Rekto.Schema

  alias Rekto.VCPP.StdString.Long

  schema do
    # Rekto.Void is a special type for pointers whose types are dynamically
    # decided (more on that later). Attempting to preload one will raise an exception.
    # The default type for a pointer is :u32.
    points_to(:buffer, Rekto.Void)
    field(:length, :u32, offset: 16, constraints: [range: {16, 1_000_000}])
    field(:capacity, :u32, constraints: [range: {16, 1_000_000}])
  end
end
```

Under the hood, pointers are just regular `:u32` or `:u64` fields with an internal `:points_to` option switched on. The default pointer type is whatever `:word_type` is configured as (`:u32` if you haven't set it — see [Installation](#installation)). You can change the type of an individual pointer with `:ptr_type`:

```elixir
points_to :buffer, Rekto.Void, ptr_type: :u64
```

# Serialization Pipeline

When you call `from_binary/1` on a binary to convert it into a struct, it goes through several steps before returning the end result. Here's a rough outline:

```elixir
binary
|> set_struct_fields()
|> check_constraints()
|> run_sanity_checks()
|> map_pointers_to_assocs()
|> run_transformations()
```

`set_struct_fields` is self-explanatory: It takes parts of the binary and converts them to struct fields. The fields are then subjected to individual constraint checks followed by sanity checks. These will be elaborated upon later in this guide; for now, all you need to know is that these functions can return an error, which will be propagated up through the call stack.

`map_pointers_to_assocs` converts raw pointer values (i.e: `0xDEADBEEF`) into `Rekto.Association.NotLoaded` structs (i.e: `#Rekto.Association.NotLoaded<Rekto.Void @ 0xDEADBEEF>`). The final step in the pipeline is `run_transformations`, which executes some user-specified transformations (more on that soon).

# Integrity Checks

## Options and Constraints

Options are all fields passed after the name and type. You can create your own options for use in sanity checks and such (we'll get to that later). The following options are reserved for special use by Rekto:

* `:constraints`
* `:offset`
* `:points_to`
* `:ptr_type`

Constraints are "limiters" on field values. If a field's value does not match all of its constraints, `from_binary/1` will return an error.

```elixir
iex> bin_str = :erlang.list_to_binary('hello') 
  <> (List.duplicate(0, 11) |> :erlang.list_to_binary) 
  <> <<127, 0, 0, 0>> 
  <> <<255, 0, 0, 0>>
<<104, 101, 108, 108, 111, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 127, 0, 0, 0, 255,
  0, 0, 0>>
iex> Short.from_binary(bin_str)
{:error,
 [capacity: {:expected_const, 15, 255}, length: {:out_of_range, 127, 0, 15}]}
```

Here's the constraint [typespec](https://hexdocs.pm/elixir/typespecs.html):

```elixir
# oops i accidentally invented lisp

@type valid_return :: true | false | :ok | :error | {:ok, any} | {:error, any}
@type field_constraint ::
        :non_null
        | {:const, any}
        | {:not_const, any}
        | {:range, {integer | float, integer | float}}
        | {:func, (any -> valid_return)}
        | {:not, field_constraint}
        | {:and, {field_constraint, field_constraint}}
        | {:or, {field_constraint, field_constraint}}
        | {:xor, {field_constraint, field_constraint}}
```

The `{:func, function}` constraint allows you to create your own arbitary constraints:

```elixir
def even(num), do: rem(num, 2) == 0

schema do
  field(:even_thing, :u64, constraints: [func: &__MODULE__.even/1])
end
```

Due to a limitation of Elixir, only named functions can be used as `:func` constraints.

## Sanity Checks

Whereas constraints restrict a single field, sanity checks check the entire struct and accept or reject it based on the relationships between the fields. 

If the function head is not qualified with `%{__struct__: [...]}`, sanity checks will also run on values supplied to a query.

Here's a classic example from the field of game hacking — this one's the real thing, straight
out of [`lib/rekto/example_struct.ex`](lib/rekto/example_struct.ex):

```elixir
defmodule Rekto.ExampleStruct do
  use Rekto.Schema

  alias Rekto.ExampleStruct
  alias Rekto.Schema.Helpers

  sanity :exe_before_heap

  schema do
    points_to :vftable, :u32, in_exe: true
    points_to :parent, __MODULE__, offset: 0x50, in_heap: true
  end

  # This library was initially developed as part of a high-level abstraction for game hacking.
  # The structs that make up your average game state usually contain pointers to both the heap
  # (most things allocated at runtime) and the executable image (vftables and global variables).
  #
  # In the Windows memory layout, the executable image is in one contiguous block near the beginning
  # of virtual memory, while thread stacks and heaps are allocated later on. For this reason, it is
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
```

Note that `exe_before_heap/1` does not specifically operate on the `ExampleStruct`. This allows you to use it on any Rekto schema where fields have the options `:in_exe` and `:in_heap`.

# Insanity

## Transformations

The final step in the serialization pipeline is the transformation stage. Transformations are pure functions that take in a struct and output an updated struct. They're useful for when you want to automatically convert a raw field into something else. Here's an example:

```elixir
defmodule Rekto.VCPP.StdString.Long do
  use Rekto.Schema

  alias Rekto.VCPP.StdString.Long

  transform :buffer_length

  schema do
    points_to(:buffer, Rekto.Void)
    field(:length, :u32, offset: 16, constraints: [range: {16, 1_000_000}])
    field(:capacity, :u32, constraints: [range: {16, 1_000_000}])
  end

  def buffer_length(
        %Long{length: len, buffer: %Rekto.Association.NotLoaded{} = buf_assoc} = str
      ) do
    # Update the `buffer` association to be the correct length
    %Long{str | buffer: %Rekto.Association.NotLoaded{buf_assoc | data_type: {:string_buffer, len}}}
  end
end
```

```elixir
iex> bin = <<0xEF, 0xBE, 0xAD, 0xDE>> <> <<0::12*8>> <> <<20::32-little>> <> <<20::32-little>>
iex> Rekto.VCPP.StdString.Long.from_binary(bin)
{:ok,
 %Rekto.VCPP.StdString.Long{
   __meta__: #Rekto.Schema.Metadata<nil, :none>,
   capacity: 20,
   length: 20,
   buffer: #Rekto.Association.NotLoaded<{:string_buffer, 20} @ 0xDEADBEEF>
 }}
```

`:buffer` started out an unloaded `Rekto.Void` association — the untyped placeholder for a
bare `points_to :buffer, Rekto.Void`, which would raise if you tried to preload it as-is.
After the transformation runs, it's retagged as `{:string_buffer, 20}`, so
`Krebs.Repo.preload/2` knows exactly how many bytes to read.

You can even output a different data type entirely. Here's a general `StdString` type that resolves to `Short` or `Long` depending on its length:

```elixir
defmodule Rekto.VCPP.StdString do
  use Rekto.Schema

  alias Rekto.VCPP.StdString
  alias Rekto.VCPP.StdString.Short
  alias Rekto.VCPP.StdString.Long

  transform :determine_type

  schema do
    field(:unk, {:u8, 16})
    field(:length, :u32, offset: 16, constraints: [range: {0, 1_000_000}])
    field(:capacity, :u32, constraints: [range: {15, 1_000_000}])
  end

  @spec determine_type(%StdString{}) :: Short.t() | Long.t()
  def determine_type(%StdString{length: len} = str) do
    if len < 16 do
      {:ok, short} =
        str
        |> StdString.to_binary()
        |> Short.from_binary(str.__meta__)

      short
    else
      {:ok, long} =
        str
        |> StdString.to_binary()
        |> Long.from_binary(str.__meta__)

      long
    end
  end
end
```

(`str.__meta__` is passed to the constructor to preserve address and association data.)

Transformations, especially when converting between struct types, can be a bit of a monkey patch, so use them sparingly.

## Extensions

Schemas can "extend" an existing schema, inheriting its fields, constraints, sanity checks, and
transformations, and placing its own fields after those. This is intended to emulate 
[C++'s implementation of inheritance](https://www.blackhat.com/presentations/bh-dc-07/Sabanal_Yason/Paper/bh-dc-07-Sabanal_Yason-WP.pdf).

```elixir
iex> defmodule Parent do
...>   use Rekto.Schema
...> 
...>   schema do
...>     field :daddys_int, :u32
...>     field :daddys_double, :f64
...>   end   
...> end
iex> defmodule Child do
...>   use Rekto.Schema
...> 
...>   schema extends: Parent do
...>     # starts at 0x0C, end of the Parent's fields
...>     field :juniors_field, :u32
...>     field :juniors_float, :f32              
...>   end
...> end
iex> Rekto.Schema.Helpers.get_field_names(Child) 
[:daddys_int, :daddys_double, :juniors_field, :juniors_float]
iex> Rekto.Schema.Helpers.get_field_offset!(Child, :juniors_field) 
12
```

Other config:

``` elixir
config :rekto, :schema_apps, [:my_app]
```

Apps whose modules will be scanned for schemas
in `SchemaRegistry`
