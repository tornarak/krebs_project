defmodule Rekto.Schema.ASTGen do
  @moduledoc false

  # AST generators for the `Rekto.Schema.__using__/1` macro

  # defp metadata_ast do
  #   {:%, [],
  #    [
  #      {:__aliases__, [alias: false], [:Rekto, :Schema, :Metadata]},
  #      {:%{}, [],
  #       [
  #         {:addr, nil},
  #         {:assoc_type, :none}
  #       ]}
  #    ]}
  # end

  def create_garbage_mask(fields_with_garbage) do
    for %{name: field_name, size: field_size} <- fields_with_garbage do
      case field_name do
        :__garbage__ ->
          String.duplicate(<<0x00>>, field_size)

        _ ->
          String.duplicate(<<0xFF>>, field_size)
      end
    end
    |> Enum.join("")
  end

  def bin_gen_ast_as_list(fields_with_garbage) do
    for %{name: field_name, size: field_size, data_type: field_type} <- fields_with_garbage do
      case field_name do
        :__garbage__ ->
          String.duplicate(<<0xCC>>, field_size)

        _ ->
          {:to_bytes, [], [{field_name, [], Elixir}, field_type]}
      end
    end
  end

  def bin_destruct_ast(fields_with_garbage) do
    field_clauses =
      for %{name: field_name, size: field_size} <- fields_with_garbage do
        {:"::", [],
         [
           {unless field_name == :__garbage__ do
              field_name
            else
              :_
            end, [], Elixir},
           {:-, [context: Elixir, import: Kernel],
            [{:binary, [], Elixir}, {:size, [], [field_size]}]}
         ]}
      end

    {:<<>>, [], field_clauses}
  end

  def fields_gen_ast(fields_in_offset_order) do
    for %{name: field_name, data_type: field_type} <- fields_in_offset_order do
      unless is_atom(field_type) and Rekto.Schema.has_schema?(field_type) do
        {field_name, {:from_bytes, [], [{field_name, [], Elixir}, field_type]}}
      else
        {field_name,
         {:from_bytes, [],
          [
            {field_name, [], Elixir},
            field_type,
            {:%{}, [], [assoc_type: :embed]}
          ]}}
      end
    end
  end

  defp assoc_grabber_ast do
    {
      :__meta__,
      {:%, [],
       [
         {:__aliases__, [alias: false], [:Rekto, :Schema, :Metadata]},
         {:%{}, [],
          [
            {:assoc_type, {:assoc_type, [], Elixir}}
          ]}
       ]}
    }
  end

  def struct_destruct_ast(fields_in_offset_order) do
    struct_fields_as_var_names =
      for %{name: field_name} <- fields_in_offset_order do
        {field_name, {field_name, [], Elixir}}
      end

    {:%{}, [], [assoc_grabber_ast() | struct_fields_as_var_names]}
  end
end
