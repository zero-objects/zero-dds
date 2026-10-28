-- SPDX-License-Identifier: Apache-2.0
-- Copyright 2026 ZeroDDS Contributors
--
-- `zerodds_wire` — the Lua wire runtime.
--
-- The runtime a generated Lua codec calls: the writer, the reader, the
-- alignment rules and the framing forms, implemented from DDS-XTypes
-- 1.3 §7.4 in plain Lua 5.4. It is a counterpart of `crates/wire`, not
-- a binding to it — a Lua probe that reached the Rust core through FFI
-- would be byte-identical by construction and would prove nothing about
-- whether the two read the specification the same way. No C module and
-- no dependency: `string.pack` and 64-bit integers are the language's
-- own, and the binary128 arithmetic is `float128.lua`'s.
--
-- This package makes no decision about a type. Which members are
-- written, in what order, with which header and which length code is
-- the layout plan's answer and arrives here as arguments.
--
-- One flat table, because that is the shape the generated code calls:
-- `zdw.write_u32(w, x)`, `zdw.read_pid_as(r, "member")`,
-- `zdw.no_dheader()`. The writer and the reader are plain tables the
-- constructors below hand back; nothing here has a metatable.
--
-- Spec: DDS-XTypes 1.3 §7.4, DDSI-RTPS 2.5 §10.5.

local writer = require("zerodds_wire.writer")
local reader = require("zerodds_wire.reader")
local typesupport = require("zerodds_wire.typesupport")
local float128 = require("zerodds_wire.float128")

local M = {}

for name, value in pairs(writer) do
  M[name] = value
end
for name, value in pairs(reader) do
  -- The two halves share three names on purpose — `align_to` is the
  -- same rule on both sides — and the reader's is the one a generated
  -- decoder reaches, so it wins where they collide. Every writer
  -- operation keeps its own name: the surfaces are disjoint apart from
  -- the alignment helper.
  M[name] = value
end
for name, value in pairs(typesupport) do
  M[name] = value
end

--- The binary128 conversions, for a caller that needs the pattern
--- rather than the sixteen bytes in a stream.
M.float128 = float128

M.VERSION = "1.0.0-rc.7"

return M
