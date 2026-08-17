defmodule Krebs.HexDump do
  @moduledoc """
  Hex dump utility with type inference, built on top of `Krebs.Scanner`.

  ## Usage

      iex> Krebs.HexDump.hex_dump(0x12345678, 0x20)
      # 4-byte words (32-bit pointer inference)

      iex> Krebs.HexDump.hex_dump(0x12345678, 0x20, 8)
      # 8-byte words (64-bit pointer inference + sub-word 4-byte values)

      iex> Krebs.HexDump.hex_dump(:my_scanner, 0x12345678, 0x20, 8)
      # named scanner instance, explicit word size
  """

  alias Krebs.Scanner

  # ── Inference helpers ───────────────────────────────────────────────────────

  # Types to try for a given chunk byte size. Sub-word sizes are included so
  # that e.g. a 4-byte int stored in an 8-byte aligned slot is still detected.
  #
  # Pointer pseudo-types (:heap, :module) are placed at the native word size
  # only — sub-word pointers aren't a thing on any platform we care about.
  defp infer_for_size(1), do: [:u8, :i8]
  defp infer_for_size(2), do: [:u16, :i16, :u8, :i8]
  defp infer_for_size(4), do: [:u32, :i32, :f32, :heap, :module]
  defp infer_for_size(8), do: [:u64, :i64, :f64, :heap, :module, :u32, :i32, :f32]
  defp infer_for_size(_), do: [:heap, :module]

  defp ptr_type_for_size(4), do: :u32
  defp ptr_type_for_size(8), do: :u64
  defp ptr_type_for_size(_), do: :u32

  # ── Private helpers ────────────────────────────────────────────────────────

  defp zip_with_offsets(bin_list, bin_size) do
    offsets = Enum.map(0..(Enum.count(bin_list) - 1), &(&1 * bin_size))
    Enum.zip(bin_list, offsets)
  end

  defp bytes_to_str(bytes, sep \\ " ") do
    bytes
    |> :erlang.binary_to_list()
    |> Enum.map(&Integer.to_string(&1, 16))
    |> Enum.map(&String.pad_leading(&1, 2, "0"))
    |> Enum.join(sep)
  end

  # Carriage returns and other control characters corrupt terminal output.
  # Learned the hard way.
  defp chars_to_str(bytes) do
    bytes
    |> :erlang.binary_to_list()
    |> Enum.map(&if &1 in 32..126, do: <<&1>>, else: ".")
    |> Enum.join()
  end

  defp pad_offset(offset, width) do
    offset |> Integer.to_string(16) |> String.pad_leading(width, "0")
  end

  # ── Plausibility filters ───────────────────────────────────────────────────

  defp plausible_inference({:u8, v}, _) when is_integer(v), do: v in 1..255
  defp plausible_inference({:i8, v}, _) when is_integer(v), do: abs(v) in 1..127
  defp plausible_inference({:u16, v}, _) when is_integer(v), do: v in 1..10_000
  defp plausible_inference({:i16, v}, _) when is_integer(v), do: abs(v) in 1..10_000
  defp plausible_inference({:u32, v}, _) when is_integer(v), do: v in 1..10_000
  defp plausible_inference({:i32, v}, _) when is_integer(v), do: abs(v) in 1..10_000
  defp plausible_inference({:u64, v}, _) when is_integer(v), do: v in 1..10_000
  defp plausible_inference({:i64, v}, _) when is_integer(v), do: abs(v) in 1..10_000

  defp plausible_inference({:f32, f}, _) when is_float(f) do
    a = abs(f)
    a >= 0.1 and a <= 100_000.0
  end

  defp plausible_inference({:f64, f}, _) when is_float(f) do
    a = abs(f)
    a >= 0.1 and a <= 100_000.0
  end

  defp plausible_inference({:heap, addr}, server),
    do: match?({:ok, true}, Scanner.in_memory?(server, addr, :heap))

  defp plausible_inference({:module, addr}, server),
    do: match?({:ok, true}, Scanner.in_memory?(server, addr, :module))

  defp plausible_inference(_, _), do: true

  # ── Type inference ─────────────────────────────────────────────────────────

  defp infer_type(bytes, :heap) do
    ptr_type = ptr_type_for_size(byte_size(bytes))
    {:ok, addr} = Rekto.Serialization.from_bytes(bytes, ptr_type)
    {:heap, addr}
  end

  defp infer_type(bytes, :module) do
    ptr_type = ptr_type_for_size(byte_size(bytes))
    {:ok, addr} = Rekto.Serialization.from_bytes(bytes, ptr_type)
    {:module, addr}
  end

  defp infer_type(bytes, type) do
    # Slice to the type's natural size so sub-word types work correctly
    # when `bytes` is wider (e.g. u32 inside an 8-byte window).
    type_size = Rekto.Serialization.get_type_size!(type)
    slice = binary_part(bytes, 0, type_size)

    value =
      case Rekto.Serialization.from_bytes(slice, type) do
        {:ok, v} -> v
        _ -> :error
      end

    {type, value}
  end

  # ── Printing ───────────────────────────────────────────────────────────────

  defp color(ansi, text), do: [ansi, text, IO.ANSI.reset()]

  defp print_dump({bytes, _} = t, base_addr, :inference, server) do
    type_values =
      bytes
      |> byte_size()
      |> infer_for_size()
      |> Enum.map(&infer_type(bytes, &1))
      |> Enum.reject(fn {_, v} -> v == :error end)
      |> Enum.filter(&plausible_inference(&1, server))

    print_dump(t, base_addr, type_values, server)
  end

  defp print_dump({bytes, _} = t, base_addr, type, server) when is_atom(type) do
    print_dump(t, base_addr, [infer_type(bytes, type)], server)
  end

  defp print_dump({bytes, offset}, base_addr, type_values, _server) when is_list(type_values) do
    [
      color(
        IO.ANSI.bright() <> IO.ANSI.white(),
        Rekto.Schema.Helpers.print_ptr(base_addr + offset)
      ),
      "  ",
      color(IO.ANSI.light_green(), pad_offset(offset, 4)),
      "  ",
      color(IO.ANSI.white(), bytes_to_str(bytes)),
      "  ",
      color(IO.ANSI.green(), chars_to_str(bytes)),
      "\t",
      if type_values == [] do
        color(IO.ANSI.light_black(), "N/A")
      else
        color(IO.ANSI.cyan(), inspect(type_values))
      end
    ]
  end

  # ── Public API ─────────────────────────────────────────────────────────────
  #
  # Dispatch is done by type rather than position, so both of these work
  # without ambiguity:
  #
  #   hex_dump(addr, n)            # implicit Krebs.Scanner, word_size=4
  #   hex_dump(addr, n, 8)         # implicit Krebs.Scanner, word_size=8
  #   hex_dump(:scanner, addr, n)  # explicit server, word_size=4
  #   hex_dump(:scanner, addr, n, 8)
  #
  # The key invariant: `addr` is always an integer, `server` never is.

  @doc """
  Prints an annotated hex dump of `n_bytes` bytes starting at `addr`.

  Options:
    * `:word_size` — row width in bytes: `4` (default, 32-bit) or `8` (64-bit).
      Controls grouping width and type inference.

  `word_size` defaults to the configured `:word_type`.
  """
  @spec hex_dump(Scanner.addr(), pos_integer, keyword) ::
          :"do not show this result in output"
  @spec hex_dump(GenServer.server(), Scanner.addr(), pos_integer, keyword) ::
          :"do not show this result in output"

  def hex_dump(addr, n_bytes) when is_integer(addr) and is_integer(n_bytes),
    do: do_hex_dump(Krebs.Scanner, addr, n_bytes, Rekto.Serialization.get_word_size())

  def hex_dump(addr, n_bytes, opts)
      when is_integer(addr) and is_integer(n_bytes) and is_list(opts),
      do:
        do_hex_dump(
          Krebs.Scanner,
          addr,
          n_bytes,
          opts[:word_size] || Rekto.Serialization.get_word_size()
        )

  def hex_dump(server, addr, n_bytes) when is_integer(addr) and is_integer(n_bytes),
    do: do_hex_dump(server, addr, n_bytes, Rekto.Serialization.get_word_size())

  def hex_dump(server, addr, n_bytes, opts)
      when is_integer(addr) and is_integer(n_bytes) and is_list(opts),
      do:
        do_hex_dump(
          server,
          addr,
          n_bytes,
          opts[:word_size] || Rekto.Serialization.get_word_size()
        )

  @doc """
  Returns hex dump as a string (with type inference) instead of printing.

  Options:
    * `:word_size` — row width in bytes (default: configured `:word_type`)
  """
  def hex_dump_to_string(addr, n_bytes) when is_integer(addr) and is_integer(n_bytes),
    do: do_hex_dump_to_string(Krebs.Scanner, addr, n_bytes, Rekto.Serialization.get_word_size())

  def hex_dump_to_string(addr, n_bytes, opts)
      when is_integer(addr) and is_integer(n_bytes) and is_list(opts),
      do:
        do_hex_dump_to_string(
          Krebs.Scanner,
          addr,
          n_bytes,
          opts[:word_size] || Rekto.Serialization.get_word_size()
        )

  def hex_dump_to_string(server, addr, n_bytes) when is_integer(addr) and is_integer(n_bytes),
    do: do_hex_dump_to_string(server, addr, n_bytes, Rekto.Serialization.get_word_size())

  def hex_dump_to_string(server, addr, n_bytes, opts)
      when is_integer(addr) and is_integer(n_bytes) and is_list(opts),
      do:
        do_hex_dump_to_string(
          server,
          addr,
          n_bytes,
          opts[:word_size] || Rekto.Serialization.get_word_size()
        )

  defp do_hex_dump_to_string(server, addr, n_bytes, word_size) do
    case build_hex_dump_lines(server, addr, n_bytes, word_size) do
      {:ok, lines} -> Enum.join(lines, "\n")
      {:error, reason} -> "error: #{inspect(reason)}"
    end
  end

  defp do_hex_dump(server, addr, n_bytes, word_size) when rem(n_bytes, word_size) == 0 do
    case build_hex_dump_lines(server, addr, n_bytes, word_size) do
      {:ok, lines} -> Enum.each(lines, &IO.puts/1)
      {:error, reason} -> IO.puts("error: #{inspect(reason)}")
    end

    :"do not show this result in output"
  end

  # Core: reads memory, chunks by word_size, runs type inference per chunk.
  # Returns {:ok, [{abs_addr, chunk_binary, [{type, value}]}]} or {:error, _}.
  defp build_rows(server, addr, n_bytes, word_size) when rem(n_bytes, word_size) == 0 do
    with {:ok, bytes} <- Scanner.read(server, addr, n_bytes) do
      rows =
        bytes
        |> :erlang.binary_to_list()
        |> Enum.chunk_every(word_size)
        |> Enum.map(&:erlang.list_to_binary/1)
        |> zip_with_offsets(word_size)
        |> Enum.map(fn {chunk, offset} ->
          type_values =
            chunk
            |> byte_size()
            |> infer_for_size()
            |> Enum.map(&infer_type(chunk, &1))
            |> Enum.reject(fn {_, v} -> v == :error end)
            |> Enum.filter(&plausible_inference(&1, server))

          {addr + offset, chunk, type_values}
        end)

      {:ok, rows}
    end
  end

  # Terminal output: formats structured rows as ANSI-coloured lines.
  defp build_hex_dump_lines(server, addr, n_bytes, word_size) when rem(n_bytes, word_size) == 0 do
    with {:ok, rows} <- build_rows(server, addr, n_bytes, word_size) do
      lines =
        Enum.map(rows, fn {abs_addr, chunk, type_values} ->
          offset = abs_addr - addr
          print_dump({chunk, offset}, addr, type_values, server)
        end)

      {:ok, lines}
    end
  end

  # ── Structured rows (for GUI rendering) ───────────────────────────────────

  @doc """
  Returns structured hex dump rows for GUI rendering.

  Each element is `{abs_addr, byte_list, [annotation_string]}`.
  Shares the same read + inference pipeline as `hex_dump/2`.

  Options:
    * `:word_size` — bytes per row (default: configured `:word_type`)
  """
  @spec hex_dump_rows(Scanner.addr(), pos_integer, keyword) ::
          {:ok, [{non_neg_integer, list(byte), [String.t()]}]} | {:error, any}
  @spec hex_dump_rows(GenServer.server(), Scanner.addr(), pos_integer, keyword) ::
          {:ok, [{non_neg_integer, list(byte), [String.t()]}]} | {:error, any}

  def hex_dump_rows(addr, n_bytes) when is_integer(addr) and is_integer(n_bytes),
    do: do_hex_dump_rows(Krebs.Scanner, addr, n_bytes, Rekto.Serialization.get_word_size())

  def hex_dump_rows(addr, n_bytes, opts) when is_integer(addr) and is_list(opts),
    do:
      do_hex_dump_rows(
        Krebs.Scanner,
        addr,
        n_bytes,
        opts[:word_size] || Rekto.Serialization.get_word_size()
      )

  def hex_dump_rows(server, addr, n_bytes) when is_integer(addr),
    do: do_hex_dump_rows(server, addr, n_bytes, Rekto.Serialization.get_word_size())

  def hex_dump_rows(server, addr, n_bytes, opts) when is_integer(addr) and is_list(opts),
    do:
      do_hex_dump_rows(
        server,
        addr,
        n_bytes,
        opts[:word_size] || Rekto.Serialization.get_word_size()
      )

  defp format_annotation({kind, addr}) when kind in [:heap, :module],
    do: "#{kind}: 0x#{Integer.to_string(addr, 16)}"

  defp format_annotation({type, val}),
    do: "#{type}: #{inspect(val)}"

  defp do_hex_dump_rows(server, addr, n_bytes, word_size) do
    n_aligned = div(n_bytes + word_size - 1, word_size) * word_size

    with {:ok, rows} <- build_rows(server, addr, n_aligned, word_size) do
      gui_rows =
        Enum.map(rows, fn {abs_addr, chunk, type_values} ->
          annotations = Enum.map(type_values, &format_annotation/1)
          {abs_addr, :erlang.binary_to_list(chunk), annotations}
        end)

      {:ok, gui_rows}
    end
  end

  @doc """
  Prints a typed dump of `n_bytes` bytes starting at `addr`, interpreting
  each `type`-sized chunk as `type`.
  """
  @spec type_dump(Scanner.addr(), pos_integer, atom) :: :"do not show this result in output"
  @spec type_dump(GenServer.server(), Scanner.addr(), pos_integer, atom) ::
          :"do not show this result in output"

  def type_dump(addr, n_bytes, type) when is_integer(addr) and is_integer(n_bytes),
    do: do_type_dump(Krebs.Scanner, addr, n_bytes, type)

  def type_dump(server, addr, n_bytes, type) when is_integer(addr) and is_integer(n_bytes),
    do: do_type_dump(server, addr, n_bytes, type)

  defp do_type_dump(server, addr, n_bytes, type) do
    type_size = Rekto.Serialization.get_type_size!(type)

    unless rem(n_bytes, type_size) == 0,
      do:
        raise(
          ArgumentError,
          "n_bytes (#{n_bytes}) must be a multiple of type size (#{type_size})"
        )

    {:ok, bytes} = Scanner.read(server, addr, n_bytes)

    bytes
    |> :erlang.binary_to_list()
    |> Enum.chunk_every(type_size)
    |> Enum.map(&:erlang.list_to_binary/1)
    |> zip_with_offsets(type_size)
    |> Enum.map(&print_dump(&1, addr, type, server))
    |> Enum.each(&IO.puts/1)

    :"do not show this result in output"
  end
end
