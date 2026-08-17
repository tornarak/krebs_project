defmodule Rekto.Schema.Constraints do
  @moduledoc """
  <span style = "color:#789922;">&gt;tfw you accidentally reinvent lisp</span>
  """

  @type valid_return :: true | false | :ok | :error | {:ok, any} | {:error, any}
  @type field_constraint ::
          :non_null
          | {:const, any}
          | {:not_const, any}
          | {:range, {number, number}}
          | {:func, (any -> valid_return)}
          | {:not, field_constraint}
          | {:and, {field_constraint, field_constraint}}
          | {:or, {field_constraint, field_constraint}}
          | {:xor, {field_constraint, field_constraint}}

  @doc """
  Returns `:ok` if the given value matches the constraint, and `{:error, reason}` otherwise.

  Error reasons are structured tuples (not strings), enabling pattern-matching by callers.

  ## Examples

      iex> check_field_constraint(:non_null, 0)
      {:error, {:non_null, 0}}
      iex> check_field_constraint(:non_null, 1)
      :ok

      iex> check_field_constraint({:const, 20}, 20)
      :ok
      iex> check_field_constraint({:const, 20}, 21)
      {:error, {:expected_const, 20, 21}}

      iex> check_field_constraint({:not_const, 20}, 21)
      :ok
      iex> check_field_constraint({:not_const, 20}, 20)
      {:error, {:unexpected_const, 20}}

      iex> check_field_constraint({:in, [1, 2, 3]}, 2)
      :ok
      iex> check_field_constraint({:in, [1, 2, 3]}, 4)
      {:error, {:not_in, 4, [1, 2, 3]}}

      iex> check_field_constraint({:range, {20, 100}}, 20)
      :ok
      iex> check_field_constraint({:range, {20, 100}}, 60)
      :ok
      iex> check_field_constraint({:range, {20, 100}}, 100)
      :ok
      iex> check_field_constraint({:range, {20, 100}}, 101)
      {:error, {:out_of_range, 101, 20, 100}}

      iex> check_field_constraint({:func, &String.printable?/1}, "Hello")
      :ok
      iex> check_field_constraint({:func, &String.printable?/1}, <<?H, ?e, 0, ?l, ?l, ?o>>)
      {:error, {:func_false, &String.printable?/1, <<72, 101, 0, 108, 108, 111>>}}

      iex> check_field_constraint({:not, {:const, 32}}, 20)
      :ok
      iex> check_field_constraint({:not, {:const, 32}}, 32)
      {:error, {:constraint_must_not_hold, {:const, 32}}}

      iex> compound_expression = {
      ...>   {:range, {20, 40}},
      ...>   {:range, {30, 50}}
      ...> }
      {{:range, {20, 40}}, {:range, {30, 50}}}
      iex> check_field_constraint({:and, compound_expression}, 32)
      :ok
      iex> check_field_constraint({:and, compound_expression}, 50)
      {:error, {:and_failed, {:error, {:out_of_range, 50, 20, 40}}, :ok}}
      iex> check_field_constraint({:and, compound_expression}, 23)
      {:error, {:and_failed, :ok, {:error, {:out_of_range, 23, 30, 50}}}}
      iex> check_field_constraint({:or, compound_expression}, 23)
      :ok
      iex> check_field_constraint({:or, compound_expression}, 47)
      :ok
      iex> check_field_constraint({:or, compound_expression}, 60)
      {:error,
       {:or_failed, {:error, {:out_of_range, 60, 20, 40}},
        {:error, {:out_of_range, 60, 30, 50}}}}
      iex> check_field_constraint({:xor, compound_expression}, 23)
      :ok
      iex> check_field_constraint({:xor, compound_expression}, 47)
      :ok
      iex> check_field_constraint({:xor, compound_expression}, 33)
      {:error, {:xor_failed, :ok, :ok}}
      iex> check_field_constraint({:xor, compound_expression}, 60)
      {:error,
       {:xor_failed, {:error, {:out_of_range, 60, 20, 40}},
        {:error, {:out_of_range, 60, 30, 50}}}}

  """
  @spec check_field_constraint(field_constraint, any) :: :ok | {:error, any}
  def check_field_constraint(:non_null, 0), do: {:error, {:non_null, 0}}
  def check_field_constraint(:non_null, nil), do: {:error, {:non_null, nil}}
  def check_field_constraint(:non_null, _), do: :ok

  def check_field_constraint({:const, expected}, value) do
    case value do
      ^expected -> :ok
      _ -> {:error, {:expected_const, expected, value}}
    end
  end

  def check_field_constraint({:not_const, haram}, value) do
    case value do
      ^haram -> {:error, {:unexpected_const, haram}}
      _ -> :ok
    end
  end

  def check_field_constraint({:in, list}, value) when is_list(list) do
    if value in list do
      :ok
    else
      {:error, {:not_in, value, list}}
    end
  end

  @dialyzer {:no_match, check_field_constraint: 2}
  def check_field_constraint({:not, subconst}, value) do
    case check_field_constraint(subconst, value) do
      :ok -> {:error, {:constraint_must_not_hold, subconst}}
      {:error, _} -> :ok
      # Defensive: dialyzer proves this is unreachable but we keep it as a safety net.
      _ -> raise ArgumentError, "Constraint #{inspect(subconst)} does not return valid status"
    end
  end

  def check_field_constraint({:and, {sub0, sub1}}, value) do
    case {check_field_constraint(sub0, value), check_field_constraint(sub1, value)} do
      {:ok, :ok} -> :ok
      {sub0_result, sub1_result} -> {:error, {:and_failed, sub0_result, sub1_result}}
    end
  end

  def check_field_constraint({:or, {sub0, sub1}}, value) do
    case {check_field_constraint(sub0, value), check_field_constraint(sub1, value)} do
      {:ok, _} -> :ok
      {_, :ok} -> :ok
      {sub0_fail, sub1_fail} -> {:error, {:or_failed, sub0_fail, sub1_fail}}
    end
  end

  def check_field_constraint({:xor, {sub0, sub1}}, value) do
    case {check_field_constraint(sub0, value), check_field_constraint(sub1, value)} do
      {:ok, :ok} -> {:error, {:xor_failed, :ok, :ok}}
      {:ok, _} -> :ok
      {_, :ok} -> :ok
      {sub0_fail, sub1_fail} -> {:error, {:xor_failed, sub0_fail, sub1_fail}}
    end
  end

  def check_field_constraint({:range, {min, max}}, value) when min <= max do
    if value >= min and value <= max do
      :ok
    else
      {:error, {:out_of_range, value, min, max}}
    end
  end

  def check_field_constraint({:range, {min, max}}, _),
    do: raise(ArgumentError, "min (#{min}) must be <= max (#{max})")

  def check_field_constraint({:func, fnc}, value) when is_function(fnc, 1) do
    case fnc.(value) do
      true ->
        :ok

      :ok ->
        :ok

      {:ok, _} ->
        :ok

      false ->
        {:error, {:func_false, fnc, value}}

      :error ->
        {:error, {:func_error, fnc, value}}

      {:error, msg} ->
        {:error, {:func_error, fnc, value, msg}}

      _ ->
        raise ArgumentError,
              "Supplied function must return a boolean, :ok, :error, or {:ok | :error, message}"
    end
  end

  # def check_field_constraint(field_constraint, _),
  #   do:
  #     raise(
  #       ArgumentError,
  #       "Invalid constraint #{inspect(field_constraint)} not found (constraints are of the form {:field_constraint_name, true_args_or_fn})"
  #     )
end
