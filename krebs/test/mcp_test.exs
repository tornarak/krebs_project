defmodule MCPTest do
  use ExUnit.Case, async: true

  alias Krebs.MCP

  describe "parse_hex_pattern/1" do
    test "parses a simple byte sequence" do
      assert {:ok, {<<0x48, 0x8B, 0x05>>, nil}} = MCP.parse_hex_pattern("48 8B 05")
    end

    test "accepts lowercase hex" do
      assert {:ok, {<<0xDE, 0xAD>>, nil}} = MCP.parse_hex_pattern("de ad")
    end

    test "accepts mixed case" do
      assert {:ok, {<<0xDE, 0xAD>>, nil}} = MCP.parse_hex_pattern("DE ad")
    end

    test "single-digit tokens are accepted" do
      assert {:ok, {<<0x01, 0x0F>>, nil}} = MCP.parse_hex_pattern("1 0F")
    end

    test "?? wildcard produces a zero byte with zero mask" do
      assert {:ok, {bytes, mask}} = MCP.parse_hex_pattern("48 ?? 05")
      assert bytes == <<0x48, 0x00, 0x05>>
      assert mask == <<0xFF, 0x00, 0xFF>>
    end

    test "single ? is also accepted as wildcard" do
      assert {:ok, {bytes, mask}} = MCP.parse_hex_pattern("48 ? 05")
      assert bytes == <<0x48, 0x00, 0x05>>
      assert mask == <<0xFF, 0x00, 0xFF>>
    end

    test "all-wildcard pattern produces nil mask" do
      # When every byte is 0xFF in the mask, mask is set to nil
      assert {:ok, {_bytes, nil}} = MCP.parse_hex_pattern("48 8B 05")
    end

    test "all-wildcard bytes produce all-zero mask" do
      # An all-wildcard pattern still returns an explicit mask (all 0x00).
      # The nil-mask optimization only applies when every byte is 0xFF (no wildcards).
      assert {:ok, {<<0, 0, 0>>, <<0, 0, 0>>}} = MCP.parse_hex_pattern("?? ?? ??")
    end

    test "leading/trailing whitespace is trimmed" do
      assert {:ok, {<<0x48>>, nil}} = MCP.parse_hex_pattern("  48  ")
    end

    test "invalid token returns structured error" do
      assert {:error, {:invalid_token, "ZZ"}} = MCP.parse_hex_pattern("48 ZZ 05")
    end

    test "token with more than 2 hex chars is invalid" do
      assert {:error, {:invalid_token, "FFF"}} = MCP.parse_hex_pattern("48 FFF 05")
    end

    test "empty string returns empty pattern with nil mask" do
      assert {:ok, {<<>>, nil}} = MCP.parse_hex_pattern("")
    end
  end

  describe "parse_addr/1" do
    test "parses 0x-prefixed address" do
      assert MCP.parse_addr("0x12345678") == 0x12345678
    end

    test "parses 0X-prefixed address" do
      assert MCP.parse_addr("0X1234") == 0x1234
    end

    test "parses bare hex string" do
      assert MCP.parse_addr("DEADBEEF") == 0xDEADBEEF
    end
  end

  describe "parse_name/2" do
    test "returns default for nil" do
      assert MCP.parse_name(nil, :default) == :default
    end

    test "returns default for empty string" do
      assert MCP.parse_name("", :default) == :default
    end

    test "converts binary name to atom" do
      result = MCP.parse_name("Krebs.Scanner", :default)
      assert is_atom(result)
      assert to_string(result) == "Krebs.Scanner"
    end
  end

  describe "collect_scan/2" do
    test "returns immediately when limit is 0" do
      assert MCP.collect_scan(0, []) == []
    end

    test "returns accumulated results when limit is 0" do
      assert MCP.collect_scan(0, [1, 2, 3]) == [3, 2, 1]
    end

    test "collects up to limit from mailbox" do
      # Seed the mailbox with a ScanMatch then done
      send(self(), %Krebs.ScanMatch{addr: 0x1000, value: <<1>>})
      send(self(), %Krebs.ScanMatch{addr: 0x2000, value: <<2>>})
      send(self(), {:done, 2})

      result = MCP.collect_scan(100, [])
      assert result == [0x1000, 0x2000]
    end

    test "stops at limit before done" do
      send(self(), %Krebs.ScanMatch{addr: 0x1000, value: <<1>>})
      send(self(), %Krebs.ScanMatch{addr: 0x2000, value: <<2>>})
      send(self(), %Krebs.ScanMatch{addr: 0x3000, value: <<3>>})
      send(self(), {:done, 3})

      result = MCP.collect_scan(2, [])
      assert result == [0x1000, 0x2000]
      # Drain the remaining messages so they don't leak
      receive do
        %Krebs.ScanMatch{} -> :ok
      end

      receive do
        {:done, _} -> :ok
      end
    end

    test "returns on scan error" do
      send(self(), %Krebs.ScanMatch{addr: 0x1000, value: <<1>>})
      send(self(), {:error, :some_error})

      result = MCP.collect_scan(100, [])
      assert result == [0x1000]
    end
  end
end
