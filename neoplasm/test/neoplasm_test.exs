defmodule NeoplasmTest do
  use ExUnit.Case
  doctest Neoplasm

  test "greets the world" do
    assert Neoplasm.hello() == :world
  end
end
