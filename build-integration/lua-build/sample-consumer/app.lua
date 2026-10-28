-- SPDX-License-Identifier: Apache-2.0
-- Copyright 2026 ZeroDDS Contributors
--
-- Round-trips a Robot::Pose through the *generated* codec -- proof that
-- build.lua (or the rockspec's build_command) ran zerodds-idlc before this
-- script ran (gen/Robot.lua is not in the source tree). The generated
-- chunk returns its codec table; the wire runtime is runtime/lua.
package.path = "../../../runtime/lua/?.lua;../../../runtime/lua/?/init.lua;gen/?.lua;"
    .. package.path

local ZD = require("Robot")
local zdw = require("zerodds_wire")

local pose = { robot_id = "r2d2", x = 1.5, y = -2.5, theta = 0.75 }

local w = zdw.writer_new({ endianness = "le", max_align = 4 })
ZD.zd_enc_Robot_Pose(w, pose)
local wire = zdw.finish(w)

local r = zdw.reader_new(wire, "le", 4)
local decoded = ZD.zd_dec_Robot_Pose(r, "discard")

assert(decoded.robot_id == pose.robot_id)
assert(decoded.x == pose.x)
assert(decoded.y == pose.y)
assert(decoded.theta == pose.theta)

print(string.format(
    "OK: Robot::Pose round-tripped through generated zd_enc_Robot_Pose/zd_dec_Robot_Pose (%d wire bytes), robot_id=%s",
    #wire, decoded.robot_id))
