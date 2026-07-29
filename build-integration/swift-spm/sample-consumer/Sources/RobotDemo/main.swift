// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Round-trips a Robot.Pose through the *generated* marshalXCDR/
// unmarshalXCDR — proof that the ZeroddsIdlcPlugin build command actually
// ran before this target compiled.

let pose = Pose(robot_id: "r2d2", x: 1.5, y: -2.5, theta: 0.75)

let wire = pose.marshalXCDR(.little)
let decoded = Pose.unmarshalXCDR(wire, .little)

guard decoded.robot_id == pose.robot_id, decoded.x == pose.x, decoded.y == pose.y, decoded.theta == pose.theta else {
    fatalError("round-trip mismatch: \(decoded)")
}

print("OK: Pose round-tripped through generated marshalXCDR/unmarshalXCDR (\(wire.count) wire bytes), robot_id=\(decoded.robot_id)")
