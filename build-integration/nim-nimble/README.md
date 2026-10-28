# Nim — Nimble `before build` hook

`sample-consumer/zerodds_idlc_sample.nimble` runs `zerodds-idlc` in a
NimScript `before build:` block:

```nim
before build:
  mkDir("src/gen")
  exec "zerodds-idlc generate --lang nim --out src/gen idl/Robot.idl"
```

The generated `src/gen/Robot.nim` imports `zdwire`, the runtime in
`runtime/nim`; `config.nims` puts that directory on the search path.

## Checked

macOS (Nim 2.2.10):

```bash
cd sample-consumer
nimble -y build && ./main
# OK: Robot::Pose round-tripped through generated zd_enc_Robot_Pose/zd_dec_Robot_Pose (40 wire bytes), robot_id=r2d2
```

## Not covered

- The hook has no staleness check; `zerodds-idlc` runs on every build.
- An `#include`d IDL file is not tracked.
- No Nimble package; the hook is written into the consumer's `.nimble`.
