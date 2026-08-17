defmodule Krebs.LogBuffer.Backend do
  @moduledoc false
  # Logger backend that feeds Elixir log events into Krebs.LogBuffer.
  # Registered via Logger.add_backend/1 from Krebs.Application.start/2.

  @behaviour :gen_event

  @impl :gen_event
  def init({__MODULE__, _opts}), do: {:ok, []}
  def init(__MODULE__), do: {:ok, []}

  @impl :gen_event
  def handle_event({level, _gl, {Logger, message, _ts, metadata}}, state) do
    entry = %{
      ts: DateTime.utc_now() |> DateTime.to_iso8601(),
      level: level,
      source: :elixir,
      module: inspect(metadata[:module]),
      message: IO.chardata_to_string(message)
    }

    Krebs.LogBuffer.insert(entry)
    {:ok, state}
  end

  def handle_event(_, state), do: {:ok, state}

  @impl :gen_event
  def handle_call({:configure, _opts}, state), do: {:ok, :ok, state}

  @impl :gen_event
  def handle_info(_, state), do: {:ok, state}

  @impl :gen_event
  def code_change(_old, state, _extra), do: {:ok, state}

  @impl :gen_event
  def terminate(_reason, _state), do: :ok
end
