defmodule Rekto.Schema.Info do
  @moduledoc """
  Metadata about a Rekto schema, returned by Schema.__schema__() and
  accessible via Schema.get_schema/1.
  """

  alias Rekto.Schema.{Constraints, FieldInfo}

  @type t :: %__MODULE__{
          size: pos_integer,
          fields: [FieldInfo.t()],
          fields_incl_garbage: [FieldInfo.t()],
          field_constraints: [{atom, Constraints.field_constraint()}],
          sanity_checks: [{module, atom}],
          transformations: [{module, atom}],
          garbage_mask: binary
        }

  defstruct [
    :size,
    :fields,
    :fields_incl_garbage,
    :field_constraints,
    :sanity_checks,
    :transformations,
    :garbage_mask
  ]
end
