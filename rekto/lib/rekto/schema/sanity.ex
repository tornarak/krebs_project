defmodule Rekto.Schema.Sanity do
  require Logger

  @moduledoc """
  Built-in sanity check implementations injected by schema DSL primitives.
  """

  @doc """
  Validates all `:this_pointer` fields in a struct against the struct's own address.

  Skips the check when `__meta__.addr` is `nil` (addr not known at deserialization time).
  Called automatically as a single sanity check when any field in the schema uses `:this_pointer`.
  """
  def check_this_pointers(%{__meta__: %{addr: nil}}), do: :ok

  def check_this_pointers(%{__struct__: mod} = struct) do
    addr = struct.__meta__.addr

    errors =
      mod.__schema__().fields
      |> Enum.filter(fn %{opts: opts} -> Keyword.get(opts, :this_pointer, false) end)
      |> Enum.flat_map(fn %{name: name} ->
        value = Map.get(struct, name)
        offset = Rekto.Schema.Helpers.resolve_pointer_offset(mod, name)

        if addr + offset == value,
          do: [],
          else: [{name, {:this_pointer_mismatch, addr, offset, value}}]
      end)

    if errors == [] do
      :ok
    else
      Logger.warning(
        "[Rekto.Schema.Sanity] #{inspect(struct.__struct__)} @ 0x#{Integer.to_string(addr, 16)} this-pointer mismatch: #{inspect(errors)}"
      )

      {:error, errors}
    end
  end
end
