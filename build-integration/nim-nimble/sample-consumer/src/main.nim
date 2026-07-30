# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# Round-trips a Pose through the *generated* marshalXCDR/unmarshalXCDRPose
# — proof that the `before build` nimble hook actually ran before this
# module compiled (src/gen/Robot.nim does not exist in the source tree).
import gen/Robot

let pose = Pose(robot_id: "r2d2", x: 1.5, y: -2.5, theta: 0.75)

let wire = pose.marshalXCDR(eLE)
let decoded = unmarshalXCDRPose(wire, eLE)

doAssert decoded.robot_id == pose.robot_id
doAssert decoded.x == pose.x
doAssert decoded.y == pose.y
doAssert decoded.theta == pose.theta

echo "OK: Pose round-tripped through generated marshalXCDR/unmarshalXCDRPose (", wire.len, " wire bytes), robot_id=", decoded.robot_id
