// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Round-trips a Robot::Pose through the *generated* zd_enc_Robot_Pose/
// zd_dec_Robot_Pose — proof that the ZeroddsIdlcPlugin build command actually
// ran before this target compiled.

import ZdWire

let pose = Robot_Pose(robot_id: "r2d2", x: 1.5, y: -2.5, theta: 0.75)

let w = ZdWriter(zdEndianness, 4, ZdPlCdr1Framing.defaults(), false)
zd_enc_Robot_Pose(w, pose)
let wire = w.finish()

let r = ZdReader(wire, zdEndianness, 4, ZdPlCdr1Framing.defaults())
let decoded = zd_dec_Robot_Pose(r, ZdTryConstruct.discard)

guard r.readerStatus() == .ok, decoded.robot_id == pose.robot_id, decoded.x == pose.x, decoded.y == pose.y, decoded.theta == pose.theta else {
    fatalError("round-trip mismatch: \(decoded)")
}

print("OK: Robot::Pose round-tripped through generated zd_enc_Robot_Pose/zd_dec_Robot_Pose (\(wire.count) wire bytes), robot_id=\(decoded.robot_id)")
