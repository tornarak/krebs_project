defmodule Neoplasm.Helpers do
  @moduledoc """
  Shared logic used by both Neoplasm.Window and Neoplasm.Scanner.
  Pure utility — no GenServer, no state.
  """

  alias Neoplasm.Nif

  @doc "Fetches regions + modules from Krebs.Scanner and pushes memory layout to the NIF."
  def push_memory_layout do
    with {:ok, regions} <- Krebs.Scanner.regions(),
         {:ok, modules} <- Krebs.Scanner.modules() do
      region_entries =
        Enum.map(regions, fn r ->
          size = r.range_end - r.range_start
          kind = if r.perms && String.contains?(r.perms, "w"), do: :heap, else: :other
          {r.range_start, size, kind}
        end)

      module_entries =
        Enum.map(modules, fn m ->
          {m.range_start, m.range_end - m.range_start, :module}
        end)

      Nif.set_memory_layout(region_entries ++ module_entries)
    else
      {:error, reason} ->
        Nif.set_error("Memory layout failed: #{inspect(reason)}")
    end
  end

  @doc """
  Reads a value at the given address, returning `{:ok, string}` or `{:error, string}`.

  If `type` is a Rekto schema module, uses `Krebs.Repo.get/2`.
  Otherwise uses `Krebs.Scanner.read_type/2`.

  Options:
    * `bare_map: true` — strip `__meta__` from schema structs before formatting
  """
  def read_value_at(addr, type, opts \\ []) do
    bare_map = Keyword.get(opts, :bare_map, false)

    if Rekto.Schema.has_schema?(type) do
      case Krebs.Repo.get(type, addr) do
        {:ok, struct} ->
          displayed =
            if bare_map do
              struct |> Map.from_struct() |> Map.delete(:__meta__)
            else
              struct
            end

          {:ok, inspect(displayed, pretty: true)}

        {:error, reason} ->
          {:error, inspect(reason)}
      end
    else
      case Krebs.Scanner.read_type(addr, type) do
        {:ok, value} -> {:ok, inspect(value)}
        {:error, reason} -> {:error, inspect(reason)}
      end
    end
  end

  @doc """
  Builds a Rekto query from a schema name and field value strings.

  Returns:
    * `{:ok, query}` — valid query ready to execute or preview
    * `:no_fields` — no fields had parseable values
    * `{:errors, [{field_str, msg_str}]}` — validation errors
    * `:unknown_schema` — schema module not found
  """
  def build_schema_query(schema_name, field_values) do
    mod = Module.concat([schema_name])

    if Rekto.Schema.has_schema?(mod) do
      info = Rekto.Schema.get_schema(mod)
      field_map = Map.new(info.fields, fn f -> {Atom.to_string(f.name), f} end)

      where_clause =
        Enum.flat_map(field_values, fn {name_str, value_str} ->
          with %{} = fi <- field_map[name_str],
               v when not is_nil(v) <- Neoplasm.Scanner.parse_value(value_str, fi.data_type) do
            [{fi.name, v}]
          else
            _ -> []
          end
        end)

      case where_clause do
        [] ->
          :no_fields

        _ ->
          expanded = Rekto.Schema.Helpers.infer_fields(mod, where_clause) |> Enum.to_list()

          case Rekto.Query.validate(mod, where: expanded) do
            [] ->
              {:ok, Rekto.Query.from(mod, where: expanded)}

            errors ->
              rows =
                for [field, _sep, msg] <- Rekto.Query.format_errors(errors), do: {field, msg}

              {:errors, rows}
          end
      end
    else
      :unknown_schema
    end
  end

  @doc """
  Serializes a space-separated list of typed values into a binary.

  `type_name` is `"array/{elem_type}/{count}"`.
  Returns `{:ok, binary}` or `:error`.
  """
  def serialize_array(type_name, value) do
    with ["array", elem_type_str, count_str] <- String.split(type_name, "/", parts: 3),
         {count, ""} <- Integer.parse(count_str),
         elem_type <- safe_to_existing_atom(elem_type_str),
         true <- is_atom(elem_type),
         elements <- String.split(value),
         true <- length(elements) == count,
         parsed when not is_nil(parsed) <-
           Enum.reduce_while(elements, [], fn v, acc ->
             case Neoplasm.Scanner.parse_value(v, elem_type) do
               nil -> {:halt, nil}
               val -> {:cont, [Rekto.Serialization.to_bytes(val, elem_type) | acc]}
             end
           end) do
      {:ok, parsed |> Enum.reverse() |> IO.iodata_to_binary()}
    else
      _ -> :error
    end
  end

  @doc false
  def safe_to_existing_atom(str) do
    String.to_existing_atom(str)
  rescue
    ArgumentError -> nil
  end

  @doc """
  Returns inferred-only field values for a schema given user-provided field strings.

  Returns a list of `{field_name_string, display_value_string}` for fields that were
  inferred (not explicitly provided by the user). Used by the GUI to show inference hints.
  """
  def infer_schema_fields(schema_name, field_values) do
    mod = Module.concat([schema_name])

    if Rekto.Schema.has_schema?(mod) and function_exported?(mod, :pattern_inference, 1) do
      info = Rekto.Schema.get_schema(mod)
      field_map = Map.new(info.fields, fn f -> {Atom.to_string(f.name), f} end)

      user_where =
        Enum.flat_map(field_values, fn {name_str, value_str} ->
          with %{} = fi <- field_map[name_str],
               v when not is_nil(v) <- Neoplasm.Scanner.parse_value(value_str, fi.data_type) do
            [{fi.name, v}]
          else
            _ -> []
          end
        end)

      merged = Rekto.Schema.Helpers.infer_fields(mod, user_where)
      user_keys = MapSet.new(Keyword.keys(user_where))

      merged
      |> Enum.reject(fn {k, _} -> MapSet.member?(user_keys, k) end)
      |> Enum.map(fn {k, v} -> {Atom.to_string(k), inspect(v)} end)
    else
      []
    end
  end
end
