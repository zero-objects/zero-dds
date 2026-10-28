// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Round-trips a Robot::Pose through the *generated* zd_enc_Robot_Pose /
// zd_dec_Robot_Pose — proof that dub's preGenerateCommands ran
// zerodds-idlc before this module compiled (source/gen/Robot.d is not in
// the source tree; `importPaths` in dub.json makes it resolve as
// `import Robot;`). zdwire is the runtime in runtime/d.
import std.stdio : writeln;
import zdwire;
import Robot;

void main()
{
    auto framing = PlCdr1Framing("sentinel-must-understand", 0x3F00, false, false, true, true);

    Robot_Pose pose;
    pose.robot_id = "r2d2";
    pose.x = 1.5;
    pose.y = -2.5;
    pose.theta = 0.75;

    auto w = new Writer(ZdEndianness, 4, framing, false);
    zd_enc_Robot_Pose(w, pose);
    ubyte[] wire = w.Finish();

    auto r = new Reader(wire, ZdEndianness, 4, framing.pidFlags, framing.highestShortID);
    Robot_Pose decoded = zd_dec_Robot_Pose(r, TryConstruct.discard);

    assert(r.ReaderStatus() == Status.ok);
    assert(decoded.robot_id == pose.robot_id);
    assert(decoded.x == pose.x);
    assert(decoded.y == pose.y);
    assert(decoded.theta == pose.theta);

    writeln("OK: Robot::Pose round-tripped through generated zd_enc_Robot_Pose/zd_dec_Robot_Pose (",
            wire.length, " wire bytes), robot_id=", decoded.robot_id);
}
