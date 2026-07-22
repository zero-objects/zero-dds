// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Byte-identity test for the Java EMHEADER / @mutable path (ADR 0013). Same
// vector as endpoints/golden-gen encode_mutable.
//
// usage: java MutableTest <golden_mutable_le.bin> <golden_mutable_be.bin>

import java.nio.file.Files;
import java.nio.file.Paths;
import java.util.Arrays;

public class MutableTest {

    static byte[] encode(int endian) {
        Zdw.Writer w = new Zdw.Writer(endian);
        w.dheader(new Zdw.Body() {          // @mutable struct DHEADER
            public void write(Zdw.Writer b) {
                b.emheader(10, false, new Zdw.Body() {
                    public void write(Zdw.Writer mb) { mb.u32(0xDEADBEEFL); } });
                b.emheader(20, false, new Zdw.Body() {
                    public void write(Zdw.Writer mb) { mb.str("mut"); } });
                b.emheader(30, false, new Zdw.Body() {
                    public void write(Zdw.Writer mb) { mb.u16(0x0777); } });
            }
        });
        return w.bytes();
    }

    static int check(int endian, String path, String tag) throws Exception {
        byte[] golden = Files.readAllBytes(Paths.get(path));
        byte[] out = encode(endian);
        if (!Arrays.equals(out, golden)) {
            System.err.println(tag + ": byte mismatch (java=" + out.length
                + " golden=" + golden.length + ")");
            return 1;
        }
        System.out.println(tag + ": " + out.length
            + " bytes byte-identical to Rust golden (EMHEADER/@mutable)");
        Zdw.Reader r = new Zdw.Reader(out, endian);
        long dh = r.dheaderRead();
        // position tracking not exposed; decode the three members in wire order.
        long x = 0, k = 0; String s = null;
        for (int i = 0; i < 3; i++) {
            long[] h = r.emheaderRead();
            long id = h[0];
            if (id == 10) x = r.u32();
            else if (id == 20) s = r.str();
            else if (id == 30) k = r.u16();
        }
        if (x != 0xDEADBEEFL || !"mut".equals(s) || k != 0x0777 || dh <= 0) {
            System.err.println(tag + ": round-trip mismatch");
            return 1;
        }
        System.out.println(tag + ": round-trip decode ok");
        return 0;
    }

    public static void main(String[] args) throws Exception {
        if (args.length < 2) {
            System.err.println("usage: MutableTest <mutable_le> <mutable_be>");
            System.exit(2);
        }
        int rc = check(Zdw.LE, args[0], "LE") | check(Zdw.BE, args[1], "BE");
        if (rc == 0) System.out.println("ALL OK");
        System.exit(rc);
    }
}
