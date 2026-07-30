# Ada — Makefile wrapper (no native GPR hook exists)

**LOUD FLAG:** GNAT project files (`.gpr`) have no native pre-build-command
hook. Cargo has `build.rs`, CMake has `add_custom_command`, SPM has
`BuildToolPlugin`, Gradle has its task graph, dub has
`preGenerateCommands`, Mix has `Mix.Task.Compiler` — the GPR project-file
language has no equivalent primitive to run an external command before
compilation and declare its output as a compile input. This is a real gap
in the GNAT Project Manager, not a corner this task cut: there is no
"real" native-hook form to fall back to short of upstream GNAT/gprbuild
feature work (out of ZeroDDS's control) or a GNAT Ada-language-server
IDE plugin (editor-specific, not a build-system integration).

The shipped answer is the documented, idiomatic-as-it-gets workaround:
a **`Makefile` wrapper** (`sample-consumer/Makefile`) that runs
`zerodds-idlc` then `gprbuild`, using `make`'s own mtime-based dependency
rule (`generated/robot.ads: idl/Robot.idl`) for incrementality —
`gprbuild` itself never sees the `.idl` file or knows codegen happened;
it only sees the resulting `.ads`/`.adb` already sitting in
`generated/`, wired in via the `.gpr`'s `Source_Dirs`.

## Usage

```
cd sample-consumer
make run
```

`make build` (or `run`, which depends on it) regenerates
`generated/robot.ads`/`robot.adb` from `idl/Robot.idl` only when the
`.idl` is newer, then runs `gprbuild -P robot_demo.gpr`.

## Validation status

Not build-validated in this pass: gprbuild/gnat were not available on
the local macOS dev machine, and the shared Linux host (which does have
`gprbuild`/`gnatmake`) was flagged mid-task as overloaded — further
builds there were stopped rather than pushed through. The `Makefile`
wrapper and sample were written and reviewed against the actual
generated Ada API surface (confirmed via a real `zerodds-idlc generate
--ada` run — `type Pose is record`, `function Marshal`/`Unmarshal`,
`Little`/`Big`, `Unbounded_String` for `string<N>`, field names
verbatim), but not compiled. Central serial validation (once the host
has room) should run `make run` in `sample-consumer/`.

## Known IDL-surface gap this sample works around (not a build-tool gap)

`crates/idl-ada` has no `Definition::Module` arm (same family finding
noted in the other READMEs in this directory) — the sample `.idl` is a
flat, unwrapped `struct Pose`.

## What a fuller pass would add

- `#include` dependency tracking (same gap as the other integrations).
- If a project already uses `alr` (Alire, Ada's newer package/build
  manager): Alire's `alr build` still shells out to `gprbuild` under the
  same GPR limitation above, so the wrapper would move to an Alire
  `post-fetch`/custom action hook instead of a bare Makefile — not
  implemented here (this sample targets plain `gprbuild`, the toolchain
  actually present on the validation host).
