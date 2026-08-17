defmodule Rekto.Schema.Helpers do
  @moduledoc """
  Dynamically dispatched functions for getting information about
  schemas and their fields.
  """

  import Bitwise

  alias Rekto.Schema.{Constraints, Metadata}
  alias Rekto.Serialization

  @doc """
  Pretty-prints a pointer of the given type.

  ## Examples

      iex> print_ptr(305419896, :u32)
      "0x12345678"
      iex> print_ptr(1311768467463790320, :u64)
      "0x123456789ABCDEF0"

  """
  @spec print_ptr(pos_integer, Rekto.Schema.pointer()) :: binary
  def print_ptr(addr, type \\ :u32) do
    "0x#{Integer.to_string(addr, 16) |> String.pad_leading(Serialization.get_type_size!(type) * 2, "0")}"
  end

  @spec get_field_info!(Rekto.Schema.t(), Rekto.Schema.field_name()) ::
          Rekto.Schema.field_info()
  defp get_field_info!(mod, field) when is_atom(field) do
    fields = Rekto.Schema.get_schema(mod, :fields)
    result = Enum.find(fields, fn %{name: name} -> name == field end)
    unless result, do: raise(ArgumentError, "Field #{inspect(field)} not in struct")

    result
  end

  @spec get_field_key!(
          Rekto.Schema.t(),
          Rekto.Schema.field_name(),
          Rekto.Schema.field_info_key()
        ) :: Rekto.Schema.field_info_value()
  defp get_field_key!(mod, field, key) when is_atom(field) and is_atom(key) do
    get_field_info!(mod, field)
    |> Map.get(key)
  end

  @doc """
  Returns a list of field names.

  ## Examples

      iex> get_field_names(Rekto.VCPP.StdString.Short)
      [:buffer, :length, :capacity]

  """
  @spec get_field_names(Rekto.Schema.t()) :: [Rekto.Schema.field_name()]
  def get_field_names(mod) do
    Rekto.Schema.get_schema(mod, :fields)
    |> Enum.map(fn %{name: n} -> n end)
  end

  @doc """
  Returns the type of a field.

  ## Examples

      iex> get_field_type!(Rekto.VCPP.StdString.Short, :buffer)
      {:string_buffer, 16}

  """
  @spec get_field_type!(Rekto.Schema.t(), Rekto.Schema.field_name()) :: Serialization.datatype()
  def get_field_type!(mod, field) when is_atom(field) do
    get_field_key!(mod, field, :data_type)
  end

  @doc """
  Returns the size of a field.

  ## Examples

      iex> get_field_size!(Rekto.VCPP.StdString.Short, :buffer)
      16

  """
  @spec get_field_size!(Rekto.Schema.t(), Rekto.Schema.field_name()) :: pos_integer
  def get_field_size!(mod, field) when is_atom(field) do
    get_field_key!(mod, field, :size)
  end

  @doc """
  Returns the offset of a field from the beginning of the struct.

  ## Examples

      iex> get_field_offset!(Rekto.VCPP.StdString.Short, :capacity)
      20

  """
  @spec get_field_offset!(Rekto.Schema.t(), Rekto.Schema.field_name()) :: pos_integer
  def get_field_offset!(mod, field) when is_atom(field) do
    get_field_key!(mod, field, :offset)
  end

  @doc """
  Returns the field's options (not including default ones such as `:offset`, `:ptr_type`, etc.)

  ## Examples

      iex> get_field_opts!(Rekto.ExampleStruct, :vftable)
      [in_exe: true]
      iex> get_field_opts!(Rekto.ExampleStruct, :epicness)
      []

  """
  @spec get_field_opts!(Rekto.Schema.t(), Rekto.Schema.field_name()) :: Keyword.t()
  def get_field_opts!(mod, field) when is_atom(field) do
    if field in [:__struct__, :__meta__] do
      []
    else
      get_field_key!(mod, field, :opts)
    end
  end

  @doc """
  Returns true if the field has the given option.

      iex> field_has_opt?(Rekto.ExampleStruct, :vftable, :in_exe)
      true
      iex> field_has_opt?(Rekto.ExampleStruct, :vftable, :in_heap)
      false

  """
  @spec field_has_opt?(Rekto.Schema.t(), Rekto.Schema.field_name(), Rekto.Schema.opt_name()) ::
          boolean
  def field_has_opt?(mod, field, opt) do
    get_field_opts!(mod, field)
    |> Keyword.has_key?(opt)
  end

  @doc """
  Returns a list of the fields of the schema that are pointers, in the format `{:name, :ptr_type}`.

  ## Examples

      iex> get_pointer_fields(Rekto.ExampleStruct)
      [vftable: :u32, parent: Rekto.ExampleStruct]

  """
  @spec get_pointer_fields(Rekto.Schema.t()) :: [Rekto.Schema.field_info()]
  def get_pointer_fields(mod) do
    Rekto.Schema.get_schema(mod, :fields)
    |> Enum.filter(fn %{points_to: pts} -> not is_nil(pts) end)
    |> Enum.map(fn %{name: name, points_to: pts} -> {name, pts} end)
  end

  @doc """
  Returns true if the given field is a pointer.

  ## Examples

      iex> field_is_pointer?(Rekto.ExampleStruct, :vftable)
      true
      iex> field_is_pointer?(Rekto.ExampleStruct, :epicness)
      false

  """
  @spec field_is_pointer?(Rekto.Schema.t(), Rekto.Schema.field_name()) :: boolean
  def field_is_pointer?(mod, field) do
    if get_field_key!(mod, field, :points_to) do
      true
    else
      false
    end
  end

  @doc """
  Returns the value of the option for the given field, or `nil` if
  the option is not set.

  ## Examples

      iex> get_field_opt!(Rekto.ExampleStruct, :vftable, :in_exe)
      true
      iex> get_field_opt!(Rekto.ExampleStruct, :vftable, :in_heap)
      nil

  """
  @spec get_field_opt!(
          Rekto.Schema.t(),
          Rekto.Schema.field_name(),
          Rekto.Schema.opt_name()
        ) :: any
  def get_field_opt!(mod, field, opt) when is_atom(field) do
    get_field_opts!(mod, field)
    |> Keyword.get(opt)
  end

  @doc """
  Resolves the `:pointer_offset` option for a field to a numeric byte offset.

  Returns:
  - `0` if the option is not set
  - the integer directly if it's already a number
  - the byte offset of the named field in the target schema, if it's an atom

  For `:this_pointer` fields, the target schema is the field's own module.
  For `points_to` fields, the target schema is the pointed-to module.
  """
  @spec resolve_pointer_offset(Rekto.Schema.t(), Rekto.Schema.field_name()) :: non_neg_integer
  def resolve_pointer_offset(mod, field_name) do
    raw = get_field_opt!(mod, field_name, :pointer_offset)

    case raw do
      nil -> 0
      n when is_integer(n) -> n
      target_field when is_atom(target_field) ->
        field_info = get_field_info!(mod, field_name)

        target_mod =
          cond do
            Keyword.get(field_info.opts, :this_pointer, false) -> mod
            field_info.points_to != nil -> field_info.points_to
            true -> mod
          end

        get_field_offset!(target_mod, target_field)
    end
  end

  @doc """
  If the first argument is a struct, gets the fields of the struct
  with the given options. Returns a list of `{:field_name, option_value, field_value}` tuples.

  If the first argument is a module with a schema, returns a list
  of the fields of the schema that have the given options, in the
  form `{:field_name, option_value}`.

  ## Examples

      iex> get_fields_with_opt(Rekto.ExampleStruct, :in_heap)
      [parent: true]

      iex> {:ok, nothing_burger} = String.duplicate(<<0x00>>, Rekto.Serialization.get_type_size!(Rekto.ExampleStruct)) |> Rekto.ExampleStruct.from_binary()
      iex> get_fields_with_opt(nothing_burger, :in_heap)
      [{:parent, true, %Rekto.Association.NotLoaded{__meta__: %Rekto.Schema.Metadata{addr: 0, assoc_type: :pointer}, data_type: Rekto.ExampleStruct}}]
      iex> get_fields_with_opt(nothing_burger, :in_exe)
      [{:vftable, true, %Rekto.Association.NotLoaded{__meta__: %Rekto.Schema.Metadata{addr: 0, assoc_type: :pointer}, data_type: :u32}}]
  """
  @spec get_fields_with_opt(Rekto.Schema.schema_struct(), Rekto.Schema.opt_name()) :: [
          {Rekto.Schema.field_name(), any, any}
        ]
  def get_fields_with_opt(%{__struct__: mod} = struct, opt) do
    get_fields_with_opt(mod, opt)
    |> Enum.map(fn {field_name, opt_value} ->
      {field_name, opt_value, Map.get(struct, field_name)}
    end)
  end

  @spec get_fields_with_opt(Rekto.Schema.t(), Rekto.Schema.opt_name()) :: Keyword.t()
  def get_fields_with_opt(mod, opt) when is_atom(mod) and is_atom(opt) do
    Rekto.Schema.get_schema(mod, :fields)
    |> Enum.filter(fn %{opts: opt_list} ->
      Keyword.has_key?(opt_list, opt)
    end)
    |> Enum.map(fn %{name: name, opts: opt_list} ->
      {name, opt_list[opt]}
    end)
  end

  @spec get_address(
          Rekto.Schema.schema_struct()
          | Rekto.Association.NotLoaded.t()
          | Rekto.Association.Primitive.t()
        ) :: non_neg_integer | nil
  def get_address(%{__meta__: %Metadata{addr: addr}}), do: addr

  @doc """
  Returns a byte mask with all bytes filled for the fields in the given list,
  and all bytes null for the ones not in the list.

  ## Example

      iex> mask_fields(Rekto.VCPP.StdString.Short, [:length])
      <<0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 255, 255, 255, 255, 0, 0, 0, 0>>

  """
  @spec mask_fields(Rekto.Schema.t(), [Rekto.Schema.field_name()]) :: binary
  def mask_fields(mod, fields) do
    my_field_names = get_field_names(mod)

    unless MapSet.subset?(MapSet.new(fields), MapSet.new(my_field_names)),
      do:
        raise(
          ArgumentError,
          "Fields #{inspect(fields)} are not all in the schema; schema fields are #{inspect(my_field_names)}"
        )

    for %{name: field_name, size: field_size} <-
          Rekto.Schema.get_schema(mod, :fields_incl_garbage) do
      cond do
        Enum.member?(fields, field_name) ->
          List.duplicate(0xFF, field_size)

        true ->
          List.duplicate(0x00, field_size)
      end
    end
    |> Enum.concat()
    |> :erlang.list_to_binary()
  end

  #  @doc """
  #  "Patches" a binary with another binary at the given index.
  #
  #  ## Examples
  #
  #      iex> patch_binary(<<0x00, 0x00, 0x00, 0x00>>, <<0xFF, 0xFF>>, 1)
  #      <<0, 255, 255, 0>>
  #
  #  """
  @doc false
  @spec patch_binary(binary, binary, non_neg_integer) :: binary
  def patch_binary(b0, b1, index) do
    s0 = byte_size(b0)
    s1 = byte_size(b1)

    unless s0 >= index + s1,
      do:
        raise(
          ArgumentError,
          "Attempt to patch binary of size #{byte_size(b0)} with binary of size #{byte_size(b1)} at index #{index} (will not fit)"
        )

    binary_part(b0, 0, index) <> b1 <> binary_part(b0, index + s1, s0 - (index + s1))
  end

  # byte-by-byte via lists — slow, but binaries are usually small here
  @spec bin_and(binary, binary) :: binary
  defp bin_and(b0, b1) do
    unless byte_size(b0) == byte_size(b1),
      do: raise(ArgumentError, "Binaries must be the same size")

    :erlang.binary_to_list(b0)
    |> Enum.zip(:erlang.binary_to_list(b1))
    |> Enum.map(fn {i0, i1} -> i0 &&& i1 end)
    |> :erlang.list_to_binary()
  end

  @doc """
  Runs pattern inference for the given schema and user-provided field values.

  Returns the merged field map with pin semantics: user-provided values are never
  overridden by inferred values. Callers pass the result to `to_masked_bytes/2`
  when they want inference to affect the scan pattern.

  If the schema does not implement `pattern_inference/1`, returns the input
  unchanged (converted to a map).
  """
  @spec infer_fields(Rekto.Schema.t(), %{required(atom) => any} | Keyword.t()) ::
          %{required(atom) => any}
  def infer_fields(mod, field_values) do
    user_map = Enum.into(field_values, %{})

    if function_exported?(mod, :pattern_inference, 1) do
      inferred = mod.pattern_inference(user_map)
      inferred_clean = Map.drop(inferred, Map.keys(user_map))
      Map.merge(user_map, inferred_clean)
    else
      user_map
    end
  end

  @doc """
  Returns a series of bytes representing the schema struct with the given field values, as well
  as a mask for the given fields.

  ## Examples

      iex> to_masked_bytes(Rekto.VCPP.StdString.Short, [length: 15, capacity: 15])
      {
        <<0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 15, 0, 0, 0, 15, 0, 0, 0>>,
        <<0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 255, 255, 255, 255, 255, 255, 255, 255>>
      }

  """
  @spec to_masked_bytes(Rekto.Schema.t(), %{required(atom) => any} | Keyword.t()) ::
          {binary, binary}
  def to_masked_bytes(mod, field_values) do
    values_dict =
      field_values
      |> Enum.into(%{})

    relevant_field_info =
      values_dict
      |> Enum.map(fn {name, value} ->
        {get_field_info!(mod, name), value}
      end)
      |> Enum.map(fn {%{name: name, data_type: type, offset: offset}, value} ->
        %{
          name: name,
          data_type: type,
          offset: offset,
          value: value
        }
      end)

    non_field_masks =
      if function_exported?(mod, :custom_mask, 1) do
        garbage_mask = Rekto.Schema.get_schema(mod, :garbage_mask)
        custom_mask = mod.custom_mask(values_dict)

        unless byte_size(custom_mask) == mod.__size__(),
          do:
            raise(
              ArgumentError,
              "#{mod}.custom_mask/1 returned mask #{byte_size(custom_mask)} bytes long, should be same size as schema (#{mod.__size__()} bytes)"
            )

        bin_and(garbage_mask, custom_mask)
      else
        Rekto.Schema.get_schema(mod, :garbage_mask)
      end

    all_i_need = bin_and(non_field_masks, mask_fields(mod, Map.keys(values_dict)))

    fields_and_dead_space =
      Enum.reduce(
        relevant_field_info,
        all_i_need,
        fn %{name: _name, data_type: type, offset: offset, value: value}, struct_bytes ->
          patch_binary(struct_bytes, Serialization.to_bytes(value, type), offset)
        end
      )

    {fields_and_dead_space, all_i_need}
  end

  @doc false
  @spec struct_from_values(
          Rekto.Schema.t(),
          [{Rekto.Schema.field_name(), {:ok, any} | {:error, any} | map}],
          map | Metadata.t()
        ) :: {:ok, Rekto.Schema.schema_struct()} | {:error, Keyword.t()}
  def struct_from_values(mod, values, meta) do
    bad_boys =
      Enum.filter(values, fn
        {_, {:error, _}} -> true
        _ -> false
      end)

    meta =
      case meta do
        %Metadata{} ->
          Map.from_struct(meta)

        %{} ->
          meta
      end

    meta_struct = struct(Metadata, meta)

    case bad_boys do
      [] ->
        keys_values =
          Enum.map(values, fn
            {field_name, {:ok, val}} ->
              {field_name, val}

            {_, %{}} = t ->
              t
          end)
          |> Kernel.++(__meta__: meta_struct)

        {:ok, struct(mod, keys_values)}

      naughty_list ->
        Enum.map(naughty_list, fn {field_name, {:error, err}} ->
          {field_name, err}
        end)
    end
  end

  @doc false
  @dialyzer {:no_match, check_field_constraints: 1}
  @dialyzer {:no_match, check_field_constraints: 2}
  def check_field_constraints(%{__struct__: mod} = struct),
    do: check_field_constraints(mod, struct)

  # 2-arg form: works with any map; only validates constraints for fields present in the map.
  # This is intentional — callers passing a partial field set (e.g. a query's :where clause)
  # should not be penalised for fields they are not constraining.
  def check_field_constraints(mod, %{} = fields) when is_atom(mod) do
    present_keys = MapSet.new(Map.keys(fields))

    Rekto.Schema.get_schema(mod, :field_constraints)
    |> Enum.filter(fn {field_name, _} -> MapSet.member?(present_keys, field_name) end)
    |> Enum.map(fn {field_name, constraint} ->
      case Constraints.check_field_constraint(constraint, Map.get(fields, field_name)) do
        :ok ->
          nil

        {:error, err_msg} ->
          {field_name, err_msg}

        # Defensive: dialyzer proves this is unreachable but we keep it as a safety net.
        unk ->
          raise ArgumentError,
                "Returned #{unk} (field constraints must return :ok or {:error, reason})"
      end
    end)
    |> Enum.filter(fn
      {_, _} -> true
      _ -> false
    end)
  end

  @doc false
  def check_sanity(%{__struct__: mod} = struct) do
    Enum.flat_map(Rekto.Schema.get_schema(mod, :sanity_checks), fn {mod_name, func_name} ->
      case apply(mod_name, func_name, [struct]) do
        :ok ->
          []

        {:error, err_msg} ->
          [{{mod_name, func_name}, err_msg}]

        unk ->
          raise ArgumentError,
                "Returned #{unk} (sanity checks must return :ok or {:error, reason})"
      end
    end)
  end

  # 2-arg form for partial maps: used by Query.validate/2.
  # Rescues crashes from pattern-match failures when checked fields are absent — those checks
  # simply cannot run on the partial data and are skipped rather than treated as failures.
  # Deliberately NOT delegated from the 1-arg form so deserialization keeps strict behaviour.
  def check_sanity(mod, %{} = fields) when is_atom(mod) do
    Enum.flat_map(Rekto.Schema.get_schema(mod, :sanity_checks), fn {mod_name, func_name} ->
      try do
        case apply(mod_name, func_name, [fields]) do
          :ok -> []
          {:error, err_msg} -> [{{mod_name, func_name}, err_msg}]
          _ -> []
        end
      rescue
        _ -> []
      end
    end)
  end

  @doc false
  def map_assocs(%{__struct__: mod} = struct) do
    old_fields = Map.from_struct(struct)

    updated_fields =
      get_pointer_fields(mod)
      |> Enum.map(fn {name, type} ->
        {name, type, Map.get(old_fields, name)}
      end)
      |> Enum.map(fn {name, type, value} ->
        {
          name,
          %Rekto.Association.NotLoaded{
            __meta__: %Metadata{
              addr: value,
              assoc_type: :pointer
            },
            data_type: type
          }
        }
      end)
      |> Enum.into(old_fields)

    struct(mod, updated_fields)
  end

  @doc false
  @spec apply_transformations(Rekto.Schema.schema_struct()) ::
          Rekto.Schema.schema_struct()
  def apply_transformations(%{__struct__: mod} = struct) do
    Enum.reduce(Rekto.Schema.get_schema(mod, :transformations), struct, fn {fn_mod, fn_name},
                                                                           acc ->
      case acc do
        %mod{} ->
          case apply(fn_mod, fn_name, [acc]) do
            %{} = s ->
              s

            {:error, reason} ->
              [{{fn_mod, fn_name}, reason}]

            x ->
              raise ArgumentError,
                    "Expected %#{inspect(mod)}{} or {:error, reason}, got #{inspect(x)}"
          end

        l when is_list(l) ->
          l
      end
    end)
  end
end
