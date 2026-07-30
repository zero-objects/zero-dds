# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# A real `Mix.Task.Compiler` — added to a project's `mix.exs` via
# `compilers: [:zerodds_idl] ++ Mix.compilers()` — mirroring the Rust
# `zerodds-build` build.rs helper (crates/zerodds-build) and the CMake
# `zerodds_idlc_generate()` function (cmake/zerodds_idlc_generate.cmake)
# for the Elixir/Mix ecosystem.
#
# Unlike a one-off `mix.exs` `System.cmd/3` call in an `aliases: [compile:
# ...]` hook (the thin form), a `Mix.Task.Compiler` participates in Mix's
# own manifest/staleness tracking (`manifests/0` + `Mix.Utils.stale?/2`,
# the same primitive `mix compile.erlang`/`mix compile.elixir` use), so
# `mix compile` only re-invokes `zerodds-idlc` when a `.idl` file is newer
# than its last-generated `.ex`, and `mix compile --force` /
# `mix clean` interoperate correctly with the rest of the build graph.
defmodule Mix.Tasks.Compile.ZeroddsIdl do
  use Mix.Task.Compiler

  @recursive true
  @manifest "compile.zerodds_idl"

  @impl Mix.Task.Compiler
  def run(_args) do
    config = Mix.Project.config()
    zerodds_idl = Keyword.get(config, :zerodds_idl, [])

    idl_files = Keyword.get(zerodds_idl, :idl_files, Path.wildcard("idl/**/*.idl"))
    backend = Keyword.get(zerodds_idl, :backend, "elixir")
    output_dir = Keyword.get(zerodds_idl, :output_dir, "lib/gen")
    include_dirs = Keyword.get(zerodds_idl, :include_dirs, [])
    executable = Keyword.get(zerodds_idl, :executable, "zerodds-idlc")

    manifest_path = manifest_path()
    stale = stale_idl_files(idl_files, manifest_path)

    if stale == [] do
      {:noop, []}
    else
      File.mkdir_p!(output_dir)
      diagnostics = Enum.flat_map(stale, &generate_one(&1, backend, output_dir, include_dirs, executable))

      if diagnostics == [] do
        write_manifest(manifest_path, idl_files)
        {:ok, []}
      else
        {:error, diagnostics}
      end
    end
  end

  @impl Mix.Task.Compiler
  def manifests, do: [manifest_path()]

  @impl Mix.Task.Compiler
  def clean do
    config = Mix.Project.config()
    zerodds_idl = Keyword.get(config, :zerodds_idl, [])
    output_dir = Keyword.get(zerodds_idl, :output_dir, "lib/gen")
    File.rm_rf(output_dir)
    File.rm(manifest_path())
    :ok
  end

  defp manifest_path do
    Path.join(Mix.Project.manifest_path(), @manifest)
  end

  # Re-runs zerodds-idlc when the .idl file is newer than the manifest's
  # recorded mtime for it, or when it is new/removed — the same staleness
  # test `Mix.Utils.stale?/2` performs internally for :elixir/:erlang.
  defp stale_idl_files(idl_files, manifest_path) do
    previous =
      case File.read(manifest_path) do
        {:ok, contents} -> :erlang.binary_to_term(contents)
        {:error, _} -> %{}
      end

    Enum.filter(idl_files, fn idl ->
      case File.stat(idl, time: :posix) do
        {:ok, %File.Stat{mtime: mtime}} -> Map.get(previous, idl) != mtime
        {:error, _} -> true
      end
    end)
  end

  defp write_manifest(manifest_path, idl_files) do
    entries =
      for idl <- idl_files, into: %{} do
        {:ok, %File.Stat{mtime: mtime}} = File.stat(idl, time: :posix)
        {idl, mtime}
      end

    File.mkdir_p!(Path.dirname(manifest_path))
    File.write!(manifest_path, :erlang.term_to_binary(entries))
  end

  defp generate_one(idl, backend, output_dir, include_dirs, executable) do
    include_args = Enum.flat_map(include_dirs, &["-I", &1])
    args = ["generate", idl, "--#{backend}", "-o", output_dir] ++ include_args

    case System.cmd(executable, args, stderr_to_stdout: true) do
      {_output, 0} ->
        []

      {output, exit_code} ->
        [
          %Mix.Task.Compiler.Diagnostic{
            file: idl,
            source: idl,
            severity: :error,
            message: "zerodds-idlc exited #{exit_code}:\n#{output}",
            position: nil,
            compiler_name: "zerodds_idl"
          }
        ]
    end
  end
end
