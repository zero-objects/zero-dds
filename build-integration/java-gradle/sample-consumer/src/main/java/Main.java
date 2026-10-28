// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Round-trips a Robot.Pose through the *generated* (not hand-written)
// type support — proof that the generateIdl task
// ran before compileJava. The generated classes are in the unnamed package,
// so this class is too.

public final class Main {
    public static void main(String[] args) {
        ZdTypes.Robot.Pose pose = new ZdTypes.Robot.Pose();
        pose.robot_id = "r2d2";
        pose.x = 1.5;
        pose.y = -2.5;
        pose.theta = 0.75;

        ZdTypeSupports.Support_Robot_Pose support = ZdTypeSupports.Support_Robot_Pose.INSTANCE;
        byte[] wire = support.encode(pose);
        ZdTypes.Robot.Pose decoded = support.decode(wire);

        if (!decoded.robot_id.equals(pose.robot_id)
                || decoded.x != pose.x
                || decoded.y != pose.y
                || decoded.theta != pose.theta) {
            throw new AssertionError("round-trip mismatch: " + decoded);
        }

        System.out.println(
                "OK: Robot.Pose round-tripped through generated Support_Robot_Pose ("
                        + wire.length + " wire bytes), robot_id=" + decoded.robot_id);
    }
}
