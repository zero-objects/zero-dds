# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
defmodule ZeroddsWire.MixProject do
  use Mix.Project

  def project do
    [
      app: :zerodds_wire,
      version: "1.0.0-rc.7",
      elixir: "~> 1.14",
      start_permanent: false,
      deps: [],
      # The same rule the generated column is held to: a warning here is
      # a defect. Elixir 1.20's type checker reports a comparison across
      # disjoint types, which is how an assertion that can only ever be
      # true is found — one was, in `float128_test.exs`. `mix test`
      # compiles the test files with its own `--warnings-as-errors`, so
      # the suite is run with that flag.
      elixirc_options: [warnings_as_errors: true],
      description:
        "DDS-XTypes 1.3 §7.4 read from the clause, in Elixir — a further " <>
          "implementation of the wire, not a binding to any other."
    ]
  end

  def application do
    # A library of pure functions: nothing to supervise, nothing to
    # start.
    [extra_applications: []]
  end
end
