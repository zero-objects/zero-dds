// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// TopicTypedSmoke.java -- reference smoke for zerodds-xcdr2-java-1.0 §10.
//
// Shows the encode/decode roundtrip of a hand-implemented
// `org.zerodds.cdr.TopicTypeSupport<Point>` (matches what
// `zerodds-idlc --lang java` would emit per IDL `struct` -- see
// `tools/idlc/src/java_support.rs`), calling straight into the
// `org.zerodds.wire` runtime, the one XCDR2 codec this binding has.
// No live DDS -- only the XCDR2 wire layer.
//
// Run:
//   cd crates/java-omgdds/java
//   mvn compile exec:java -Dexec.mainClass=examples.TopicTypedSmoke \
//     -Dexec.classpathScope=test
//
// Or copy into src/main/java and call normally.

package examples;

import java.util.Objects;

import org.zerodds.cdr.EndianMode;
import org.zerodds.cdr.ExtensibilityKind;
import org.zerodds.cdr.TopicTypeSupport;
import org.zerodds.wire.Endianness;
import org.zerodds.wire.PlCdr1Framing;
import org.zerodds.wire.Reader;
import org.zerodds.wire.Writer;

public final class TopicTypedSmoke {

    public static final class Point {
        private int x;
        private int y;

        public Point() { }
        public Point(int x, int y) { this.x = x; this.y = y; }

        public int getX() { return x; }
        public int getY() { return y; }
        public void setX(int x) { this.x = x; }
        public void setY(int y) { this.y = y; }

        @Override
        public boolean equals(Object o) {
            if (!(o instanceof Point p)) return false;
            return x == p.x && y == p.y;
        }
        @Override
        public int hashCode() { return Objects.hash(x, y); }
        @Override
        public String toString() { return "Point(x=" + x + ", y=" + y + ")"; }
    }

    public static final class PointTypeSupport implements TopicTypeSupport<Point> {
        public static final PointTypeSupport INSTANCE = new PointTypeSupport();

        @Override public String getTypeName() { return "Point"; }
        @Override public boolean isKeyed() { return false; }
        @Override public ExtensibilityKind getExtensibility() {
            return ExtensibilityKind.FINAL;
        }

        private static Endianness order(EndianMode endian) {
            switch (endian) {
                case LITTLE_ENDIAN: return Endianness.LITTLE;
                case BIG_ENDIAN: return Endianness.BIG;
                default: throw new IllegalArgumentException("Unsupported byte order");
            }
        }

        @Override
        public byte[] encode(Point s) { return encode(s, EndianMode.LITTLE_ENDIAN); }

        @Override
        public byte[] encode(Point s, EndianMode endian) {
            Writer w = new Writer(order(endian), 4, PlCdr1Framing.defaults(), false);
            w.writeI32(s.getX());
            w.writeI32(s.getY());
            return w.finish();
        }

        @Override
        public Point decode(byte[] bytes) { return decode(bytes, 0, bytes.length); }

        @Override
        public Point decode(byte[] bytes, int offset, int length) {
            Reader r = new Reader(
                    java.util.Arrays.copyOfRange(bytes, offset, offset + length),
                    Endianness.LITTLE, 4, PlCdr1Framing.defaults());
            Point p = new Point();
            p.setX(r.readI32());
            p.setY(r.readI32());
            return p;
        }

        @Override
        public byte[] keyHash(Point s) { return new byte[16]; }
    }

    public static void main(String[] args) {
        Point p = new Point(42, -7);
        PointTypeSupport ts = PointTypeSupport.INSTANCE;

        System.out.println("type_name = " + ts.getTypeName());
        System.out.println("extensibility = " + ts.getExtensibility());
        System.out.println("sample = " + p);

        byte[] bytes = ts.encode(p);
        StringBuilder sb = new StringBuilder("wire = ");
        for (byte b : bytes) sb.append(String.format("%02X ", b & 0xFF));
        System.out.println(sb.toString());

        Point roundtripped = ts.decode(bytes);
        System.out.println("roundtrip = " + roundtripped);

        if (!p.equals(roundtripped)) {
            System.err.println("FAIL: roundtrip mismatch");
            System.exit(1);
        }
        System.out.println("OK");
    }
}
