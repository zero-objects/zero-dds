# Lua — LuaRocks `build.type = "command"` and a plain `build.lua`

- `zerodds-idlc-sample-0.1-1.rockspec`: `luarocks make` runs
  `zerodds-idlc generate --lang lua --out gen idl/Robot.idl` and installs
  `gen/Robot.lua` as the module `Robot`.
- `build.lua`: the same command for a bare `lua`, run only when
  `gen/Robot.lua` is missing or older than the IDL file.

## Usage

```bash
cd sample-consumer
luarocks make            # or: lua build.lua
lua app.lua
```

The generated chunk returns its codec table (`ZD`, with
`zd_enc_Robot_Pose`/`zd_dec_Robot_Pose`) and requires `zerodds_wire`, the
runtime in `runtime/lua`. `app.lua` puts that directory and `gen/` on
`package.path`.

## Checked

macOS (Lua 5.5.0, LuaRocks 3.13.0):

```
lua build.lua && lua app.lua
OK: Robot::Pose round-tripped through generated zd_enc_Robot_Pose/zd_dec_Robot_Pose (40 wire bytes), robot_id=r2d2
```

A second `lua build.lua` prints `up to date, skipping regeneration`.
`luarocks make --tree <dir>` ran the build command and installed
`Robot.lua` and `app.lua` into that tree.

## Not covered

- An `#include`d IDL file is not part of `build.lua`'s mtime check.
- The rockspec runs `zerodds-idlc` on every `luarocks make`.
