// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Round-trips a Robot.Pose through the *generated* (not hand-written)
// PoseTypeSupport.encode/decode — proof that `generateIdl` actually ran
// before compileJava and that its output is the real emitter, not a stub.
package com.example;

import robot.Pose;
import robot.PoseTypeSupport;

public final class Main {
    public static void main(String[] args) {
        Pose pose = new Pose();
        pose.setRobot_id("r2d2");
        pose.setX(1.5);
        pose.setY(-2.5);
        pose.setTheta(0.75);

        byte[] wire = PoseTypeSupport.INSTANCE.encode(pose);
        Pose decoded = PoseTypeSupport.INSTANCE.decode(wire);

        if (!decoded.getRobot_id().equals(pose.getRobot_id())
                || decoded.getX() != pose.getX()
                || decoded.getY() != pose.getY()
                || decoded.getTheta() != pose.getTheta()) {
            throw new AssertionError("round-trip mismatch: " + decoded);
        }

        System.out.println(
                "OK: Robot.Pose round-tripped through generated PoseTypeSupport ("
                        + wire.length + " wire bytes), robot_id=" + decoded.getRobot_id());
    }
}
