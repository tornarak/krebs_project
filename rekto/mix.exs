defmodule Rekto.MixProject do
  use Mix.Project

  def project do
    [
      app: :rekto,
      version: "1.1.0",
      elixir: "~> 1.18",
      start_permanent: Mix.env() == :prod,
      deps: deps(),
      dialyzer: [ignore_warnings: ".dialyzer_ignore.exs"],

      # Docs
      name: "Rekto",
      source_url: "https://github.com/tornarak/krebs_project/tree/master/rekto",
      homepage_url: "https://github.com/tornarak/krebs_project/tree/master/rekto",
      docs: [
        # The main page in the docs
        main: "readme",
        extras: ["README.md"],
        groups_for_modules: [
          "Data Specification": [
            Rekto.Schema,
            Rekto.Schema.Constraints,
            Rekto.Schema.Helpers,
            Rekto.Schema.Metadata,
            Rekto.Void
          ],
          "Data Retrieval": [
            Rekto.Repo,
            Rekto.Query,
            Rekto.Serialization,
            Rekto.Association.Error,
            Rekto.Association.NotLoaded,
            Rekto.Association.Primitive
          ],
          "Example Data": [
            Rekto.VCPP.String,
            Rekto.VCPP.String.Short,
            Rekto.VCPP.String.Long
          ]
        ]
      ]
    ]
  end

  # Run "mix help compile.app" to learn about applications.
  def application do
    [
      env: [],
      extra_applications: [:logger],
      mod: {Rekto.Application, []}
    ]
  end

  defp deps do
    [
      {:ex_doc, "~> 0.34", only: :dev, runtime: false},
      {:dialyxir, "~> 1.0", only: :dev, runtime: false}
    ]
  end
end
