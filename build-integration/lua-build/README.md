# Lua build step

Lua has no single de facto build system the way Cargo/npm/Cargo do, but
it does have a de facto *package manager*, LuaRocks, whose
`build.type = "command"` is the closest native equivalent to a build-step
hook (`luarocks make` runs `build_command` before installing). This
sample ships both:

- `zerodds-idlc-sample-0.1-1.rockspec` — the LuaRocks-native form.
- `build.lua` — a plain fallback for consumers without LuaRocks (run
  `lua build.lua` before `lua app.lua`), with its own mtime-based
  staleness check since there is no build graph to lean on outside
  LuaRocks.

Mirrors `zerodds-build` / CMake's `zerodds_idlc_generate()` for Lua.

## Usage (LuaRocks)

```
luarocks make
```

## Usage (plain Lua)

```
lua build.lua
lua app.lua
```

## Wire-format detail specific to this backend

`crates/idl-lua`'s generated file has no `return` statement — it defines
`marshal_<Type>`/`unmarshal_<Type>`/`read_<Type>` as **global** functions
as a side effect of `dofile`/`require`, while `LE`/`BE` (endianness
markers, the strings `"<"`/`">"`) are declared `local` to the generated
chunk and are therefore **not** visible to the caller. `app.lua` passes
the literal `"<"` for little-endian rather than a symbolic constant.

## Validated

`lua build.lua && lua app.lua` (Lua 5.5.0, locally-built `zerodds-idlc`
1.0.0-rc.6): generates `gen/Robot.lua`, round-trips a `Pose` through the
generated `marshal_Pose`/`unmarshal_Pose` (40 wire bytes). Re-running
`build.lua` unchanged prints "up to date, skipping regeneration" — the
mtime check works. `luarocks make` (the rockspec form) was not run —
LuaRocks itself was not installed on the validation machine; flagged,
not claimed.

## Known IDL-surface gap this sample works around (not a build-tool gap)

`crates/idl-lua` has no `Definition::Module` arm (same family finding
noted in the Swift/Nim/D/Julia READMEs) — the sample `.idl` is a flat,
unwrapped `struct Pose`.

## What a fuller pass would add

- `#include` dependency tracking (same gap as the other integrations).
- A real build-system integration if/when one of the growing Lua build
  tools (e.g. a `premake5` or `xmake` project) becomes the de facto
  choice — today's answer is genuinely "LuaRocks or nothing", which is
  why both a LuaRocks form and a plain fallback are shipped rather than
  picking one.
