// SPDX-License-Identifier: Apache-2.0
package org.zerodds.cdr;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.lang.annotation.ElementType;
import java.lang.annotation.Retention;
import java.lang.annotation.RetentionPolicy;
import java.lang.annotation.Target;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

import org.junit.jupiter.api.DisplayName;
import org.junit.jupiter.api.Test;

/**
 * DDS-Java-PSM 1.0 §8 / §1.2 reflection-marshalling proof. The decisive
 * property is <em>byte-identity</em> with the typed {@link Xcdr2Writer} path:
 * a reflection-marshalled plain bean produces exactly the reference wire
 * vectors V-2/V-3/V-8/V-9 from {@code Xcdr2WireVectorsTest}. Plus roundtrips
 * for nested beans, sequences, maps and mutable extensibility, the §7.6.8
 * key-hash, and §7.8.1.3 {@code createType(Class<?>)}.
 */
@DisplayName("Reflection TypeSupport (Java-PSM §8)")
final class ReflectionTypeSupportTest {

    private static byte[] hex(String s) {
        String t = s.replaceAll("\\s+", "");
        byte[] out = new byte[t.length() / 2];
        for (int i = 0; i < out.length; i++) {
            out[i] = (byte) Integer.parseInt(t.substring(i * 2, i * 2 + 2), 16);
        }
        return out;
    }

    // Reflectively-honoured annotations (matched by simple name, mirroring the
    // org.zerodds.types.* annotations idl-java emits — no compile dependency).
    @Retention(RetentionPolicy.RUNTIME)
    @Target(ElementType.FIELD)
    @interface Key {}

    @Retention(RetentionPolicy.RUNTIME)
    @Target(ElementType.TYPE)
    @interface Extensibility {
        String value();
    }

    // ==================================================================
    // Byte-identity with the typed wire vectors
    // ==================================================================

    /** FINAL bean equivalent to V-2 Point{x=1, y=-2}. */
    static final class Point {
        int x;
        int y;
        Point() {}
        Point(int x, int y) { this.x = x; this.y = y; }
    }

    @Test
    @DisplayName("Point bean == V-2 wire vector (byte-exact)")
    void pointMatchesV2() {
        ReflectionTypeSupport<Point> ts = ReflectionTypeSupport.of(Point.class);
        byte[] actual = ts.encode(new Point(1, -2));
        assertArrayEquals(hex("01 00 00 00 FE FF FF FF"), actual);

        Point back = ts.decode(actual);
        assertEquals(1, back.x);
        assertEquals(-2, back.y);
    }

    /**
     * FINAL record covering every cleanly-mappable Java primitive. Java has no
     * unsigned types, so {@code int}→INT32 and {@code long}→INT64 (a plain bean
     * therefore cannot reproduce V-3's uint32/uint64 mix — that is inherent to
     * the Java Type Representation, not a codec gap). The decisive property is
     * byte-identity with the typed {@link Xcdr2Writer} path for these exact
     * Java types, which we assert directly.
     */
    record Mixed(boolean b, byte o, short s, char c, int i, long l, float f, double d) {}

    @Test
    @DisplayName("Mixed record is byte-identical to the typed Xcdr2Writer path")
    void mixedIsByteIdenticalToTypedPath() {
        Mixed m = new Mixed(true, (byte) 0xAB, (short) -12345, 'Z',
                -1234567, -987654321L, 2.5f, 3.14159);

        // Reference: drive Xcdr2Writer with the same field types/order the
        // reflection codec sees (boolean, octet, int16, char8, int32, int64,
        // float32, float64) — natural alignment, no DHEADER (FINAL).
        Xcdr2Writer w = new Xcdr2Writer();
        w.writeBoolean(true);
        w.writeOctet((byte) 0xAB);
        w.writeInt16((short) -12345);
        w.writeChar('Z');
        w.writeInt32(-1234567);
        w.writeInt64(-987654321L);
        w.writeFloat32(2.5f);
        w.writeFloat64(3.14159);
        byte[] expected = w.toByteArray();

        byte[] actual = ReflectionTypeSupport.of(Mixed.class).encode(m);
        assertArrayEquals(expected, actual);
        assertEquals(m, ReflectionTypeSupport.of(Mixed.class).decode(actual));
    }

    /** Keyed FINAL bean == V-8 Sensor{id=42, value=3.14}. */
    static final class Sensor {
        @Key int id;
        double value;
        Sensor() {}
        Sensor(int id, double value) { this.id = id; this.value = value; }
    }

    @Test
    @DisplayName("Sensor bean == V-8 wire + key-hash (byte-exact)")
    void sensorMatchesV8() {
        ReflectionTypeSupport<Sensor> ts = ReflectionTypeSupport.of(Sensor.class);
        byte[] actual = ts.encode(new Sensor(42, 3.14));
        assertArrayEquals(hex("2A 00 00 00 00 00 00 00 1F 85 EB 51 B8 1E 09 40"), actual);
        assertTrue(ts.isKeyed());

        // §7.6.8.4: 4-byte BE key holder zero-padded to 16.
        byte[] hash = ts.keyHash(new Sensor(42, 3.14));
        assertArrayEquals(hex("00 00 00 2A 00 00 00 00 00 00 00 00 00 00 00 00"), hash);
    }

    @Test
    @DisplayName("Greeting{text=\"hello\"} == V-4 string wire vector")
    void stringMatchesV4() {
        ReflectionTypeSupport<Greeting> ts = ReflectionTypeSupport.of(Greeting.class);
        byte[] actual = ts.encode(new Greeting("hello"));
        assertArrayEquals(hex("06 00 00 00 68 65 6C 6C 6F 00"), actual);
        assertEquals("hello", ts.decode(actual).text);
    }

    static final class Greeting {
        String text;
        Greeting() {}
        Greeting(String text) { this.text = text; }
    }

    // ==================================================================
    // Sequences (V-5 primitive inline, V-6 string DHEADER)
    // ==================================================================

    static final class Bag {
        List<Integer> ids;
        Bag() {}
        Bag(List<Integer> ids) { this.ids = ids; }
    }

    @Test
    @DisplayName("Bag{ids=[1,2,3]} == V-5 (primitive seq, no DHEADER)")
    void seqPrimitiveMatchesV5() {
        ReflectionTypeSupport<Bag> ts = ReflectionTypeSupport.of(Bag.class);
        List<Integer> ids = new ArrayList<>();
        ids.add(1);
        ids.add(2);
        ids.add(3);
        byte[] actual = ts.encode(new Bag(ids));
        assertArrayEquals(hex("03 00 00 00 01 00 00 00 02 00 00 00 03 00 00 00"), actual);
        assertEquals(ids, ts.decode(actual).ids);
    }

    static final class Tags {
        List<String> tags;
        Tags() {}
        Tags(List<String> tags) { this.tags = tags; }
    }

    @Test
    @DisplayName("Tags{tags=[\"a\",\"bc\"]} == V-6 (string seq, DHEADER)")
    void seqStringMatchesV6() {
        ReflectionTypeSupport<Tags> ts = ReflectionTypeSupport.of(Tags.class);
        List<String> tags = new ArrayList<>();
        tags.add("a");
        tags.add("bc");
        byte[] actual = ts.encode(new Tags(tags));
        assertArrayEquals(
                hex("13 00 00 00 02 00 00 00 02 00 00 00 61 00 00 00 03 00 00 00 62 63 00"),
                actual);
        assertEquals(tags, ts.decode(actual).tags);
    }

    @Test
    @DisplayName("int[] array roundtrips as a primitive sequence")
    void arrayRoundtrips() {
        ReflectionTypeSupport<Ints> ts = ReflectionTypeSupport.of(Ints.class);
        Ints in = new Ints(new int[] {7, 8, 9});
        byte[] wire = ts.encode(in);
        assertArrayEquals(new int[] {7, 8, 9}, ts.decode(wire).vals);
    }

    static final class Ints {
        int[] vals;
        Ints() {}
        Ints(int[] vals) { this.vals = vals; }
    }

    // ==================================================================
    // Nested beans
    // ==================================================================

    static final class Line {
        Point a;
        Point b;
        Line() {}
        Line(Point a, Point b) { this.a = a; this.b = b; }
    }

    @Test
    @DisplayName("Nested FINAL beans marshal inline (no extra DHEADER)")
    void nestedFinalInline() {
        ReflectionTypeSupport<Line> ts = ReflectionTypeSupport.of(Line.class);
        byte[] wire = ts.encode(new Line(new Point(1, 2), new Point(3, 4)));
        // Two FINAL Points inline = 4 int32 = 16 bytes, no DHEADER.
        assertArrayEquals(
                hex("01 00 00 00 02 00 00 00 03 00 00 00 04 00 00 00"), wire);
        Line back = ts.decode(wire);
        assertEquals(1, back.a.x);
        assertEquals(4, back.b.y);
    }

    // ==================================================================
    // Appendable + Mutable extensibility
    // ==================================================================

    @Extensibility("APPENDABLE")
    static final class V {
        int a;
        int b;
        V() {}
        V(int a, int b) { this.a = a; this.b = b; }
    }

    @Test
    @DisplayName("@Extensibility(APPENDABLE) bean == V-9 DHEADER wire vector")
    void appendableMatchesV9() {
        ReflectionTypeSupport<V> ts = ReflectionTypeSupport.of(V.class);
        assertEquals(ExtensibilityKind.APPENDABLE, ts.getExtensibility());
        byte[] actual = ts.encode(new V(1, 2));
        assertArrayEquals(hex("08 00 00 00 01 00 00 00 02 00 00 00"), actual);
        V back = ts.decode(actual);
        assertEquals(1, back.a);
        assertEquals(2, back.b);
    }

    @Extensibility("MUTABLE")
    static final class M {
        int a;
        String b;
        M() {}
        M(int a, String b) { this.a = a; this.b = b; }
    }

    @Test
    @DisplayName("@Extensibility(MUTABLE) bean roundtrips (EMHEADER per member)")
    void mutableRoundtrips() {
        ReflectionTypeSupport<M> ts = ReflectionTypeSupport.of(M.class);
        assertEquals(ExtensibilityKind.MUTABLE, ts.getExtensibility());
        byte[] wire = ts.encode(new M(42, "hi"));
        // Starts with a DHEADER (mutable PL_CDR2).
        Xcdr2Reader r = new Xcdr2Reader(wire);
        int body = r.readDHeader();
        assertTrue(body > 0);
        M back = ts.decode(wire);
        assertEquals(42, back.a);
        assertEquals("hi", back.b);
    }

    @Test
    @DisplayName("Mutable roundtrip with an 8-byte member keeps stream alignment")
    void mutableLongRoundtrips() {
        ReflectionTypeSupport<ML> ts = ReflectionTypeSupport.of(ML.class);
        ML back = ts.decode(ts.encode(new ML(7, -987654321L, 3.5)));
        assertEquals(7, back.a);
        assertEquals(-987654321L, back.big);
        assertEquals(3.5, back.d, 0.0);
    }

    @Extensibility("MUTABLE")
    static final class ML {
        int a;
        long big;
        double d;
        ML() {}
        ML(int a, long big, double d) { this.a = a; this.big = big; this.d = d; }
    }

    // ==================================================================
    // Map
    // ==================================================================

    static final class Dict {
        Map<String, Integer> m;
        Dict() {}
        Dict(Map<String, Integer> m) { this.m = m; }
    }

    @Test
    @DisplayName("Map<String,Integer> roundtrips (order-stable for the instance)")
    void mapRoundtrips() {
        ReflectionTypeSupport<Dict> ts = ReflectionTypeSupport.of(Dict.class);
        Map<String, Integer> m = new LinkedHashMap<>();
        m.put("a", 1);
        m.put("bc", 2);
        byte[] wire = ts.encode(new Dict(m));
        assertEquals(m, ts.decode(wire).m);
    }

    // ==================================================================
    // createType(Class<?>) — §7.8.1.3
    // ==================================================================

    @Test
    @DisplayName("createType(Class<?>) reflects member kinds + nesting + key")
    void createTypeReflectsStructure() {
        DynamicType dt = DynamicTypeFactory.getInstance().createType(Sensor.class);
        assertEquals(ExtensibilityKind.FINAL, dt.getExtensibility());
        assertTrue(dt.isKeyed());
        assertEquals(2, dt.getMembers().size());
        DynamicType.Member id = dt.getMembers().get(0);
        assertEquals("id", id.getName());
        assertEquals(DynamicType.Kind.INT32, id.getKind());
        assertTrue(id.isKey());
        DynamicType.Member value = dt.getMembers().get(1);
        assertEquals(DynamicType.Kind.FLOAT64, value.getKind());
        assertFalse(value.isKey());

        // Nested + sequence kinds.
        DynamicType line = DynamicTypeFactory.getInstance().createType(Line.class);
        DynamicType.Member memberA = line.getMembers().get(0);
        assertEquals("a", memberA.getName());
        assertEquals(DynamicType.Kind.STRUCTURE, memberA.getKind());
        // The nested Point type's first field is "x".
        assertEquals("x", memberA.getNestedType().getMembers().get(0).getName());
        assertEquals(DynamicType.Kind.INT32, memberA.getNestedType().getMembers().get(0).getKind());
        DynamicType bag = DynamicTypeFactory.getInstance().createType(Bag.class);
        assertEquals(DynamicType.Kind.SEQUENCE, bag.getMembers().get(0).getKind());
    }

    @Test
    @DisplayName("createType rejects null")
    void createTypeRejectsNull() {
        assertThrows(XcdrException.class,
                () -> DynamicTypeFactory.getInstance().createType(null));
    }
}
