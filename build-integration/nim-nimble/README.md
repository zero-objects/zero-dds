# Nimble `before build` hook

The idiomatic native Nimble build-step hook (NimScript `before build: ...`
in the `.nimble` file), not an out-of-band wrapper script. Mirrors
`zerodds-build` / CMake's `zerodds_idlc_generate()` for the Nim/Nimble
ecosystem. See `sample-consumer/zerodds_idlc_sample.nimble`.

## Usage

Add to your `.nimble` file:

```nim
before build:
  mkDir("src/gen")
  exec "zerodds-idlc generate idl/Robot.idl --nim -o src/gen"
```

## Validation status

Not build-validated in this pass: nimble/nim were not installed on the
local macOS dev machine, and the shared Linux validation host was
flagged mid-task as overloaded (further installs/builds there were
stopped rather than pushed through). The `.nimble` hook syntax and
sample were written and reviewed against the actual generated Nim API
surface (confirmed via a real `zerodds-idlc generate --nim` run against
the flat sample `.idl` — `type Pose* = object`, `marshalXCDR*`,
`unmarshalXCDRPose*`, `eLE`/`eBE`, all field names verbatim), but not
compiled. Central serial validation (once the host has room) should run
`nimble run` in `sample-consumer/`.

## Known IDL-surface gap this sample works around (not a build-tool gap)

`crates/idl-nim` has no `Definition::Module` arm (same family finding as
the Swift plugin's README) — the sample `.idl` here is a flat, unwrapped
`struct Pose`, the surface `idl-nim` actually supports today, not a
`module`-wrapped one.

## What a fuller pass would add

- **Staleness tracking.** Nimble's hook API has no manifest/mtime
  primitive to key off (unlike Mix.Task.Compiler's `manifests/0` or
  Gradle's declared `@InputFiles`/`@OutputDirectory`) — `before build`
  re-invokes `zerodds-idlc` on every build. For IDL sets large enough
  for this to matter, the hook would need to hand-roll an mtime check
  (compare `idl/Robot.idl`'s mtime against `src/gen/Robot.nim`'s) before
  calling `exec`.
- `#include` dependency tracking (same gap as the other integrations).
- Nimble package registry publication — consumed today via a local path,
  not `requires "zerodds_idlc"`.
