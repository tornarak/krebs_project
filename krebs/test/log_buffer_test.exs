defmodule LogBufferTest do
  use ExUnit.Case, async: false

  # LogBuffer uses a named ETS table and a named GenServer, so tests must be
  # sequential to avoid conflicts. We restart the process around each test.

  setup do
    # Start a fresh LogBuffer for each test under a unique name
    name = :"log_buffer_#{System.unique_integer([:positive])}"
    pid = start_supervised!({Krebs.LogBuffer, name: name})
    %{pid: pid, name: name}
  end

  defp entry(level, message, source \\ :elixir) do
    %{
      ts: "2024-01-01T00:00:00Z",
      level: level,
      source: source,
      module: "Test",
      message: message
    }
  end

  test "starts empty", %{pid: pid} do
    assert Krebs.LogBuffer.get_recent(50, pid) == []
  end

  test "insert and retrieve one entry", %{pid: pid} do
    e = entry(:info, "hello")
    Krebs.LogBuffer.insert(e, pid)
    # cast is async — allow the GenServer to process it
    entries = Krebs.LogBuffer.get_recent(50, pid)
    assert [^e] = entries
  end

  test "entries returned oldest first", %{pid: pid} do
    Krebs.LogBuffer.insert(entry(:info, "first"), pid)
    Krebs.LogBuffer.insert(entry(:info, "second"), pid)
    Krebs.LogBuffer.insert(entry(:info, "third"), pid)

    [a, b, c] = Krebs.LogBuffer.get_recent(50, pid)
    assert a.message == "first"
    assert b.message == "second"
    assert c.message == "third"
  end

  test "get_recent respects n limit", %{pid: pid} do
    for i <- 1..5, do: Krebs.LogBuffer.insert(entry(:info, "msg #{i}"), pid)

    entries = Krebs.LogBuffer.get_recent(3, pid)
    assert length(entries) == 3
    assert Enum.map(entries, & &1.message) == ["msg 3", "msg 4", "msg 5"]
  end

  test "clear removes all entries", %{pid: pid} do
    Krebs.LogBuffer.insert(entry(:info, "hello"), pid)
    Krebs.LogBuffer.clear(pid)
    assert Krebs.LogBuffer.get_recent(50, pid) == []
  end

  test "ring buffer caps at max entries", %{pid: pid} do
    # Insert 510 entries (max is 500)
    for i <- 1..510, do: Krebs.LogBuffer.insert(entry(:info, "msg #{i}"), pid)

    entries = Krebs.LogBuffer.get_recent(600, pid)
    assert length(entries) == 500
    # Oldest surviving entry should be msg 11, newest msg 510
    assert hd(entries).message == "msg 11"
    assert List.last(entries).message == "msg 510"
  end
end
