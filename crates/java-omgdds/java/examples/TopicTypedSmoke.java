// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// TopicTypedSmoke.java -- reference smoke for zerodds-xcdr2-java-1.0 §10.
//
// Shows the encode/decode roundtrip of a hand-implemented
// `org.zerodds.cdr.TopicTypeSupport<Point>` (matches what idl-java
// would emit per IDL `struct`). No live DDS -- only the XCDR2
// wire layer.
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
import org.zerodds.cdr.Xcdr2Reader;
import org.zerodds.cdr.Xcdr2Writer;

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

        @Override
        public byte[] encode(Point s) { return encode(s, EndianMode.LITTLE_ENDIAN); }

        @Override
        public byte[] encode(Point s, EndianMode endian) {
            Xcdr2Writer w = new Xcdr2Writer(endian);
            w.writeInt32(s.getX());
            w.writeInt32(s.getY());
            return w.toByteArray();
        }

        @Override
        public Point decode(byte[] bytes) { return decode(bytes, 0, bytes.length); }

        @Override
        public Point decode(byte[] bytes, int offset, int length) {
            Xcdr2Reader r = new Xcdr2Reader(bytes, offset, length, EndianMode.LITTLE_ENDIAN);
            Point p = new Point();
            p.setX(r.readInt32());
            p.setY(r.readInt32());
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
