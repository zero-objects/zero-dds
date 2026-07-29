#!/usr/bin/env lua
-- SPDX-License-Identifier: Apache-2.0
-- Copyright 2026 ZeroDDS Contributors
--
-- Plain build script for consumers not using LuaRocks (the rockspec form
-- -- zerodds-idlc-sample-0.1-1.rockspec, `build.type = "command"` -- is
-- the native LuaRocks mechanism; this is the fallback for a bare `lua`
-- install). Run before `lua app.lua`. Only regenerates when the .idl is
-- newer than the generated output (no build-system staleness primitive
-- to lean on outside LuaRocks/a real build graph).

local function mtime(path)
    local f = io.popen('stat -f %m "' .. path .. '" 2>/dev/null || stat -c %Y "' .. path .. '" 2>/dev/null')
    local out = f:read("*a")
    f:close()
    local n = tonumber(out)
    return n
end

local function file_exists(path)
    local f = io.open(path, "r")
    if f then f:close() return true end
    return false
end

local idl = "idl/Robot.idl"
local out_dir = "gen"
local out_file = out_dir .. "/Robot.lua"
local executable = os.getenv("ZERODDS_IDLC") or "zerodds-idlc"

os.execute('mkdir -p "' .. out_dir .. '"')

local needs_regen = not file_exists(out_file)
if not needs_regen then
    local idl_mtime = mtime(idl)
    local out_mtime = mtime(out_file)
    needs_regen = (idl_mtime == nil) or (out_mtime == nil) or (idl_mtime > out_mtime)
end

if needs_regen then
    print("zerodds-idlc: generating " .. out_file .. " from " .. idl)
    local cmd = string.format('%s generate "%s" --lua -o "%s"', executable, idl, out_dir)
    local ok = os.execute(cmd)
    if not ok then
        io.stderr:write("zerodds-idlc failed for " .. idl .. "\n")
        os.exit(1)
    end
else
    print("zerodds-idlc: " .. out_file .. " up to date, skipping regeneration")
end
