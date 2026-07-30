# `:zerodds_idl` Mix compiler

A real `Mix.Task.Compiler` (`lib/mix/tasks/compile/zerodds_idl.ex`), not a
`mix.exs` `aliases:` shell-out. Mirrors `zerodds-build` (Rust,
`crates/zerodds-build`) and the CMake `zerodds_idlc_generate()` function
for the Elixir/Mix ecosystem.

## Usage

```elixir
# mix.exs
def project do
  [
    # ...
    compilers: [:zerodds_idl] ++ Mix.compilers(),
    zerodds_idl: [
      idl_files: ["idl/Robot.idl"],   # default: Path.wildcard("idl/**/*.idl")
      backend: "elixir",               # default
      output_dir: "lib/gen",           # default
      # include_dirs: ["idl/common"],  # -I search dirs
      # executable: "/opt/zerodds/bin/zerodds-idlc"
    ],
    deps: [{:zerodds_idlc, path: "path/to/build-integration/elixir-mix"}]
  ]
end
```

`mix compile` now regenerates `lib/gen/Robot.ex` whenever `idl/Robot.idl`
is newer than the recorded manifest entry — the manifest
(`_build/<env>/lib/<app>/.mix/compile.zerodds_idl`) is the same
staleness-tracking primitive Mix's own `:elixir`/`:erlang` compilers use
(`manifests/0` + a persisted term). `mix compile --force` and `mix clean`
both interoperate correctly (the latter via the `clean/0` callback).

## Validation status

Not build-validated in this pass: `mix`/Elixir generation for the sample
was checked (`zerodds-idlc generate --elixir` against the flat sample
`.idl` — confirmed `defmodule Robot.Pose`, `defstruct [:robot_id, :x,
:y, :theta]`, `marshal_xcdr/2`, `unmarshal/2`), but the
`Mix.Task.Compiler` wiring itself (`mix compile` end to end) was not
run — the shared Linux host (the only one with `elixir`/`mix`
available) was flagged mid-task as overloaded, and further work there
was stopped rather than pushed through. Central serial validation (once
the host has room) should run `mix run -e 'RobotDemo.main()'` in
`sample-consumer/` with `deps/zerodds_idlc` path-resolved.

## What a fuller pass would add

- **`#include` dependency tracking** (same gap as the Gradle plugin): only
  the top-level `.idl` files feed the staleness check, not files they
  `#include`. Fix: parse `zerodds-idlc print-deps` output into the
  manifest instead of just the top-level file list.
- **Hex package** — consumed today via a `path:` dependency (this whole
  directory *is* the Mix project, `app: :zerodds_idlc`); a published
  package would additionally need a `mix.lock`-friendly version and a
  `Hex.pm` release.
- **Parallel generation** — `run/1` invokes `zerodds-idlc` once per stale
  file sequentially; for large `.idl` sets `Task.async_stream/3` would
  parallelize it the way `mix compile.elixir` parallelizes module
  compilation.
