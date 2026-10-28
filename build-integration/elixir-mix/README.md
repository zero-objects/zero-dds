# `:zerodds_idl` Mix compiler

A `Mix.Task.Compiler` (`lib/mix/tasks/compile/zerodds_idl.ex`). It runs
`zerodds-idlc generate --lang <backend> --out <output_dir> [-I …] <file>`
once per stale IDL file.

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
    deps: [
      {:zerodds_idlc, path: "path/to/build-integration/elixir-mix"},
      # the wire runtime the generated ZdCodec calls
      {:zerodds_wire, path: "path/to/runtime/elixir"}
    ]
  ]
end
```

`mix compile` now regenerates `lib/gen/Robot.ex` whenever `idl/Robot.idl`
is newer than the recorded manifest entry
(`_build/<env>/lib/<app>/.mix/compile.zerodds_idl`, `manifests/0`).
`mix clean` removes it through the `clean/0` callback.

## Checked

macOS (Elixir 1.20.2, Erlang/OTP 29):

```bash
cd sample-consumer
mix compile
mix run -e "RobotDemo.main()"
# OK: Robot::Pose round-tripped through generated ZdCodec (40 wire bytes), robot_id=r2d2
```

The mtime of `lib/gen/Robot.ex` stays the same over a second `mix compile`
and changes after `touch idl/Robot.idl`.

## Not covered

- An `#include`d IDL file does not feed the staleness check.
- No Hex package; the compiler is a `path:` dependency.
