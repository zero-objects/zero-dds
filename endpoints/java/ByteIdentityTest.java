// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Byte-identity test for the pure-Java (Java 8) wire-core (ADR 0013). Encodes
// the fixed @final SensorReading (same vector as the C/C++/Python tests + the
// Rust golden generator) in LE and BE, compares to the goldens, round-trips.
//
// usage: java ByteIdentityTest <golden_le.bin> <golden_be.bin>

import java.nio.file.Files;
import java.nio.file.Paths;
import java.util.Arrays;

public class ByteIdentityTest {

    static byte[] encode(int endian) {
        Zdw.Writer w = new Zdw.Writer(endian);
        w.u32(0xA1B2C3D4L);
        w.u16(0x1234);
        w.u8(0x5A);
        w.f32(3.5f);
        w.u64(0x0102030405060708L);
        w.str("bay-12");
        w.seqU8(new byte[] { (byte) 0xDE, (byte) 0xAD, (byte) 0xBE, (byte) 0xEF });
        return w.bytes();
    }

    static int check(int endian, String goldenPath, String tag) throws Exception {
        byte[] golden = Files.readAllBytes(Paths.get(goldenPath));
        byte[] out = encode(endian);
        if (!Arrays.equals(out, golden)) {
            System.err.println(tag + ": byte mismatch (java=" + out.length
                + " golden=" + golden.length + ")");
            return 1;
        }
        System.out.println(tag + ": " + out.length + " bytes byte-identical to Rust golden");
        Zdw.Reader r = new Zdw.Reader(out, endian);
        long id = r.u32(); int kind = r.u16(); int flags = r.u8();
        float value = r.f32(); long stamp = r.u64(); String label = r.str();
        byte[] raw = r.seqU8();
        if (id != 0xA1B2C3D4L || kind != 0x1234 || flags != 0x5A || value != 3.5f
            || stamp != 0x0102030405060708L || !label.equals("bay-12")
            || !Arrays.equals(raw, new byte[] { (byte) 0xDE, (byte) 0xAD, (byte) 0xBE, (byte) 0xEF })) {
            System.err.println(tag + ": round-trip mismatch");
            return 1;
        }
        System.out.println(tag + ": round-trip decode ok");
        return 0;
    }

    public static void main(String[] args) throws Exception {
        if (args.length < 2) {
            System.err.println("usage: ByteIdentityTest <golden_le.bin> <golden_be.bin>");
            System.exit(2);
        }
        int rc = check(Zdw.LE, args[0], "LE") | check(Zdw.BE, args[1], "BE");
        if (rc == 0) System.out.println("ALL OK");
        System.exit(rc);
    }
}
