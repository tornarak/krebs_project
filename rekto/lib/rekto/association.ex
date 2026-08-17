defmodule Rekto.Association do
  @moduledoc false

  defmodule NotLoaded do
    @enforce_keys [:__meta__, :data_type]
    defstruct [:__meta__, :data_type]

    @type t ::
            %__MODULE__{
              __meta__: Rekto.Schema.Metadata.t(),
              data_type: Rekto.Serialization.datatype()
            }

    defimpl Inspect do
      import Inspect.Algebra
      alias Rekto.Schema.Helpers

      def inspect(
            %Rekto.Association.NotLoaded{
              __meta__: %Rekto.Schema.Metadata{addr: addr},
              data_type: type
            },
            opts
          ) do
        concat([
          "#Rekto.Association.NotLoaded<",
          to_doc(type, opts),
          " @ ",
          Helpers.print_ptr(addr),
          ">"
        ])
      end
    end
  end

  defmodule Primitive do
    @derive Inspect

    @enforce_keys [:__meta__, :data_type, :value]
    defstruct [:__meta__, :data_type, :value]

    @type t ::
            %__MODULE__{
              __meta__: Rekto.Schema.Metadata.t(),
              data_type: Rekto.Serialization.primitive(),
              value: any
            }
  end

  defmodule Error do
    @derive Inspect

    defstruct [:message]

    @type t :: %__MODULE__{message: any}
  end
end
