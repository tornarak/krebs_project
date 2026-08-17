defmodule Krebs.Repo do
  @moduledoc """
  A `Rekto.Repo` implementation backed by `Krebs.Scanner`.

  `Krebs.Repo` is a plain module (no process). `read_bytes/2` and
  `execute_query/2` delegate directly to the default `Krebs.Scanner`
  instance. The full `Rekto.Repo` API (`get/3`, `all/2`, `one/2`,
  `preload/2`) is available via `use Rekto.Repo`.

  ## Examples

      # Read a struct from memory
      Krebs.Repo.get(MySchema, 0x12345678)

      # Query by field value — scans heap by default
      query = Rekto.Query.from(MySchema, where: [hp: 100])
      {:ok, results} = Krebs.Repo.all(query)

      # Scan module memory instead
      {:ok, results} = Krebs.Repo.all(query, mem_type: :module)

  ## Options

  Options accepted by `execute_query/2` (and propagated through `all/2`,
  `one/2`):

    * `:mem_type` — `:heap` (default) or `:module`
    * `:scanner`  — named scanner to target (default: `Krebs.Scanner`)

  """

  use Rekto.Repo

  require Logger

  alias Krebs.{Scanner, ScanMatch, ScanPattern}
  alias Rekto.{Query}
  alias Rekto.Schema.Helpers

  # — Rekto.Repo callbacks ──────────────────────────────────────────────────

  @impl Rekto.Repo
  def read_bytes(addr, n, opts \\ [])

  def read_bytes(0, _n, _opts), do: {:error, :null_ptr}

  def read_bytes(addr, n, opts) when is_integer(addr) and addr > 0 and is_integer(n) and n > 0 do
    scanner = Keyword.get(opts, :scanner, Krebs.Scanner)
    Scanner.read(scanner, addr, n)
  end

  def stream_query(%Query{schema: schema, where: where} = query, opts \\ []) do
    scanner = Keyword.get(opts, :scanner, Krebs.Scanner)
    mem_type = Keyword.get(opts, :mem_type, :heap)

    Logger.info(
      "[Repo] stream_query schema=#{inspect(schema)} where=#{inspect(where)} mem_type=#{mem_type} scanner=#{inspect(scanner)}"
    )

    {pattern, offset} = query_to_scan_pattern(query)

    significant =
      case pattern do
        %{mask: nil, bytes: bytes} ->
          byte_size(bytes)

        %{mask: mask} ->
          mask |> :erlang.binary_to_list() |> Enum.count(&(&1 != 0))
      end

    Logger.debug(
      "[Repo] full struct=#{Rekto.Serialization.get_type_size!(schema)}b significant=#{significant}b"
    )

    Logger.info("[Repo] pattern offset=+#{offset}b #{inspect(pattern)}")

    Scanner.scan_stream(scanner, pattern, mem_type: mem_type)
    |> Stream.map(fn
      %ScanMatch{addr: addr} -> addr - offset
      {:error, _} = err -> err
    end)
  end

  @impl Rekto.Repo
  def execute_query(%Query{schema: schema} = query, opts \\ []) do
    result =
      stream_query(query, opts)
      |> Enum.reduce_while({:ok, []}, fn
        {:error, reason}, _ -> {:halt, {:error, reason}}
        addr, {:ok, acc} -> {:cont, {:ok, [addr | acc]}}
      end)

    case result do
      {:ok, addrs} ->
        addrs = Enum.reverse(addrs)

        Logger.debug(
          "[Repo] query done — #{length(addrs)} match#{if length(addrs) == 1, do: "", else: "es"} schema=#{inspect(schema)}"
        )

        {:ok, addrs}

      {:error, reason} ->
        Logger.error("[Repo] query failed schema=#{inspect(schema)} reason=#{inspect(reason)}")
        {:error, reason}
    end
  end

  @doc "Sugar for `get/3` when you already have a struct and want to reread it as another schema."
  def convert(%{__meta__: %{addr: addr}}, schema, opts \\ []) do
    get(schema, addr, opts)
  end

  # — Private: query execution ──────────────────────────────────────────────

  @doc """
  Returns the full struct layout as a space-separated hex string for the given query.

  Bytes that will be compared carry their hex value; bytes outside the scan window
  (leading offset and trailing remainder) are shown as `??`.
  """
  def pattern_hex_for_query(%Query{schema: schema} = query) do
    total_size = schema.__size__()
    {%ScanPattern{bytes: bytes, mask: pat_mask}, offset} = query_to_scan_pattern(query)
    pattern_size = byte_size(bytes)
    trailing = total_size - offset - pattern_size

    mask_list =
      if pat_mask,
        do: :erlang.binary_to_list(pat_mask),
        else: List.duplicate(0xFF, pattern_size)

    middle =
      :erlang.binary_to_list(bytes)
      |> Enum.zip(mask_list)
      |> Enum.map(fn
        {_, 0} -> "??"
        {b, _} -> b |> Integer.to_string(16) |> String.pad_leading(2, "0") |> String.upcase()
      end)

    (List.duplicate("??", offset) ++ middle ++ List.duplicate("??", trailing))
    |> Enum.join(" ")
  end

  defp query_to_scan_pattern(%Query{schema: schema, where: where}) do
    {as_bytes, mask} = Helpers.to_masked_bytes(schema, where)

    {bytes, trunc_mask, offset} = truncate_significant_bytes(as_bytes, mask)

    final_mask =
      if Enum.all?(:erlang.binary_to_list(trunc_mask), &(&1 == 0xFF)), do: nil, else: trunc_mask

    pattern = ScanPattern.new(bytes, final_mask)

    {pattern, offset}
  end

  defp truncate_significant_bytes(as_bytes, mask) do
    mask_list = :erlang.binary_to_list(mask)
    first = Enum.find_index(mask_list, &(&1 == 0xFF))
    last = length(mask_list) - 1 - Enum.find_index(Enum.reverse(mask_list), &(&1 == 0xFF))
    len = last - first + 1
    {binary_part(as_bytes, first, len), binary_part(mask, first, len), first}
  end
end
