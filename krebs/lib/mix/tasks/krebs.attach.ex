defmodule Mix.Tasks.Krebs.Attach do
  use Mix.Task

  @shortdoc "Attach to a running process and open an IEx session"
  @moduledoc """
  Attaches a `Krebs.Scanner` to a running process and drops into IEx.

  ## Usage

      # By PID
      mix krebs.attach 1234

      # By window title or executable name (first match wins)
      mix krebs.attach --name RobloxPlayerBeta

      # Explicit --pid flag
      mix krebs.attach --pid 1234

  The following are pre-loaded in the IEx session:

  **Aliases:** `Scanner`, `HexDump`, `Repo`, `Query`, `ScanPattern`

  **Imported from `Scanner`:** `read/2`, `write/2`, `regions/0`, `modules/0`,
  `executable_name/0`, `pid/0`, `read_type/2`, `in_memory?/2`

      iex> executable_name()
      iex> read(0x1000, 64)
      iex> HexDump.hex_dump(0x1000, 0x40)
      iex> Repo.get(MySchema, 0x12345678)
  """

  @requirements ["compile"]

  alias Krebs.Nif

  def run(args) do
    {opts, positional, _} =
      OptionParser.parse(args, strict: [name: :string, pid: :integer])

    target_pid = resolve_pid(positional, opts)

    Application.put_env(:krebs, :attach_pid, target_pid)
    {:ok, _} = Application.ensure_all_started(:krebs)

    {:ok, _} = Krebs.Scanner.start_link(pid: target_pid)

    exe = Krebs.Scanner.executable_name()

    IO.puts(
      IO.ANSI.green() <> "[krebs] Attached to #{exe} (pid #{target_pid})" <> IO.ANSI.reset()
    )

    IO.puts("Aliases:  Scanner · HexDump · Repo · Query · ScanPattern")
    IO.puts("Imported: read/2 · write/2 · hex_dump/2,3 · regions/0 · modules/0 · pid/0")
  end

  # ── PID resolution ──────────────────────────────────────────────────────────

  defp resolve_pid([pid_str | _], _opts) do
    case Integer.parse(pid_str) do
      {n, ""} -> n
      _ -> Mix.raise("Invalid PID: #{pid_str}")
    end
  end

  defp resolve_pid([], opts) do
    cond do
      is_binary(opts[:name]) ->
        name = opts[:name]

        case Nif.search_windows(name) do
          [pid | _] ->
            pid

          [] ->
            case Nif.search_processes(name) do
              [pid | _] -> pid
              [] -> Mix.raise("No process found matching: #{name}")
            end
        end

      is_integer(opts[:pid]) ->
        opts[:pid]

      true ->
        Mix.raise("Usage: mix krebs.attach <pid> | --name <name> | --pid <n>")
    end
  end
end
