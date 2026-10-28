# Ada — Makefile wrapper around gprbuild

A GNAT project file (`.gpr`) has no step that runs an external command
before compilation. `sample-consumer/Makefile` therefore runs
`zerodds-idlc` and then `gprbuild`; `make`'s own mtime rule
(`generated/zd_types.ads: idl/Robot.idl`) decides whether to generate
again. `gprbuild` sees only the generated `.ads`/`.adb` through the
`.gpr`'s `Source_Dirs`.

## Usage

```bash
cd sample-consumer
make run        # ZERODDS_IDLC=/path/to/zerodds-idlc to override PATH
```

The Makefile calls `zerodds-idlc generate --lang ada --out generated
idl/Robot.idl`. Ada writes the fixed unit `Zd_Types`
(`zd_types.ads`/`zd_types.adb`) for every input, so one IDL file per
`--out` directory. The generated unit needs `Zd_Wire` from
`runtime/ada/src`, which `robot_demo.gpr` lists as a source directory.

## Checked

the Linux test host (Linux x86_64, GPRBUILD Pro 18.0w):

```
./obj/main
OK: Robot::Pose round-tripped through generated zd_enc_Robot_Pose/zd_dec_Robot_Pose ( 40 wire bytes), robot_id=r2d2
```

A second `make generate` without a change to the IDL file prints
`make: Nothing to be done for 'generate'.`

## Not covered

- An `#include`d IDL file is not a prerequisite of the rule.
- Alire (`alr build`) is not covered; it calls `gprbuild` with the same
  limitation.
