defmodule Rekto.Repo do
  @moduledoc """
  A behaviour for a module that can read and scan contiguous regions of bytes.

  Implementors must implement `c:read_bytes/3` and `c:execute_query/2`.
  Everything else can be automatically implemented with `use Rekto.Repo`.

  This can be a file, a buffer, or, as in the original use case, another
  process's memory.
  """

  require Logger

  alias Rekto.Association
  alias Rekto.Schema.Metadata
  alias Rekto.Serialization
  alias Rekto.Query

  @type t :: module

  @typedoc """
  An integer address from which bytes can be read.

  This can be a memory address or perhaps a location within a file.
  """
  @type address :: non_neg_integer

  @typedoc """
  Type that can be passed to `c:preload/2` to preload fields.

  ## Examples

  ```elixir
  :my_field
  [:one_field, :another_field]
  {:my_struct_field, :my_struct_subfield}
  {:my_struct_field, [:field1, :field2, :field3]}
  {:my_struct_field, {:field_inception, :grandchild}}
  ```
  """
  @type preload_fields ::
          Rekto.Schema.field_name()
          | [Rekto.Schema.field_name()]
          | {Rekto.Schema.field_name(), preload_fields}

  # Required

  @doc """
  Reads the given number of bytes from the given address.

  Returns an error if the address is invalid.
  """
  @callback read_bytes(address :: address, num_bytes :: pos_integer, opts :: Keyword.t()) ::
              {:ok, binary} | {:error, any}

  @doc """
  Executes a `Rekto.Query`, returning a list of matching addresses.

  How this is done is up to the implementor, although the functions in
  `Rekto.Schema.Helpers` are a big help.
  """
  @callback execute_query(query :: Query.t(), opts :: Keyword.t()) ::
              {:ok, [address]} | {:error, any}

  # Macro'd

  @doc """
  Reads `Rekto.Serialization.get_type_size!(schema)` bytes from the given address
  and converts them to a struct.

  ## Options

  * `:preload` - Pointers to preload.
  * `:filter` - Same as the `:filter` field of `t:Rekto.Query.t/0`.

  """
  @callback get(schema :: Rekto.Schema.t(), address :: address, opts :: Keyword.t()) ::
              {:ok, Rekto.Schema.schema_struct()} | {:error, any}

  @doc """
  Loads the pointers at the given fields. The given fields will be converted to:

  * Regular `t:Rekto.Schema.schema_struct/0`, if `:points_to` is a module with a schema.
  * Instances of `t:Rekto.Association.Primitive.t/0`, if `:points_to` is a primitive type.
  * Instances of `t: Rekto.Association.Error.t/0`, if the preloading operation is unsuccessful.
  """
  @callback preload(
              struct :: Rekto.Schema.schema_struct(),
              fields :: preload_fields,
              opts :: Keyword.t()
            ) :: Rekto.Schema.schema_struct()

  @doc """
  Returns all matches for the given query, converted to the proper types.

  Takes the same options as `c:get/3`. These options will override options in the query.
  """
  @callback all(query :: Query.t(), opts :: Keyword.t()) ::
              {:ok, [Rekto.Schema.schema_struct()]} | {:error, any}

  @doc """
  Returns a single match for the query, or an error if there is more than one match.

  Takes the same options as `c:get/3`. These options will override options in the query.
  """
  @callback one(Query.t(), opts :: Keyword.t()) ::
              {:ok, Rekto.Schema.schema_struct()} | nil | {:error, :multiple} | {:error, any}

  # @get_opts ~w[preload]a

  defmacro __using__(_opts) do
    quote do
      @behaviour Rekto.Repo

      alias Rekto.Repo

      @impl Repo
      def get(schema, addr, opts \\ []) do
        Repo.get(__MODULE__, schema, addr, opts)
      end

      @impl Repo
      def preload(%{__struct__: _mod} = s_struct, keys, opts) do
        Repo.preload(__MODULE__, s_struct, keys, opts)
      end

      @impl Repo
      def all(%Query{} = query, opts \\ []) do
        Repo.all(__MODULE__, query, opts)
      end

      @impl Repo
      def one(%Query{} = query, opts \\ []) do
        Repo.one(__MODULE__, query, opts)
      end
    end
  end

  defp maybe_filter(list, %Query{filter: nil}), do: list
  defp maybe_filter(list, %Query{filter: filter}), do: Enum.filter(list, filter)

  @doc false
  @spec get(t, Rekto.Schema.t(), pos_integer, Keyword.t()) ::
          {:ok, Rekto.Schema.schema_struct()} | {:error, any}
  def get(repo, schema, addr, opts \\ []) do
    if addr == 0 do
      {:error, :null_ptr}
    else
      with schema_size <- Serialization.get_type_size!(schema),
           {:ok, bytes} <- repo.read_bytes(addr, schema_size, opts),
           {:ok, struct} <- schema.from_binary(bytes, %{addr: addr, assoc_type: :none}) do
        if opts[:preload] do
          {:ok, preload(repo, struct, opts[:preload], opts)}
        else
          {:ok, struct}
        end
      end
    end
  end

  @doc false
  @spec all(t, Query.t(), Keyword.t()) :: {:ok, [Rekto.Schema.schema_struct()]} | {:error, any}
  def all(repo, %Query{schema: schema} = query, opts \\ []) do
    query_opts = [preload: query.preload, filter: query.filter]

    opts = Keyword.merge(query_opts, opts)

    with {:ok, addrs} <- repo.execute_query(query, opts) do
      Logger.debug("[Rekto.Repo] #{schema} — #{length(addrs)} candidates, validating")

      {ok_results, discarded} =
        addrs
        |> Enum.map(fn addr -> {addr, get(repo, schema, addr, opts)} end)
        |> Enum.split_with(fn {_addr, result} -> match?({:ok, _}, result) end)

      Enum.each(discarded, fn {addr, {:error, reason}} ->
        Logger.debug(
          "[Rekto.Repo] discarded 0x#{Integer.to_string(addr, 16)} reason=#{inspect(reason)}"
        )

        if opts[:show_discarded], do: IO.inspect({:error, reason})
      end)

      struct_list =
        ok_results
        |> maybe_filter(query)
        |> Enum.map(fn {_addr, {:ok, struct}} -> struct end)

      Logger.info(
        "[Rekto.Repo] #{schema} — #{length(struct_list)}/#{length(addrs)} passed validation"
      )

      {:ok, struct_list}
    end
  end

  @doc false
  @spec one(t, Query.t(), Keyword.t()) ::
          {:ok, Rekto.Schema.schema_struct()} | nil | {:error, :multiple} | {:error, any}
  def one(repo, %Query{} = query, opts \\ []) do
    case all(repo, query, opts) do
      {:ok, [the_one]} ->
        {:ok, the_one}

      {:ok, [_ | _]} ->
        {:error, :multiple}

      {:ok, []} ->
        nil

      err ->
        err
    end
  end

  defp prim_or_mod(bytes, type, meta) do
    if is_atom(type) and String.starts_with?(to_string(type), "Elixir.") do
      Serialization.from_bytes(bytes, type, meta)
    else
      Serialization.from_bytes(bytes, type)
    end
  end

  defp valid_preload_key!(mod, {subfield, subkey}) do
    ptr_fields = Rekto.Schema.Helpers.get_pointer_fields(mod)

    subtype = ptr_fields[subfield] || Rekto.Schema.Helpers.get_field_type!(mod, subfield)

    unless Rekto.Schema.has_schema?(subtype),
      do: raise(ArgumentError, "Field #{subfield} has no schema")

    unless subkey in Rekto.Schema.Helpers.get_field_names(subtype),
      do: raise(ArgumentError, "Field #{subfield} has no subfield #{subtype}")

    valid_preload_key!(subtype, subkey)
  end

  defp valid_preload_key!(mod, keys) when is_list(keys) do
    Enum.all?(keys, &valid_preload_key!(mod, &1))
  end

  defp valid_preload_key!(mod, key) when is_atom(key) do
    unless Rekto.Schema.Helpers.field_is_pointer?(mod, key),
      do: raise(ArgumentError, "Field #{key} of #{mod} is not a pointer")

    true
  end

  @doc false
  def preload(repo, struct, keys, opts \\ [])

  @spec preload(t, Rekto.Schema.schema_struct(), atom, Keyword.t()) ::
          Rekto.Schema.schema_struct()
  def preload(repo, %{__struct__: _mod} = struct, key, opts) when is_atom(key) do
    preload(repo, struct, [key], opts)
  end

  @doc false
  @spec preload(t, Rekto.Schema.schema_struct(), [Rekto.Schema.field_name()], Keyword.t()) ::
          Rekto.Schema.schema_struct()
  def preload(repo, %{__struct__: mod} = s_struct, keys, opts) do
    valid_preload_key!(mod, keys)

    atom_keys = Enum.filter(keys, &is_atom/1)
    compound_keys = Enum.filter(keys, &(!is_atom(&1)))

    new_l1_fields =
      s_struct
      |> Map.take(atom_keys)
      |> Enum.map(fn
        {field_name, %Association.NotLoaded{data_type: Rekto.Void}} ->
          raise ArgumentError,
                "Attempt to load void pointer at field #{field_name}" <>
                  "\nVoid pointers are not meant to be loaded;" <>
                  "\nDid you mean to implement a transformation that types it?"

        {field_name, %Association.NotLoaded{__meta__: %Metadata{addr: addr_v}, data_type: type}} ->
          offset = Rekto.Schema.Helpers.resolve_pointer_offset(mod, field_name)
          addr = addr_v - offset

          with field_size <- Serialization.get_type_size!(type),
               {:ok, <<bytes::binary-size(field_size)>>} <-
                 repo.read_bytes(addr, field_size, opts),
               {:ok, field_value} <- prim_or_mod(bytes, type, %{addr: addr, assoc_type: :pointer}) do
            case field_value do
              %{__struct__: _} = struct ->
                {field_name, struct}

              _ ->
                {field_name,
                 %Association.Primitive{
                   __meta__: %Metadata{addr: addr, assoc_type: :pointer},
                   data_type: type,
                   value: field_value
                 }}
            end
          else
            {:error, err} -> {field_name, %Association.Error{message: err}}
          end

        {field_name, unk} ->
          raise ArgumentError,
                "Expected unloaded association or struct w/ unloaded associations, got #{inspect(unk)} as field #{inspect(field_name)}"
      end)
      |> Map.new()

    new_substruct_fields =
      Enum.reduce(compound_keys, Map.merge(Map.from_struct(s_struct), new_l1_fields), fn k,
                                                                                         my_struct ->
        case {k, Map.get(my_struct, elem(k, 0))} do
          {{field_name, subfields}, %{__struct__: struct_type, __meta__: %Metadata{}} = substruct}
          when struct_type not in [Association.NotLoaded, Association.Primitive] ->
            Map.put(my_struct, field_name, preload(repo, substruct, subfields))

          {field_name, unk} ->
            raise ArgumentError,
                  "Expected unloaded association or struct w/ unloaded associations, got #{inspect(unk)} as field #{inspect(field_name)}"
        end
      end)

    struct(s_struct, new_substruct_fields)
  end
end
