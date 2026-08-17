defmodule Rekto.Application do
  @moduledoc false

  use Application

  @impl Application
  def start(_type, _args) do
    children = [
      Rekto.SchemaRegistry
    ]

    opts = [strategy: :one_for_one, name: Rekto.Supervisor]
    Supervisor.start_link(children, opts)
  end
end
