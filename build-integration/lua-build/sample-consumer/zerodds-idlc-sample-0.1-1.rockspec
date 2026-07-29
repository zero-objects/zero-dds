-- SPDX-License-Identifier: Apache-2.0
-- Copyright 2026 ZeroDDS Contributors
--
-- LuaRocks' `build.type = "command"` is the native LuaRocks build-step
-- hook (LuaRocks is Lua's de facto package manager, the closest
-- equivalent Lua has to Cargo/npm/Nimble). Mirrors `zerodds-build` /
-- CMake's `zerodds_idlc_generate()` for the LuaRocks ecosystem. `luarocks
-- make` runs `build_command` before installing the rock.
package = "zerodds-idlc-sample"
version = "0.1-1"
source = {
   url = "file://./"
}
description = {
   summary = "Sample consumer proving the zerodds-idlc LuaRocks build-step integration",
   license = "Apache-2.0"
}
build = {
   type = "command",
   build_command = "zerodds-idlc generate idl/Robot.idl --lua -o gen",
   install = {
      lua = {
         ["gen.Robot"] = "gen/Robot.lua",
         ["app"] = "app.lua"
      }
   }
}
