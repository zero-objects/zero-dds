# Julia `deps/build.jl`

The idiomatic native Julia package build step. Mirrors `zerodds-build` /
CMake's `zerodds_idlc_generate()` for the Julia ecosystem. See
`sample-consumer/deps/build.jl`.

## Usage

```
julia --project=sample-consumer -e 'using Pkg; Pkg.build()'
julia --project=sample-consumer sample-consumer/src/RobotDemo.jl
```

`Pkg.build()` runs `deps/build.jl`, which regenerates `src/gen/Robot.jl`
from `idl/Robot.idl` only when the `.idl` is newer than the generated
file (`build.jl` does its own `mtime` comparison — `Pkg.build` itself
has no incremental concept, it always re-executes `build.jl`).

## Validation status

Not build-validated in this pass: Julia was not installed on the local
macOS dev machine, and the shared Linux validation host was flagged
mid-task as overloaded (further installs/builds there were stopped
rather than pushed through). `deps/build.jl` and the sample were written
and reviewed against the actual generated Julia API surface (confirmed
via a real `zerodds-idlc generate --julia` run — `struct Pose`,
`marshal_xcdr`, `unmarshal_xcdr_Pose`, `LE`/`BE`, field names verbatim),
but not run. Central serial validation (once the host has room) should
run `julia --project=sample-consumer -e 'using Pkg; Pkg.build()'` then
`julia --project=sample-consumer sample-consumer/src/RobotDemo.jl`.

## Known IDL-surface gap this sample works around (not a build-tool gap)

`crates/idl-julia` has no `Definition::Module` arm (same family finding
noted in the Swift/Nim/D READMEs) — the sample `.idl` is a flat,
unwrapped `struct Pose`.

## What a fuller pass would add

- **General Registry publication.** This sample consumes `zerodds-idlc`
  as a build step *inside its own* `deps/build.jl`; there is no separate
  "zerodds_idlc" Julia package to depend on (Julia's build-step
  convention is per-package, not a shared plugin registered elsewhere —
  unlike Gradle/SPM/Mix). A fuller pass would extract the
  generate-if-stale logic into a small shared `.jl` helper file other
  packages could `include()`.
- `#include` dependency tracking (same gap as the other integrations).
- Precompilation interaction: `Pkg.build()` must run *before* Julia
  precompiles `RobotDemo` (true today because `Pkg.build` and
  precompilation are already separate, ordered Pkg operations) — not
  wired into `Pkg.precompile()` directly.
