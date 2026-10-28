# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
#
# Round-trips a Robot::Pose through the *generated* zd_enc_Robot_Pose/
# zd_dec_Robot_Pose — proof that the `before build` nimble hook ran before
# this module compiled (src/gen/Robot.nim is not in the source tree). The
# wire runtime `zdwire` is runtime/nim, on the search path via config.nims.
import zdwire
import gen/Robot

let framing = zdwire.PlCdr1Framing(pidFlags: "sentinel-must-understand",
  highestShortId: 16128, padDeclaredLength: true,
  padDeclaredOptionalLength: true, memberMustUnderstand: true,
  padAfterValue: true)

let pose = Robot_Pose(robot_id: "r2d2", x: 1.5, y: -2.5, theta: 0.75)

var w = zdwire.newWriter(zdEndianness, 4, framing, false)
zd_enc_Robot_Pose(w, pose)
let wire = w.finish()

var r = zdwire.newReader(wire, zdEndianness, 4, framing)
let decoded = zd_dec_Robot_Pose(r, zdwire.TryConstructDiscard)

doAssert r.readerStatus() == zdwire.StatusOk
doAssert decoded.robot_id == pose.robot_id
doAssert decoded.x == pose.x
doAssert decoded.y == pose.y
doAssert decoded.theta == pose.theta

echo "OK: Robot::Pose round-tripped through generated zd_enc_Robot_Pose/zd_dec_Robot_Pose (", wire.len, " wire bytes), robot_id=", decoded.robot_id
