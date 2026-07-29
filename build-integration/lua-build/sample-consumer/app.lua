-- SPDX-License-Identifier: Apache-2.0
-- Copyright 2026 ZeroDDS Contributors
--
-- Round-trips a Pose through the *generated* marshal_Pose/unmarshal_Pose
-- -- proof that build.lua (or the rockspec's build_command) actually ran
-- zerodds-idlc before this script ran (gen/Robot.lua does not exist in
-- the source tree). `dofile` runs the generated chunk's top-level code,
-- which defines marshal_Pose/unmarshal_Pose as globals (the generated
-- file has no `return`, so `require` would also work but only via
-- side-effecting globals -- `dofile` makes that explicit).
dofile("gen/Robot.lua")

local pose = { robot_id = "r2d2", x = 1.5, y = -2.5, theta = 0.75 }

-- LE ("<") -- zerodds-idlc's Lua backend keeps LE/BE `local` to the
-- generated chunk (not exported), so callers pass the literal wire-format
-- string directly.
local wire = marshal_Pose(pose, "<")
local decoded = unmarshal_Pose(wire, "<")

assert(decoded.robot_id == pose.robot_id)
assert(decoded.x == pose.x)
assert(decoded.y == pose.y)
assert(decoded.theta == pose.theta)

print(string.format(
    "OK: Pose round-tripped through generated marshal_Pose/unmarshal_Pose (%d wire bytes), robot_id=%s",
    #wire, decoded.robot_id))
