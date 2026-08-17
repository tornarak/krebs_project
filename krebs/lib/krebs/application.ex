defmodule Krebs.Application do
  @moduledoc false

  use Application

  @impl Application
  def start(_type, _args) do
    mcp_port = Application.get_env(:krebs, :mcp_port, 4040)

    children = [
      {Krebs.LogBuffer, []},
      {Krebs.ScannerRegistry, []},
      {Krebs.ScanSetRegistry, []},
      {Bandit, scheme: :http, port: mcp_port, plug: {Vancouver.Router, tools: Krebs.MCP.tools()}}
    ]

    result = Supervisor.start_link(children, strategy: :one_for_one, name: Krebs.Supervisor)

    # Wire up logging after supervised processes are running.
    Logger.add_backend(Krebs.LogBuffer.Backend)
    Krebs.Nif.nif_log_init(Process.whereis(Krebs.LogBuffer))

    result
  end
end
