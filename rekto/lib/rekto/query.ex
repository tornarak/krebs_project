defmodule Rekto.Query do
  @moduledoc """
  A query struct for use with `Rekto.Repo`.

  Build queries with `from/2` and pass them to `c:Rekto.Repo.all/2`,
  `c:Rekto.Repo.one/2`, or `c:Rekto.Repo.execute_query/2`.
  """

  @valid_opts ~w[where filter preload]a
  @enforce_keys [:schema, :where]
  defstruct [:schema | @valid_opts]

  @type t :: %Rekto.Query{
          schema: Rekto.Schema.t(),
          where: [{Rekto.Schema.field_name(), any}],
          filter: (Rekto.Schema.schema_struct() -> boolean) | nil,
          preload: [Rekto.Schema.field_name()] | nil
        }

  alias Rekto.Schema.Helpers

  defp valid_opts_set do
    MapSet.new(@valid_opts)
  end

  @spec subset(nonempty_list(), MapSet.t()) :: boolean
  defp subset([{_, _} | _] = kw, set) do
    kw
    |> Keyword.keys()
    |> MapSet.new()
    |> MapSet.subset?(set)
  end

  defp subset([_ | _] = regular_list, set) do
    regular_list
    |> MapSet.new()
    |> MapSet.subset?(set)
  end

  defp subset([], _), do: raise(ArgumentError, "empty list")

  # ended up reinventing something like Ecto changesets

  @doc """
  Validates query options against a schema without raising.

  Returns `[]` when all values are valid. Returns a non-empty list of
  `{field_or_key, reason}` error tuples for value-level problems.

  Raises `ArgumentError` for structural caller errors (unknown field names) just like `from/2`.

  Checks (in order, short-circuiting per field):
  1. Value can be serialized as the field's declared type.
  2. Value satisfies the field's declared constraints (only for queried fields).
  3. Sanity checks over the full `:where` map (skips checks that crash on partial data).

  Constraint/sanity error reasons use the structured tuple format from
  `Rekto.Schema.Constraints` so callers can pattern-match on them.

  Intended use: call `validate/2` before `from/2` in contexts where invalid user input
  should produce an error message rather than a crash.
  """
  @spec validate(module, Keyword.t()) :: [{atom | {module, atom}, any}]
  def validate(schema, opts) when is_atom(schema) do
    where = opts[:where] || []

    field_errors =
      Enum.flat_map(where, fn {name, val} ->
        # Raises ArgumentError for unknown fields — same contract as from/2.
        type = Helpers.get_field_type!(schema, name)

        serialize_error =
          try do
            Rekto.Serialization.to_bytes(val, type)
            nil
          rescue
            e -> {name, {:not_serializable, Exception.message(e)}}
          end

        constraint_errors =
          if is_nil(serialize_error),
            do: Helpers.check_field_constraints(schema, %{name => val}),
            else: []

        List.wrap(serialize_error) ++ constraint_errors
      end)

    sanity_errors =
      if where == [],
        do: [],
        else: Helpers.check_sanity(schema, Map.new(where))

    field_errors ++ sanity_errors
  end

  @doc """
  Formats the error list returned by `validate/2` as an iolist of
  `[key, ": ", reason]` rows interspersed with `"\\n"` — prints identically to
  the old flat-string version when passed to `IO.iodata_to_binary/1`.

  To feed to the NIF (skipping the `"\\n"` separators):

      for [field, _sep, msg] <- Rekto.Query.format_errors(errors), do: {field, msg}
  """
  @spec format_errors([{atom | {module, atom}, any}]) :: iolist()
  def format_errors(errors) do
    errors
    |> Enum.map(fn
      {field, {:not_serializable, msg}} ->
        ["#{field}", ": ", msg]

      {{mod, func}, reason} ->
        key = "#{mod}.#{func}" |> String.trim_leading("Elixir.")
        [key, ": ", inspect(reason)]

      {field, reason} ->
        ["#{field}", ": ", inspect(reason)]
    end)
    |> Enum.intersperse("\n")
  end

  @spec enforce_non_nil(Keyword.t(), Keyword.key()) :: Keyword.t()
  defp enforce_non_nil(opts, opt_name) do
    case opts[opt_name] do
      nil ->
        raise ArgumentError, "Option #{opt_name} is required"

      [] ->
        raise ArgumentError, "Option #{opt_name} cannot be empty"

      _ ->
        opts
    end
  end

  @spec enforce_keyword(Keyword.t(), Keyword.key()) :: Keyword.t()
  defp enforce_keyword(opts, opt_name) do
    case opts[opt_name] do
      nil ->
        opts

      [{_, _} | _] ->
        opts

      _ ->
        raise ArgumentError, "Option #{opt_name} must be a non-empty keyword list"
    end
  end

  @spec enforce_subset(Keyword.t(), Keyword.key(), MapSet.t()) :: Keyword.t()
  defp enforce_subset(opts, opt_name, set) do
    opt = opts[opt_name]

    cond do
      is_nil(opt) ->
        opts

      subset(opt, set) ->
        opts

      true ->
        raise ArgumentError,
              "option #{opt_name} (#{inspect(opt)}) must be a subset of #{inspect(set)}"
    end
  end

  @doc """
  Returns a new query for the given schema and options.

  ## Required Options

  * `:where` - A non-empty keyword list of field names and values.
  For obvious reasons, only direct equivalence will result in a match.

  ## Optional Options

  * `:preload` - A list of pointer fields to preload.
  * `:filter` - A function that takes a schema struct and
  returns true or false. Effectively a dynamic sanity check.
  """
  # MapSet.subset? triggers an opaque-type warning due to dialyzer's strict MapSet analysis.
  @dialyzer {:no_opaque, from: 2}
  @spec from(module, Keyword.t()) :: t
  def from(schema, opts) when is_atom(schema) do
    #   unless Schema.has_schema?(schema), do: raise(ArgumentError, "#{schema} is not a valid schema")

    unless MapSet.subset?(opts |> MapSet.new(fn {k, _} -> k end), valid_opts_set()),
      do:
        raise(
          ArgumentError,
          "The given options (#{inspect(Keyword.keys(opts))}) are invalid (valid options are #{inspect(@valid_opts)})"
        )

    pointer_fields =
      Helpers.get_field_names(schema)
      |> Enum.filter(&Helpers.field_is_pointer?(schema, &1))
      |> MapSet.new()

    unless is_list(opts[:where]),
      do: raise(ArgumentError, "Query must contain field :where, a keyword list")

    case validate(schema, opts) do
      [] ->
        :ok

      errors ->
        raise ArgumentError,
              "Query validation failed for #{schema}: #{inspect(errors)}"
    end

    opts
    |> enforce_non_nil(:where)
    |> enforce_keyword(:where)
    |> enforce_subset(:preload, pointer_fields)

    unless is_nil(opts[:filter]) or is_function(opts[:filter], 1),
      do: raise(ArgumentError, "Option :filter (#{opts[:filter]}) must be a function of arity 1")

    %__MODULE__{
      schema: schema,
      where: opts[:where],
      filter: opts[:filter],
      preload: opts[:preload]
    }
  end

  @doc """
  Expands the query's `:where` clause with pattern inference.

  Calls `pattern_inference/1` on the schema (if implemented) and merges
  inferred fields with pin semantics — user-provided fields always win.

  Returns a new query with the expanded `:where`. The original query is
  unchanged. No-op if the schema does not implement `pattern_inference/1`.

  ## Options

    * `:only` — list of field names; only infer these fields (ignore others)
    * `:except` — list of field names; infer everything except these

  ## Examples

      query = Query.from(VCPP.StdString.Short, where: [buffer: "hello"])
      Query.infer(query)
      # => %Query{where: [buffer: "hello", length: 5, capacity: 15], ...}

      Query.infer(query, except: [:capacity])
      # => %Query{where: [buffer: "hello", length: 5], ...}
  """
  @spec infer(t(), Keyword.t()) :: t()
  def infer(%__MODULE__{schema: schema, where: where} = query, opts \\ []) do
    merged = Helpers.infer_fields(schema, where)
    user_keys = MapSet.new(Keyword.keys(where))
    inferred_only = Map.drop(merged, MapSet.to_list(user_keys))

    filtered =
      cond do
        only = opts[:only] ->
          Map.take(inferred_only, only)

        except = opts[:except] ->
          Map.drop(inferred_only, except)

        true ->
          inferred_only
      end

    %{query | where: where ++ Enum.to_list(filtered)}
  end
end
