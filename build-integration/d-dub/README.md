# dub `preGenerateCommands`

The idiomatic native dub build-step hook, run before every build/compile
(`dub build`, `dub run`). Mirrors `zerodds-build` / CMake's
`zerodds_idlc_generate()` for the D/dub ecosystem. See
`sample-consumer/dub.json`.

## Usage

```json
{
    "preGenerateCommands": [
        "mkdir -p source/gen && zerodds-idlc generate idl/Robot.idl --d -o source/gen"
    ],
    "importPaths": ["source", "source/gen"]
}
```

`zerodds-idlc` doesn't emit a D `module` statement, so the generated
`source/gen/Robot.d` is imported by its filename relative to an
`importPaths` entry: `import Robot;` (with `source/gen` on the import
path), not `import gen.Robot;`.

## Validation status

Not build-validated in this pass: no D toolchain (dub/ldc/dmd) was
available on the local macOS dev machine, and the shared Linux
validation host was flagged mid-task as overloaded (further
installs/builds there were stopped rather than pushed through). The
`dub.json` and sample were written and reviewed against the actual
generated D API surface (confirmed via a real `zerodds-idlc generate --d`
run — `struct Pose { ... }`, `marshalXCDR`, `UnmarshalXCDRPose`,
`Endian.LE`/`BE`, no `module` statement so `import Robot;` via
`importPaths`), but not compiled. Central serial validation (once the
host has room) should run `dub run` in `sample-consumer/`.

## Known IDL-surface gap this sample works around (not a build-tool gap)

`crates/idl-d` has no `Definition::Module` arm (same family finding noted
in the Swift/Nim READMEs) — the sample `.idl` is a flat, unwrapped
`struct Pose`.

## What a fuller pass would add

- **Staleness tracking.** dub's `preGenerateCommands` has no built-in
  skip-if-unchanged primitive — it shells out unconditionally on every
  `dub build`. A fuller pass would wrap the command in a small
  `Makefile`-style mtime check, or move the logic into a dub *plugin*
  (dub supports pre-build "commands" but not first-class build-graph
  nodes the way SPM/Gradle/MSBuild do).
- `#include` dependency tracking (same gap as the other integrations).
- dub registry (code.dlang.org) publication — this sample consumes the
  hook inline in its own `dub.json`, there being no separate "plugin
  package" to depend on for a shell-command hook.
