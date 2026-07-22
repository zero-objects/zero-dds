// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Byte-identity test for the Java DHEADER path: @appendable Outer with a
// nested @appendable Inner + sequence<Inner> + string (ADR 0013). Same vector
// as endpoints/golden-gen encode_nested.
//
// usage: java NestedTest <golden_nested_le.bin> <golden_nested_be.bin>

import java.nio.file.Files;
import java.nio.file.Paths;
import java.util.Arrays;

public class NestedTest {

    static final int[][] MANY = { { 0xAAAA, 0xBBBBCCCC }, { 0xDDDD, 0xEEEEFFFF } };

    static void encodeInner(Zdw.Writer w, final int a, final long b) {
        w.dheader(new Zdw.Body() {
            public void write(Zdw.Writer ib) { ib.u16(a); ib.u32(b); }
        });
    }

    static byte[] encode(int endian) {
        Zdw.Writer w = new Zdw.Writer(endian);
        w.dheader(new Zdw.Body() {           // outer @appendable
            public void write(Zdw.Writer b) {
                b.u32(0xCAFEBABEL);
                encodeInner(b, 0x1111, 0x22223333L);
                b.dheader(new Zdw.Body() {    // sequence<Inner> collection DHEADER
                    public void write(Zdw.Writer sub) {
                        sub.u32(MANY.length);
                        for (int[] m : MANY) encodeInner(sub, m[0], m[1] & 0xFFFFFFFFL);
                    }
                });
                b.str("nested");
            }
        });
        return w.bytes();
    }

    static long[] decodeInner(Zdw.Reader r) {
        r.dheaderRead();
        return new long[] { r.u16(), r.u32() };
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
            + " bytes byte-identical to Rust golden (DHEADER/nested/seq)");
        Zdw.Reader r = new Zdw.Reader(out, endian);
        r.dheaderRead();
        long id = r.u32();
        long[] one = decodeInner(r);
        r.dheaderRead();
        long n = r.u32();
        long[][] many = new long[(int) n][];
        for (int i = 0; i < n; i++) many[i] = decodeInner(r);
        String label = r.str();
        if (id != 0xCAFEBABEL || one[0] != 0x1111 || one[1] != 0x22223333L
            || n != 2 || many[0][0] != 0xAAAA || many[0][1] != 0xBBBBCCCCL
            || many[1][0] != 0xDDDD || many[1][1] != 0xEEEEFFFFL
            || !label.equals("nested")) {
            System.err.println(tag + ": round-trip mismatch");
            return 1;
        }
        System.out.println(tag + ": round-trip decode ok");
        return 0;
    }

    public static void main(String[] args) throws Exception {
        if (args.length < 2) {
            System.err.println("usage: NestedTest <nested_le> <nested_be>");
            System.exit(2);
        }
        int rc = check(Zdw.LE, args[0], "LE") | check(Zdw.BE, args[1], "BE");
        if (rc == 0) System.out.println("ALL OK");
        System.exit(rc);
    }
}
