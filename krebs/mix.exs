defmodule Krebs.MixProject do
  use Mix.Project

  def project do
    [
      app: :krebs,
      compilers: Mix.compilers(),
      deps: deps(),
      dialyzer: [ignore_warnings: ".dialyzer_ignore.exs"],
      elixir: "~> 1.18",
      rustler_crates: [libkrebs_nif: []],
      start_permanent: Mix.env() == :prod,
      version: "2.0.0",

      # Docs
      name: "Krebs",
      source_url: "https://github.com/tornarak/krebs_project/tree/master/krebs",
      homepage_url: "https://github.com/tornarak/krebs_project/tree/master/krebs",
      docs: [
        # The main page in the docs
        main: "Krebs",
        extras: ["README.md"]
      ]
    ]
  end

  def application do
    [
      env: env(),
      extra_applications: [:iex, :logger],
      mod: {Krebs.Application, []}
    ]
  end

  defp env do
    [
      init_opts: [
        read_size: 1 * 1024 * 1024
      ],
      scan_opts: [
        scan_type: :heap
      ]
    ]
  end

  # Run "mix help deps" to learn about dependencies.
  defp deps do
    [
      {:ex_doc, "~> 0.34", only: :dev, runtime: false},
      {:dialyxir, "~> 1.0", only: :dev, runtime: false},
      {:rustler, "~> 0.33"},
      {:bandit, "~> 1.5"},
      {:plug, "~> 1.16"},
      {:vancouver, "~> 0.3"},
      {:rekto, path: "../rekto"}
    ]
  end
end
