alias Krebs.Scanner
alias Krebs.ScanPattern

defmodule Bench do
  def scan_2_to_n_kb(num, pattern, pid) do
    num_kb = :math.pow(2, num) |> round
    num_bytes = round(1024 * num_kb)

    IO.puts "=== SCANNING WITH #{num_kb} KB READ SIZE ==="

    # To force garbage collection
    result =
      Task.async(fn ->
        {:ok, boomer} = Impl.new(pid, num_bytes)

        ret = {
          round(1024 * :math.pow(2, num)),
          :timer.tc(
            &Impl.scan/4,
            [
              boomer,
              pattern,
              self(),
              [print: true]
            ]
          )
        }

        :erlang.garbage_collect()

        ret
      end)
      |> Task.await()

    IO.puts("\n")

    result
  end
end

{opts, _, _} =
  System.argv()
  |> OptionParser.parse([strict: [exp: :integer, pid: :integer, pattern_str: :string]])

pid = opts[:pid] || raise ArgumentError, "PID required"

# Buffers are lazy & thread-local, so running w/ different buffer sizes in a row tells nothing

exp = opts[:exp] || raise ArgumentError, "Exponent required"
pattern_str = opts[:pattern_str] || "torrent"

pattern = ScanPattern.new(pattern_str)

Bench.scan_2_to_n_kb(exp, pattern, pid)
