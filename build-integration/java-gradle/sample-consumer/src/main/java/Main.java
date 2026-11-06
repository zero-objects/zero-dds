// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Writes a Robot.Pose through the *generated* (not hand-written) type support
// and holds the octets to DDS-XTypes 1.3 §7.4 (XCDR2, little-endian): the proof
// that the generateIdl task ran before compileJava and that its output encodes
// as the clauses say. The generated classes are in the unnamed package, so this
// class is too.

import java.util.Arrays;

public final class Main {
    private static byte[] hex(String text) {
        String digits = text.replace(" ", "");
        byte[] out = new byte[digits.length() / 2];
        for (int i = 0; i < out.length; i++) {
            out[i] = (byte) Integer.parseInt(digits.substring(2 * i, 2 * i + 2), 16);
        }
        return out;
    }

    public static void main(String[] args) {
        ZdTypes.Robot.Pose pose = new ZdTypes.Robot.Pose();
        pose.robot_id = "r2d2";
        pose.x = 1.5;
        pose.y = -2.5;
        pose.theta = 0.75;

        // @appendable: the DHEADER 36; robot_id as length 5, "r2d2" and NUL, three octets
        // of padding; x, y and theta, each a float64 aligned to 4 (§7.4.1.1.1).
        byte[] body = hex("24 00 00 00 05 00 00 00 72 32 64 32 00 00 00 00"
                + " 00 00 00 00 00 00 F8 3F 00 00 00 00 00 00 04 C0 00 00 00 00 00 00 E8 3F");
        ZdTypeSupports.Support_Robot_Pose support = ZdTypeSupports.Support_Robot_Pose.INSTANCE;
        byte[] wire = support.encode(pose);
        // With an encapsulation header it is D_CDR2_LE, 00 09 00 00 (§7.6.3.1.2).
        byte[] expected = wire.length == body.length + 4
                ? concat(hex("00 09 00 00"), body) : body;
        if (!Arrays.equals(expected, wire)) {
            throw new AssertionError("Robot.Pose octets " + hexOf(wire) + ", expected " + hexOf(expected));
        }
        ZdTypes.Robot.Pose decoded = support.decode(wire);
        if (!decoded.robot_id.equals("r2d2") || decoded.x != 1.5 || decoded.y != -2.5 || decoded.theta != 0.75) {
            throw new AssertionError("Robot.Pose read back as " + decoded);
        }
        System.out.println("OK: Robot.Pose has the octets of DDS-XTypes 1.3 §7.4 (" + wire.length
                + " octets) and reads back");
    }

    private static byte[] concat(byte[] a, byte[] b) {
        byte[] out = Arrays.copyOf(a, a.length + b.length);
        System.arraycopy(b, 0, out, a.length, b.length);
        return out;
    }

    private static String hexOf(byte[] bytes) {
        StringBuilder out = new StringBuilder();
        for (byte b : bytes) {
            out.append(String.format("%02X ", b & 0xFF));
        }
        return out.toString().trim();
    }
}
