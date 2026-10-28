// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Round-trips a Robot.Pose through the *generated* PoseTypeSupport —
// proof that ZeroddsIdlcGenerate (ZeroDDS.Idlc.targets) actually ran
// before CoreCompile.
using Robot;

var pose = new Pose
{
    Robot_id = "r2d2",
    X = 1.5,
    Y = -2.5,
    Theta = 0.75,
};

byte[] wire = PoseTypeSupport.Instance.Encode(pose);
Pose decoded = PoseTypeSupport.Instance.Decode(wire);

if (decoded.Robot_id != pose.Robot_id || decoded.X != pose.X || decoded.Y != pose.Y || decoded.Theta != pose.Theta)
{
    throw new InvalidOperationException($"round-trip mismatch: {decoded}");
}

Console.WriteLine($"OK: Robot.Pose round-tripped through generated PoseTypeSupport ({wire.Length} wire bytes), Robot_id={decoded.Robot_id}");
