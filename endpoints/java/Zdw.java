// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Zdw -- ZeroDDS endpoint wire-core, pure Java 8 (ADR 0013).
//
// The `wire-core` unit of the native Java endpoint SDK: XCDR2 primitives
// byte-identical to the Rust core. No JNI, no native lib -- runs on any JVM.
// Byte-by-byte, explicit LE/BE wire order, stream-relative alignment capped
// at 4. DHEADER (delimited CDR2) via a sub-buffer, alignment-equivalent to the
// inline form because the body starts 4-aligned and the cap is 4.

import java.io.ByteArrayOutputStream;
import java.nio.charset.Charset;

public final class Zdw {

    public static final int LE = 0;
    public static final int BE = 1;
    static final int MAX_ALIGN = 4;
    static final Charset UTF8 = Charset.forName("UTF-8");

    private Zdw() {}

    /** A block of members serialized into a DHEADER-delimited sub-buffer. */
    public interface Body {
        void write(Writer w);
    }

    public static final class Writer {
        private final ByteArrayOutputStream out = new ByteArrayOutputStream();
        private final int endian;

        public Writer(int endian) { this.endian = endian; }

        private void align(int a) {
            int eff = (a <= MAX_ALIGN) ? a : MAX_ALIGN;
            int mask = eff - 1;
            int pad = (eff - (out.size() & mask)) & mask;
            for (int i = 0; i < pad; i++) out.write(0);
        }

        private void putUint(int a, byte[] le) {
            align(a);
            if (endian == BE) {
                for (int i = le.length - 1; i >= 0; i--) out.write(le[i]);
            } else {
                out.write(le, 0, le.length);
            }
        }

        public void u8(int v) { out.write(v & 0xFF); }

        public void u16(int v) {
            putUint(2, new byte[] { (byte) v, (byte) (v >> 8) });
        }

        public void u32(long v) {
            putUint(4, new byte[] {
                (byte) v, (byte) (v >> 8), (byte) (v >> 16), (byte) (v >> 24) });
        }

        public void u64(long v) {
            putUint(8, new byte[] {
                (byte) v, (byte) (v >> 8), (byte) (v >> 16), (byte) (v >> 24),
                (byte) (v >> 32), (byte) (v >> 40), (byte) (v >> 48), (byte) (v >> 56) });
        }

        public void f32(float v) { u32(Float.floatToRawIntBits(v) & 0xFFFFFFFFL); }
        public void f64(double v) { u64(Double.doubleToRawLongBits(v)); }

        public void str(String s) {
            byte[] raw = s.getBytes(UTF8);
            u32(raw.length + 1);
            out.write(raw, 0, raw.length);
            out.write(0);
        }

        public void seqU8(byte[] data) {
            u32(data.length);
            out.write(data, 0, data.length);
        }

        /** DHEADER-delimited block (@appendable/@mutable struct, or a
         *  sequence/array with a non-primitive element). */
        public void dheader(Body body) {
            Writer sub = new Writer(endian);
            body.write(sub);
            byte[] b = sub.bytes();
            u32(b.length);
            out.write(b, 0, b.length);
        }

        /** @mutable member (LC4): EMHEADER u32 = (M<<31)+(LC4<<28)+id, then the
         *  NEXTINT + body (via dheader). */
        public void emheader(int memberId, boolean mustUnderstand, Body body) {
            long em = (mustUnderstand ? 0x80000000L : 0L) + (4L << 28)
                    + (memberId & 0x0FFFFFFF);
            u32(em);
            dheader(body);
        }

        public byte[] bytes() { return out.toByteArray(); }
    }

    public static final class Reader {
        private final byte[] buf;
        private int pos = 0;
        private final int endian;

        public Reader(byte[] buf, int endian) { this.buf = buf; this.endian = endian; }

        private void align(int a) {
            int eff = (a <= MAX_ALIGN) ? a : MAX_ALIGN;
            int mask = eff - 1;
            pos += (eff - (pos & mask)) & mask;
        }

        private byte[] getUint(int a, int n) {
            align(a);
            byte[] le = new byte[n];
            for (int i = 0; i < n; i++) le[i] = buf[pos + i];
            pos += n;
            if (endian == BE) {
                for (int i = 0; i < n / 2; i++) {
                    byte t = le[i]; le[i] = le[n - 1 - i]; le[n - 1 - i] = t;
                }
            }
            return le;
        }

        public int u8() { return buf[pos++] & 0xFF; }

        public int u16() {
            byte[] le = getUint(2, 2);
            return (le[0] & 0xFF) | ((le[1] & 0xFF) << 8);
        }

        public long u32() {
            byte[] le = getUint(4, 4);
            return (le[0] & 0xFFL) | ((le[1] & 0xFFL) << 8)
                 | ((le[2] & 0xFFL) << 16) | ((le[3] & 0xFFL) << 24);
        }

        public long u64() {
            byte[] le = getUint(8, 8);
            long v = 0;
            for (int i = 7; i >= 0; i--) v = (v << 8) | (le[i] & 0xFFL);
            return v;
        }

        public float f32() { return Float.intBitsToFloat((int) u32()); }
        public double f64() { return Double.longBitsToDouble(u64()); }

        public String str() {
            long n = u32();
            String s = new String(buf, pos, (int) n - 1, UTF8);
            pos += (int) n;
            return s;
        }

        public byte[] seqU8() {
            long n = u32();
            byte[] r = new byte[(int) n];
            for (int i = 0; i < n; i++) r[i] = buf[pos++];
            return r;
        }

        public long dheaderRead() { return u32(); }

        /** Reads an LC4 member header: returns {memberId, mustUnderstand(0/1),
         *  nextint = body byte length}. */
        public long[] emheaderRead() {
            long em = u32();
            long ni = u32();
            return new long[] { em & 0x0FFFFFFF, ((em & 0x80000000L) != 0) ? 1 : 0, ni };
        }
    }
}
