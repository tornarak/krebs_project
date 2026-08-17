defmodule Rekto.SchemaRegistry do
  require Logger

  @moduledoc """
  ETS-backed registry of all compiled Rekto schemas.

  Schemas are discovered at application startup by scanning configured apps
  (default: `:rekto` plus any apps configured via `:rekto, :schema_apps`).

  Supports hot-reload by checking if the table exists in the postlude and
  registering individual schemas as they compile.
  """

  @doc """
  Starts the schema registry.

  Scans all modules in the configured apps and registers those that use
  `Rekto.Schema`.

  Returns `:ignore` since ETS table is persistent.
  """
  def start_link(_opts) do
    :ets.new(__MODULE__, [:named_table, :public])
    register_all_schemas()
    :ignore
  end

  @doc false
  def child_spec(opts) do
    %{
      id: __MODULE__,
      start: {__MODULE__, :start_link, [opts]},
      restart: :permanent,
      shutdown: :brutal_kill,
      type: :worker
    }
  end

  @doc """
  Registers a single schema module at runtime (for hot reload).

  Called by the schema macro postlude if the registry table exists.
  """
  @spec register(atom) :: true
  def register(module) when is_atom(module) do
    if Rekto.Schema.has_schema?(module) do
      :ets.insert(__MODULE__, {module})
    end
  end

  @doc """
  Returns all registered schema modules as a list.
  """
  @spec all :: [atom]
  def all do
    :ets.match_object(__MODULE__, {:"$1"})
    |> Enum.map(&elem(&1, 0))
    |> Enum.sort()
  end

  @doc """
  Returns a count of registered schemas.
  """
  @spec count :: non_neg_integer
  def count do
    :ets.info(__MODULE__, :size)
  end

  # Scans configured apps and registers all schemas
  @spec register_all_schemas :: :ok
  def register_all_schemas do
    apps = configured_schema_apps()

    Enum.each(apps, fn app ->
      case Application.spec(app, :modules) do
        nil ->
          :ok

        modules ->
          Enum.each(modules, fn module ->
            try do
              Code.ensure_loaded(module)
              register(module)
            rescue
              _ -> :ok
            end
          end)

          registered = Enum.count(modules, &Rekto.Schema.has_schema?/1)
          Logger.info("[Rekto.SchemaRegistry] #{app} — #{registered} schemas registered")
      end
    end)
  end

  # Get the list of apps to scan for schemas
  # Default: [:rekto] + any apps configured in rekto config
  @spec configured_schema_apps :: [atom]
  def configured_schema_apps do
    configured = Application.get_env(:rekto, :schema_apps, [])
    [:rekto | configured] |> Enum.uniq()
  end
end
