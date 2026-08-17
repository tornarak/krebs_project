defmodule Rekto.Schema do
  @moduledoc """
  DSL for defining binary-deserialization structs with typed fields,
  constraints, sanity checks, and transformations.

  Heavily inspired by Ecto's schema DSL.

  See `Rekto.ExampleStruct` for usage examples, and `Rekto.Schema.Helpers`
  for introspection utilities.
  """

  alias Rekto.Schema.{ASTGen, Constraints, Helpers, Metadata}
  alias Rekto.Serialization

  @type t :: module

  @type pointer :: :u32 | :u64

  @type field_name :: atom
  @type opt_name :: atom
  @type schema_struct :: %{
          required(:__struct__) => Rekto.Schema.t(),
          required(:__meta__) => Metadata.t()
        }
  @type schema_struct(mod) :: %{
          required(:__struct__) => mod,
          required(:__meta__) => Metadata.t()
        }
  @type assoc_type :: :none | :embed | :pointer

  @doc """
  FOR INTERNAL USE ONLY
  """
  @callback __size__() :: pos_integer

  @doc """
  FOR INTERNAL USE ONLY
  """
  @callback __schema__() :: Rekto.Schema.Info.t()

  @doc """
  Converts a binary to an instance of the schema struct.
  Intentionally named to be distinct from `Rekto.Serialization.from_bytes/2`.

  This operation is not necessarily reversible by `c:to_binary/1`.

  The second parameter is a map or `t:Rekto.Schema.Metadata.t/0` struct. This
  is almost solely for internal use; the only really valid use case for a library user
  is to preserve metadata when converting one schema type to another, as with `Rekto.CppString`.
  """
  @callback from_binary(binary, map | Metadata.t()) :: {:ok, schema_struct} | {:error, any}

  @doc """
  Converts an instance of the schema struct to a binary.
  Intentionally named to be distinct from `Rekto.Serialization.to_bytes/2`.

  This operation is not necessarily reversible by `c:from_binary/2`.
  """
  @callback to_binary(map) :: binary

  @doc """
  Returns a byte mask that excludes the "garbage bytes" (empty space that doesn't
  belong to any field).

      iex> defmodule MySchema do
      ...>   use Rekto.Schema
      ...>
      ...>   schema do
      ...>     field :field1, :u32
      ...>     # 0x4C bytes of dead space
      ...>     field :field2, :u32, offset: 0x50
      ...>   end
      ...> end
      iex> (MySchema.garbage_mask()
      ...>  == (<<0xFF, 0xFF, 0xFF, 0xFF>>
      ...>    <> String.duplicate(<<0x00>>, 0x4C)
      ...>    <> <<0xFF, 0xFF, 0xFF, 0xFF>>))
      true
  """
  @callback garbage_mask() :: binary

  @callback custom_mask(map) :: binary

  @doc """
  Infers additional field values from user-provided field values for pattern scanning.

  Receives a map of user-specified field values and returns a map of inferred values.
  The framework enforces "pin" semantics: if the user explicitly provided a value for
  a field, the inferred value for that field is discarded. Implementors need not check
  for this — just return everything inferrable.

  Single pass: chains like `buffer -> length -> capacity` must be resolved in one call.
  """
  @callback pattern_inference(user_fields :: map()) :: map()

  @optional_callbacks [custom_mask: 1, pattern_inference: 1]

  @reserved_fields ~w[__meta__ __struct__ __garbage__]a
  @reserved_opts ~w[offset constraints points_to ptr_type]a

  @spec prelude(pos_integer, any) :: any
  defp prelude(line, block) do
    quote do
      def_line = Module.get_attribute(__MODULE__, :rekto_schema_line, nil)

      if def_line,
        do:
          raise(
            ArgumentError,
            "Schema for module #{__MODULE__} already defined on line #{def_line}"
          )

      @rekto_schema_line unquote(line)

      try do
        import Rekto.Schema
        unquote(block)
      after
        :ok
      end
    end
  end

  defmacro __using__(_) do
    %{module: mod} = __CALLER__

    Module.register_attribute(mod, :sanity_checks, accumulate: true)
    Module.register_attribute(mod, :transformations, accumulate: true)

    quote do
      import Rekto.Schema
    end
  end

  @doc """
  Returns true if the given module has a Rekto Schema.
  """
  # Dialyzer determines via call-graph analysis that this is always called with atoms,
  # making the catch-all clause unreachable. It remains for safety at API boundaries.
  @dialyzer {:nowarn_function, has_schema?: 1}
  @spec has_schema?(module) :: boolean
  def has_schema?(mod_name) when is_atom(mod_name) do
    if Module.open?(mod_name) do
      # Still being compiled, check module attributes
      Module.has_attribute?(mod_name, :rekto_schema)
    else
      # Already compiled, check beam info
      String.starts_with?(to_string(mod_name), "Elixir.") and
        Kernel.function_exported?(mod_name, :__info__, 1) and
        mod_name.__info__(:attributes) |> Keyword.get(:rekto_schema) == [true]
    end
  end

  def has_schema?(_), do: false

  @doc """
  Returns the schema info for the given module.

  The returned `Rekto.Schema.Info` struct contains all metadata about the schema including
  field definitions, constraints, sanity checks, transformations, and garbage mask.

  This replaces the need to call individual internal callbacks like `__fields__()`,
  `__field_constraints__()`, etc.
  """
  @spec get_schema(module) :: Rekto.Schema.Info.t()
  def get_schema(mod) when is_atom(mod) do
    mod.__schema__()
  end

  @doc """
  Returns a specific key from the schema info.

  Retrieves the value of that key from the Info struct
  (e.g., `:fields`, `:sanity_checks`, `:garbage_mask`, `:field_constraints`, etc.).
  """
  @spec get_schema(module, atom) :: any
  def get_schema(mod, key)
      when is_atom(mod) and is_atom(key) and
             key in [
               :fields,
               :fields_incl_garbage,
               :field_constraints,
               :sanity_checks,
               :transformations,
               :garbage_mask
             ] do
    mod |> get_schema() |> Map.get(key)
  end

  @doc """
  Declares a schema.

  ## Schema Inheritance

  If you pass an `:extends` option with another schema as the argument, the new schema
  will start off with all the fields, constraints, sanity checks, and transforms of the
  "parent" schema. All new fields will be declared after the fields of the parent class
  in the byte representation. This is intended to emulate
  [C++'s class inheritance model](https://www.blackhat.com/presentations/bh-dc-07/Sabanal_Yason/Paper/bh-dc-07-Sabanal_Yason-WP.pdf).
  Because they share all fields, methods intended for a base class can be used on a child
  class as well, provided that said methods match on a map instead of a struct. `c:to_binary/1`
  and `c:custom_mask/1` take maps as arguments for this reason.

  ```elixir
  defmodule IGotASchema do
    use Rekto.Schema

    schema do
      # ...
    end
  end
  """
  defmacro schema([do: _block] = do_block) do
    schema(__CALLER__, __MODULE__, do_block[:do])
  end

  defmacro schema([extends: extends], [do: _block] = do_block) do
    schema(__CALLER__, __MODULE__, do_block[:do], Macro.expand(extends, __CALLER__))
  end

  @doc false
  def schema(caller, _mod, block, extends \\ nil) do
    %{line: line, module: mod} = caller

    Module.register_attribute(mod, :fields, accumulate: true)
    Module.register_attribute(mod, :field_constraints, accumulate: true)
    Module.register_attribute(mod, :rekto_schema, persist: true)

    if extends do
      # unless has_schema?(extends), do:
      #   raise ArgumentError, "Can only extend a module with a schema (got #{inspect extends})"

      parent_info = extends.__schema__()

      Enum.each(parent_info.fields, &Module.put_attribute(mod, :fields, &1))
      Enum.each(parent_info.field_constraints, &Module.put_attribute(mod, :field_constraints, &1))
      Enum.each(parent_info.sanity_checks, &Module.put_attribute(mod, :sanity_checks, &1))
      Enum.each(parent_info.transformations, &Module.put_attribute(mod, :transformations, &1))
      Module.put_attribute(mod, :size, parent_info.size)
    end

    postlude =
      quote unquote: false do
        alias Rekto.Schema

        if length(@fields) < 1 do
          raise ArgumentError, "No fields defined"
        end

        [%{offset: last_offset, size: last_size} | _] = @fields
        struct_size = last_offset + last_size

        element_names = [:__meta__ | @fields |> Enum.map(fn %{name: name} -> name end)]

        @fields_in_offset_order @fields |> Enum.reverse()

        @fields_with_garbage @fields_in_offset_order
                             |> Enum.concat(get_garbage_fields(@fields_in_offset_order))
                             |> Enum.sort_by(fn %{offset: off} -> off end)

        @size struct_size

        @enforce_keys element_names
        defstruct element_names

        @behaviour Schema

        @rekto_schema true

        @impl Schema
        def __size__ do
          @size
        end

        @garbage_mask ASTGen.create_garbage_mask(@fields_with_garbage)

        _memoize_fields =
          Enum.filter(@fields_in_offset_order, fn %{opts: opts} ->
            Keyword.get(opts, :memoize, false)
          end)

        _this_ptr_fields =
          Enum.filter(@fields_in_offset_order, fn %{opts: opts} ->
            Keyword.get(opts, :this_pointer, false)
          end)

        _all_sanity_checks =
          if _this_ptr_fields != [],
            do: [{Rekto.Schema.Sanity, :check_this_pointers} | @sanity_checks],
            else: @sanity_checks

        _all_sanity_checks =
          if _memoize_fields != [],
            do: [{Rekto.MemoTable, :check_struct_with_meta} | _all_sanity_checks],
            else: _all_sanity_checks

        @schema_info %Rekto.Schema.Info{
          size: @size,
          fields: @fields_in_offset_order,
          fields_incl_garbage: @fields_with_garbage,
          field_constraints: @field_constraints,
          sanity_checks: Enum.reverse(_all_sanity_checks),
          transformations: Enum.reverse(@transformations),
          garbage_mask: @garbage_mask
        }

        @impl Schema
        def __schema__ do
          @schema_info
        end

        # Register at runtime if registry is available (for hot reload)
        try do
          Rekto.SchemaRegistry.register(__MODULE__)
        rescue
          ArgumentError -> :ok
        end

        @impl Schema
        def garbage_mask do
          @garbage_mask
        end

        bin_destruct_ast = ASTGen.bin_destruct_ast(@fields_with_garbage)
        fields_gen_ast = ASTGen.fields_gen_ast(@fields_in_offset_order)

        @impl Schema
        def from_binary(unquote(bin_destruct_ast), meta \\ %{addr: nil, assoc_type: :none}) do
          import Rekto.Serialization, only: [from_bytes: 2, from_bytes: 3]

          field_values = unquote(fields_gen_ast)

          with {:ok, raw_struct} <- Helpers.struct_from_values(__MODULE__, field_values, meta),
               [] <- Helpers.check_field_constraints(raw_struct),
               [] <- Helpers.check_sanity(raw_struct),
               a_struct <- Helpers.map_assocs(raw_struct),
               %{} = a_struct <- Helpers.apply_transformations(a_struct) do
            {:ok, a_struct}
          else
            err_list -> {:error, err_list}
          end
        end

        struct_destruct_ast = ASTGen.struct_destruct_ast(@fields_in_offset_order)
        bin_gen_ast = ASTGen.bin_gen_ast_as_list(@fields_with_garbage)

        @impl Schema
        def to_binary(unquote(struct_destruct_ast)) do
          import Rekto.Serialization, only: [to_bytes: 2]

          unquote(bin_gen_ast)
          |> Enum.reduce(&(&2 <> &1))
        end
      end

    quote do
      unquote(prelude(line, block))
      unquote(postlude)
    end
  end

  @doc false
  @spec get_garbage_fields([Rekto.Schema.FieldInfo.t()]) :: [Rekto.Schema.FieldInfo.t()]
  def get_garbage_fields(field_list) do
    garbage_fields(field_list, 0, [])
  end

  defp garbage_fields([], _, garbage_acc), do: garbage_acc

  defp garbage_fields([%{offset: next_offset, size: next_size} | tail], last_end, garbage_acc) do
    next_end = next_offset + next_size

    if next_offset != last_end do
      gap_size = next_offset - last_end

      garbage_fields(tail, next_end, [
        %Rekto.Schema.FieldInfo{
          name: :__garbage__,
          data_type: {:u8, gap_size},
          offset: last_end,
          size: gap_size,
          opts: [],
          points_to: nil
        }
        | garbage_acc
      ])
    else
      garbage_fields(tail, next_end, garbage_acc)
    end
  end

  @doc """
  Creates a field with the given name and type. The field will be added
  directly after the current struct's fields by default.

  ## Special type shortcuts

  * `:void_pointer` — a word-sized pointer to `Rekto.Void` (untyped target).
    Equivalent to `points_to name, Rekto.Void`.

  * `:vtable` — composite shorthand for a vtable pointer. Equivalent to a
    `:void_pointer` with `constraints: [non_null: true]`, `memoize: true`, and
    `in_module: true`. The first deserialized value is memoized via `Rekto.MemoTable`;
    subsequent instances must match.

  * `:this_pointer` — a word-sized field storing the struct's own address.
    Automatically injects a sanity check that compares the field value against
    `__meta__.addr` (skipped when addr is `nil`).

  ## Options

  * `:offset` - A custom offset for the field. Fields must still be declared in
  order of increasing offset, and cannot overlap with one another.
  * `:constraints` - A list of constraints for the given field. See
  `Rekto.Schema.Constraints` for more detail.
  * `:points_to` - The type that the value will point to. Don't pass this to
  the macro directly - use `points_to/3` instead.
  * `:pointer_offset` - Byte offset subtracted from a pointer's raw address during
  `Repo.preload` to obtain the base of the target struct. Also used when validating
  `:this_pointer` fields (expected value = `addr + offset`). Accepts:
    * an integer — used as-is
    * a field name atom — resolved at runtime to the byte offset of that field in the
      target schema (the same module for `:this_pointer` fields, or the `points_to`
      target for pointer fields). This lets you say "this pointer points to `:buffer`"
      instead of hardcoding a numeric offset.
  * `:memoize` - When `true`, the first deserialized value of this field is stored in
  `Rekto.MemoTable` and subsequent values must match. Applied automatically by `:vtable`.
  * `:in_module` - Informational opt marking the field as expected to point into the
  executable module region. Not enforced by rekto (enforcement lives in Krebs).
  * `:in_heap` - Informational opt marking the field as expected to point into the
  heap region. Not enforced by rekto (enforcement lives in Krebs).
  * Other options - will be stored along with the rest of the field metadata.
  Can be retrieved with `Rekto.Schema.Helpers.get_field_opt!/3`,
  `Rekto.Schema.Helpers.get_field_opts!/2`, and `Rekto.Schema.Helpers.get_fields_with_opt/2`.
  Useful when implementing custom behavior for multiple schemas.

  ## Examples

      iex> defmodule NiceSchema do
      ...>   use Rekto.Schema
      ...>
      ...>   schema do
      ...>     # 0x00 - 0x04 are discarded
      ...>     # 0x04 - 0x08
      ...>     field :int1, :u32, offset: 0x04, custom_opt: 20
      ...>     # 0x08 - 0x0C are discarded
      ...>     # 0x0C - 0x10
      ...>     field :float1, :f32, offset: 0x0C
      ...>     # 0x10 - 0x18
      ...>     field :int2, :i64, custom_opt: 30
      ...>   end
      ...> end
      iex> Rekto.Schema.Helpers.get_fields_with_opt(NiceSchema, :custom_opt)
      [int1: 20, int2: 30]

  """
  @spec field(field_name, Serialization.datatype(), Keyword.t()) :: any
  defmacro field(name, type, opts \\ [])

  @ptr_type Application.compile_env!(:rekto, :word_type)

  # :void_pointer — word-sized pointer to Rekto.Void (untyped target)
  defmacro field(name, :void_pointer, opts) do
    quote do
      Rekto.Schema.add_pointer(__MODULE__, unquote(name), Rekto.Void, unquote(opts))
    end
  end

  # :vtable — void pointer + non_null constraint + memoize: true + in_module: true
  defmacro field(name, :vtable, opts) do
    quote do
      merged =
        unquote(opts)
        |> Keyword.update(:constraints, [non_null: true], &Keyword.put_new(&1, :non_null, true))
        |> Keyword.put_new(:memoize, true)
        |> Keyword.put_new(:in_module, true)

      Rekto.Schema.add_pointer(__MODULE__, unquote(name), Rekto.Void, merged)
    end
  end

  # :this_pointer — word-sized field; postlude injects check_this_pointers/1 when any present
  defmacro field(name, :this_pointer, opts) do
    quote do
      Rekto.Schema.add_field(
        __MODULE__,
        unquote(name),
        unquote(@ptr_type),
        [{:this_pointer, true} | unquote(opts)]
      )
    end
  end

  defmacro field(name, type, opts) do
    quote do
      Rekto.Schema.add_field(__MODULE__, unquote(name), unquote(type), unquote(opts))
    end
  end

  @doc false
  @spec add_field(module, field_name, Serialization.datatype(), Keyword.t()) ::
          {Rekto.Schema.FieldInfo.t(),
           [{field_name, Rekto.Schema.Constraints.field_constraint()}], pos_integer}
  def add_field(mod, name, type, opts \\ []) do
    if Enum.member?(@reserved_fields, name),
      do: raise(ArgumentError, "Cannot use reserved field name #{inspect(name)}")

    current_size = Module.get_attribute(mod, :size) || 0
    current_fields = Module.get_attribute(mod, :fields)

    unless Enum.all?(current_fields, &is_map/1),
      do: raise(ArgumentError, "Expected list of field maps, got: #{inspect(current_fields)}")

    if Enum.find(current_fields, nil, fn %{name: n} -> n == name end),
      do: raise(ArgumentError, "Field #{name} already defined")

    points_to = opts[:points_to] || nil
    offset = opts[:offset] || current_size

    if points_to do
      # Validate that points_to is a known type — get_type_size! raises on unknown atoms.
      # Self-referential pointers and schema modules are also valid.
      unless points_to == mod or has_schema?(points_to) do
        Serialization.get_type_size!(points_to)
      end

      if type not in [:u32, :u64],
        do: raise(ArgumentError, "Pointers can only be :u32 or :u64")
    end

    unless is_integer(offset) and offset >= 0,
      do:
        raise(
          ArgumentError,
          "Offset #{inspect(offset)} of field #{inspect(name)} must be a non-negative integer"
        )

    other_opts =
      opts
      |> Enum.filter(fn {k, _} ->
        not Enum.member?(@reserved_opts, k)
      end)

    field_size = Serialization.get_type_size!(type)

    Enum.each(current_fields, fn %{name: other_name, size: other_size, offset: other_offset} ->
      other_upper_bound = other_offset + other_size

      cond do
        Enum.member?(other_offset..(other_upper_bound - 1), offset) ->
          raise ArgumentError, "Field #{inspect(name)} overlaps with field #{inspect(other_name)}"

        other_upper_bound > offset ->
          raise ArgumentError,
                "Field #{inspect(name)} [#{offset} - #{offset + field_size}) comes before field #{inspect(other_name)} [#{other_offset} - #{other_upper_bound}) (fields must be declared in order)"

        true ->
          nil
      end
    end)

    field_info = %Rekto.Schema.FieldInfo{
      name: name,
      data_type: type,
      offset: offset,
      size: field_size,
      opts: other_opts,
      points_to: points_to || nil
    }

    Module.put_attribute(mod, :fields, field_info)

    constraints = opts[:constraints] || []

    field_constraints =
      for field_constraint <- constraints do
        case field_constraint do
          {c_name, true} ->
            Module.put_attribute(mod, :field_constraints, {name, c_name})

          {_c_name, _c_args} ->
            Module.put_attribute(mod, :field_constraints, {name, field_constraint})

          _ ->
            raise ArgumentError,
                  "Invalid field_constraint #{inspect(field_constraint)} (field_constraints are of the form {:field_constraint_name, true_or_arg})"
        end
      end

    new_size = offset + field_size
    Module.put_attribute(mod, :size, new_size)

    {field_info, field_constraints, new_size}
  end

  @doc """
  Creates a new pointer. A pointer is just a `:u32` or `:u64` with
  a special `:points_to` option that causes the instantiation functions
  to automatically convert it to a `t:Rekto.Association.NotLoaded.t/0`.

  A pointer can be preloaded with `c:Rekto.Repo.preload/2`.

  ## Options

  * `:ptr_type` - the type of the pointer itself. Can be `:u32` or `:u64`.
  `:u32` by default. The default can be set as an environment variable:
  ```elixir
  import Config

  config :rekto, word_type: :u64
  ```
  * other options - passed as they would be to `field/3`.

  ## Examples

      iex> defmodule LinkedListNode do
      ...>   use Rekto.Schema
      ...>
      ...>   schema do
      ...>     field :value, :u32
      ...>     points_to :next, __MODULE__, ptr_type: :u64
      ...>   end
      ...> end
      iex> LinkedListNode.from_binary(<<100, 0, 0, 0>> <> <<0xF0, 0xDE, 0xBC, 0x9A, 0x78, 0x56, 0x34, 0x12>>)
      {:ok,
        %LinkedListNode{
          __meta__: %Rekto.Schema.Metadata{addr: nil, assoc_type: :none},
          next: %Rekto.Association.NotLoaded{
            __meta__: %{addr: 1311768467463790320, assoc_type: :pointer},
            data_type: LinkedListNode
          },
          value: 100
        }}
  """
  @spec points_to(field_name, Serialization.datatype(), Keyword.t()) :: any
  defmacro points_to(name, pointed_type, opts \\ []) do
    quote do
      Rekto.Schema.add_pointer(
        __MODULE__,
        unquote(name),
        unquote(pointed_type),
        unquote(opts)
      )
    end
  end

  @doc false
  @spec add_pointer(module, atom, Serialization.datatype(), Keyword.t()) ::
          {Rekto.Schema.FieldInfo.t(), [{atom, Constraints.field_constraint()}], pos_integer}
  def add_pointer(mod, name, ptd_type, opts \\ []) do
    ptr_type = opts[:ptr_type] || @ptr_type

    add_field(mod, name, ptr_type, [{:points_to, ptd_type} | Keyword.delete(opts, :ptr_type)])
  end

  @doc """
  Uses a local function as a sanity check for the module's schema.

  A sanity check is a function that takes in the entire struct and
  returns `:ok` or `{:error, reason}`. Failing a sanity check will
  cause the struct instantiation to fail.

  This can be called prior to the `schema/1` macro.

  ## Examples

      iex> defmodule IAmNotInsane do
      ...>   use Rekto.Schema
      ...>
      ...>   sanity :first_greater_than_second
      ...>   schema do
      ...>     field :first, :u16
      ...>     field :second, :u16
      ...>   end
      ...>
      ...>   def first_greater_than_second(%{first: first, second: second}) do
      ...>     if first > second do
      ...>       :ok
      ...>     else
      ...>       {:error, "YOU HAD ONE JOB"}
      ...>     end
      ...>   end
      ...> end
      iex> IAmNotInsane.from_binary(<<100, 0, 50, 0>>)
      {:ok,
       %IAmNotInsane{
         __meta__: %Rekto.Schema.Metadata{addr: nil, assoc_type: :none},
         first: 100,
         second: 50
       }}
      iex> IAmNotInsane.from_binary(<<50, 0, 100, 0>>)
      {:error, [{IAmNotInsane, :first_greater_than_second, "YOU HAD ONE JOB"}]}

  """
  @spec sanity(atom) :: any
  defmacro sanity(func_name) when is_atom(func_name) do
    quote do
      Rekto.Schema.add_sanity_check(__MODULE__, __MODULE__, unquote(func_name))
    end
  end

  @doc """
  The same as `sanity/1`, but with a function from another module.
  """
  @spec sanity(module, atom) :: any
  defmacro sanity(mod_name, func_name) do
    quote do
      Rekto.Schema.add_sanity_check(__MODULE__, unquote(mod_name), unquote(func_name))
    end
  end

  @doc false
  @spec add_sanity_check(module, module, atom) :: :ok
  def add_sanity_check(mod, mod_name, func_name) do
    unless is_atom(mod_name) and is_atom(func_name),
      do:
        raise(
          ArgumentError,
          "Expected module and function name, got (#{inspect(mod_name)}, #{inspect(func_name)})"
        )

    Module.put_attribute(mod, :sanity_checks, {mod_name, func_name})
  end

  @doc """
  Adds a transformation. Transformations take a schema struct and return a changed schema struct.

  The changed struct can even be of another type!

  ```elixir
  defmodule Long do
    use Rekto.Schema

    alias Rekto.CppString.Long

    transform :buffer_length

    schema do
      field :buffer, :void_pointer
      field :length, :u32, offset: 16, constraints: [range: {16, 1_000_000}]
      field :capacity, :u32, constraints: [range: {16, 1_000_000}]
    end

    def buffer_length(
          %Long{length: len, buffer: %Rekto.Association.NotLoaded{} = buf_assoc} = str
        ) do
      # Update the `buffer` association to be the correct length
      %Long{str | buffer: %Rekto.Association.NotLoaded{buf_assoc | data_type: {:string_buffer, len}}}
    end
  end
  ```
  """
  @spec transform(atom) :: any
  defmacro transform(func_name) do
    quote do
      Rekto.Schema.add_transformation(__MODULE__, __MODULE__, unquote(func_name))
    end
  end

  @spec transform(module, atom) :: any
  defmacro transform(mod_name, func_name) do
    quote do
      Rekto.Schema.add_transformation(__MODULE__, unquote(mod_name), unquote(func_name))
    end
  end

  @doc false
  @spec add_transformation(module, module, atom) :: :ok
  def add_transformation(mod, mod_name, func_name) do
    Module.put_attribute(mod, :transformations, {mod_name, func_name})
  end
end
