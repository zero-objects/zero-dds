# Julia — `deps/build.jl`

`Pkg.build()` runs `deps/build.jl`, which calls `zerodds-idlc generate
--lang julia --out src/gen idl/Robot.idl` when `src/gen/Robot.jl` is
missing or older than the IDL file. `Pkg.build` re-runs `build.jl` every
time; the mtime check is the script's own.

## Usage

```bash
julia --project=sample-consumer -e 'using Pkg; Pkg.build()'
julia --project=sample-consumer sample-consumer/src/RobotDemo.jl
```

`ZERODDS_IDLC` overrides the `zerodds-idlc` on `PATH`. The generated file
names `.ZeroddsWire`; `src/RobotDemo.jl` includes the runtime
`runtime/julia/zerodds_wire.jl` first. That runtime imports the standard
library `InteractiveUtils`, which `Project.toml` lists so the package
precompiles.

## Checked

macOS (Julia 1.12.6): `Pkg.build()` generates and precompiles `RobotDemo`;
both the script and `using RobotDemo; RobotDemo.main()` print

```
OK: Robot::Pose round-tripped through generated zd_enc_Robot_Pose/zd_dec_Robot_Pose (40 wire bytes), robot_id=r2d2
```

Running `deps/build.jl` again without a change prints `up to date,
skipping regeneration`.

## Not covered

- An `#include`d IDL file is not part of the mtime check.
- No shared package; each consumer carries its own `deps/build.jl`.
