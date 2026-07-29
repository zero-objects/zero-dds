// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Round-trips a Pose through the *generated* marshalXCDR/UnmarshalXCDRPose
// — proof that dub's preGenerateCommands actually ran zerodds-idlc before
// this module compiled (source/gen/Robot.d does not exist in the source
// tree; `importPaths` in dub.json makes it resolve as `import Robot;`).
import std.stdio : writeln;
import Robot;

void main() {
    Pose pose;
    pose.robot_id = "r2d2";
    pose.x = 1.5;
    pose.y = -2.5;
    pose.theta = 0.75;

    ubyte[] wire = pose.marshalXCDR(Endian.LE);
    Pose decoded = UnmarshalXCDRPose(wire, Endian.LE);

    assert(decoded.robot_id == pose.robot_id);
    assert(decoded.x == pose.x);
    assert(decoded.y == pose.y);
    assert(decoded.theta == pose.theta);

    writeln("OK: Pose round-tripped through generated marshalXCDR/UnmarshalXCDRPose (",
            wire.length, " wire bytes), robot_id=", decoded.robot_id);
}
