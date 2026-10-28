# D — dub `preGenerateCommands`

dub runs `preGenerateCommands` before every `dub build` and `dub run`
(`sample-consumer/dub.json`).

## Usage

```json
{
    "sourceFiles": ["../../../runtime/d/zdwire.d"],
    "importPaths": ["source", "source/gen", "../../../runtime/d"],
    "preGenerateCommands": [
        "mkdir -p source/gen && zerodds-idlc generate --lang d --out source/gen idl/Robot.idl"
    ]
}
```

The generated `source/gen/Robot.d` has no `module` statement, so it is
imported by its file name through `importPaths`: `import Robot;`. It
imports `zdwire`, the runtime in `runtime/d`, which the sample compiles in
through `sourceFiles`.

## Checked

the Linux test host (Linux x86_64, LDC 1.40.0, DUB 1.40.0):

```
dub run --compiler=ldc2
OK: Robot::Pose round-tripped through generated zd_enc_Robot_Pose/zd_dec_Robot_Pose (40 wire bytes), robot_id=r2d2
```

## Not covered

- dub has no staleness check for `preGenerateCommands`; `zerodds-idlc`
  runs on every build.
- An `#include`d IDL file is not tracked.
