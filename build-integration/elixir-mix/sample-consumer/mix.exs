# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# `mix compile` runs `:zerodds_idl` (the Mix.Task.Compiler in
# ../lib/mix/tasks/compile/zerodds_idl.ex, pulled in as a path dep) before
# `:elixir`, regenerating `lib/gen/Robot.ex` from `idl/Robot.idl` whenever
# the .idl changes, then compiles the generated module normally.
defmodule RobotDemo.MixProject do
  use Mix.Project

  def project do
    [
      app: :robot_demo,
      version: "0.1.0",
      elixir: "~> 1.15",
      compilers: [:zerodds_idl] ++ Mix.compilers(),
      zerodds_idl: [
        idl_files: ["idl/Robot.idl"],
        backend: "elixir",
        output_dir: "lib/gen"
      ],
      deps: deps()
    ]
  end

  defp deps do
    [
      {:zerodds_idlc, path: "../"}
    ]
  end
end
