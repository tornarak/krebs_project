defmodule Neoplasm.MixProject do
  use Mix.Project

  def project do
    [
      app: :neoplasm,
      version: "0.1.0",
      elixir: "~> 1.19",
      compilers: Mix.compilers(),
      rustler_crates: [neoplasm_nif: []],
      start_permanent: Mix.env() == :prod,
      deps: deps()
    ]
  end

  def application do
    [
      extra_applications: [:logger],
      mod: {Neoplasm.Application, []}
    ]
  end

  defp deps do
    [
      {:rustler, "~> 0.33"},
      {:krebs, path: "../krebs"},
      {:rekto, path: "../rekto"}
    ]
  end
end
