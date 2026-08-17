defmodule Rekto.Void do
  @moduledoc """
  Marker for untyped/dynamically-typed pointer targets.

  A special "placeholder type" with a sanity check that will raise an exception
  if it is instantiated.

  Intended for pointers that dynamically change type with a transformation.
  """

  use Rekto.Schema

  sanity(:always_fail)

  schema do
    field(:nothing, :u8)
  end

  @doc """
  A sanity check.

  Always fails.
  """
  def always_fail(_lmao) do
    raise ArgumentError, "Attempt to instantiate a void struct"
  end
end
