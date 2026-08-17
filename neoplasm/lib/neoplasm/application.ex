defmodule Neoplasm.Application do
  @moduledoc false

  use Application

  @impl true
  def start(_type, _args) do
    children = [
      Neoplasm.Scanner,
      Neoplasm.Window
    ]

    opts = [strategy: :one_for_one, name: Neoplasm.Supervisor]

    with {:ok, sup} <- Supervisor.start_link(children, opts) do
      maybe_auto_attach()
      {:ok, sup}
    end
  end

  defp maybe_auto_attach do
    {parsed, _, _} =
      OptionParser.parse(System.argv(),
        strict: [pid: :integer, name: :string]
      )

    cond do
      pid = parsed[:pid] ->
        send(Neoplasm.Window, {:neoplasm, :attach_requested, pid})

      name = parsed[:name] ->
        case Krebs.Scanner.search_processes(name) do
          [pid | _] ->
            send(Neoplasm.Window, {:neoplasm, :attach_requested, pid})

          [] ->
            require Logger
            Logger.warning("neoplasm: no process found matching #{inspect(name)}")
        end

      true ->
        :ok
    end

    :ok
  end
end
