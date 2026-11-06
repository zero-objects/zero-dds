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
import java.util.List;

import org.junit.jupiter.api.Test;
import org.omg.dds.type.Extensibility;
import org.omg.dds.type.Key;

/**
 * The Java Type Representation of DDS-Java-PSM 1.0 §8 as {@link ReflectionTypeSupport} writes
 * it, held to octets written out by hand from DDS-XTypes 1.3 §7.4 (XCDR2, little-endian): the
 * primitive kinds of Table 8.1, a list as a sequence, a nested structure, the extensibility
 * kinds, the member IDs of a MUTABLE structure (§7.3.1.2.1.1), the key holder of §7.6.8, and
 * {@code createType(Class<?>)} of §7.8.1.3.
 */
final class ReflectionTypeSupportTest {

    private static byte[] hex(String s) {
        String t = s.replaceAll("\\s+", "");
        byte[] out = new byte[t.length() / 2];
        for (int i = 0; i < out.length; i++) {
            out[i] = (byte) Integer.parseInt(t.substring(i * 2, i * 2 + 2), 16);
        }
        return out;
    }

    // ==================================================================
    // Byte-identity with the typed wire vectors
    // ==================================================================

    /** A FINAL bean of two int fields. */
    static final class Point {
        int x;
        int y;
        Point() {}
        Point(int x, int y) { this.x = x; this.y = y; }
    }

    @Test
    void javapsm_8_2_an_int_field_is_an_int32_of_a_final_structure() {
        ReflectionTypeSupport<Point> ts = ReflectionTypeSupport.of(Point.class);
        byte[] actual = ts.encode(new Point(1, -2));
        assertArrayEquals(hex("01 00 00 00 FE FF FF FF"), actual);

        Point back = ts.decode(actual);
        assertEquals(1, back.x);
        assertEquals(-2, back.y);
    }

    /** A FINAL record with a field of each primitive kind of Table 8.1. */
    // A record extends java.lang.Record, not java.lang.Object, so it is EXTENSIBLE unless it
    // says otherwise (DDS-Java-PSM 1.0 8.6.1.2); this one says FINAL.
    @Extensibility(Extensibility.Kind.FINAL_EXTENSIBILITY)
    record Mixed(boolean b, byte o, short s, char c, int i, long l, float f, double d) {}

    @Test
    void javapsm_8_2_the_primitive_fields_of_table_8_1_have_the_octets_of_xtypes_7_4() {
        Mixed m = new Mixed(true, (byte) 0xAB, (short) -12345, 'Z',
                -1234567, -987654321L, 2.5f, 3.14159);
        // boolean at 0, octet at 1, int16 at 2, char8 at 4, padding to 8, int32 at 8, int64 at
        // 12 (aligned to 4, XTypes 1.3 §7.4.1.1.1), float32 at 20, float64 at 24; FINAL: no DHEADER
        byte[] expected = hex("01 AB C7 CF 5A 00 00 00 79 29 ED FF 4F 97 21 C5 FF FF FF FF"
                + " 00 00 20 40 6E 86 1B F0 F9 21 09 40");
        assertArrayEquals(expected, ReflectionTypeSupport.of(Mixed.class).encode(m));
        assertEquals(m, ReflectionTypeSupport.of(Mixed.class).decode(expected));
    }

    /** A keyed FINAL bean: an int key and a double. */
    static final class Sensor {
        @Key int id;
        double value;
        Sensor() {}
        Sensor(int id, double value) { this.id = id; this.value = value; }
    }

    @Test
    void xtypes_7_4_1_1_1_a_double_after_an_int32_is_aligned_to_four_and_the_short_key_holder_is_padded() {
        ReflectionTypeSupport<Sensor> ts = ReflectionTypeSupport.of(Sensor.class);
        byte[] actual = ts.encode(new Sensor(42, 3.14));
        // XCDR2 MAXALIGN=4 (XTypes 1.3 §7.4.1.1.1): double at off 4, no 8-align
        // pad. Was 16 bytes under the over-alignment Bug XW; spec-correct is 12.
        assertArrayEquals(hex("2A 00 00 00 1F 85 EB 51 B8 1E 09 40"), actual);
        assertTrue(ts.isKeyed());

        // §7.6.8.4: 4-byte BE key holder zero-padded to 16.
        byte[] hash = ts.keyHash(new Sensor(42, 3.14));
        assertArrayEquals(hex("00 00 00 2A 00 00 00 00 00 00 00 00 00 00 00 00"), hash);
    }

    // ==================================================================
    // Sequences
    // ==================================================================

    static final class Bag {
        List<Integer> ids;
        Bag() {}
        Bag(List<Integer> ids) { this.ids = ids; }
    }

    @Test
    void javapsm_8_5_3_a_list_is_a_sequence_in_the_order_of_the_list() {
        ReflectionTypeSupport<Bag> ts = ReflectionTypeSupport.of(Bag.class);
        List<Integer> ids = new ArrayList<>();
        ids.add(1);
        ids.add(2);
        ids.add(3);
        byte[] actual = ts.encode(new Bag(ids));
        assertArrayEquals(hex("03 00 00 00 01 00 00 00 02 00 00 00 03 00 00 00"), actual);
        assertEquals(ids, ts.decode(actual).ids);
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
    void javapsm_8_6_1_a_nested_final_structure_is_written_inline() {
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

    // DDS-Java-PSM 1.0 §8.6.1.2 names FINAL, EXTENSIBLE and MUTABLE for the Java Type
    // Representation, and org.omg.dds.type.Extensibility.Kind has no APPENDABLE. XTypes 1.3
    // §7.2.2.4.4.4: EXTENSIBLE is the name XTypes 1.1 gave to APPENDABLE, so the class is
    // written delimited, with a DHEADER.
    @Extensibility(Extensibility.Kind.EXTENSIBLE_EXTENSIBILITY)
    static final class V {
        int a;
        int b;
        V() {}
        V(int a, int b) { this.a = a; this.b = b; }
    }

    @Test
    void javapsm_8_6_1_2_extensible_is_written_as_appendable_with_a_dheader() {
        ReflectionTypeSupport<V> ts = ReflectionTypeSupport.of(V.class);
        assertEquals(ExtensibilityKind.APPENDABLE, ts.getExtensibility());
        byte[] actual = ts.encode(new V(1, 2));
        assertArrayEquals(hex("08 00 00 00 01 00 00 00 02 00 00 00"), actual);
        V back = ts.decode(actual);
        assertEquals(1, back.a);
        assertEquals(2, back.b);
    }

    @Test
    void xtypes_7_4_3_5_3_a_mutable_structure_numbers_its_members_from_zero_and_aligns_an_eight_octet_member_to_four() {
        ReflectionTypeSupport<ML> ts = ReflectionTypeSupport.of(ML.class);
        // The DHEADER 44; per member an EMHEADER with LC 4 (0x40000000 + member ID, IDs from 0 by
        // §7.3.1.2.1.1), its NEXTINT and the value; the int64 and the float64 at offsets 24 and
        // 40, aligned to 4 (§7.4.1.1.1).
        byte[] expected = hex("2C 00 00 00"
                + " 00 00 00 40 04 00 00 00 07 00 00 00"
                + " 01 00 00 40 08 00 00 00 4F 97 21 C5 FF FF FF FF"
                + " 02 00 00 40 08 00 00 00 00 00 00 00 00 00 0C 40");
        assertArrayEquals(expected, ts.encode(new ML(7, -987654321L, 3.5)));
        ML back = ts.decode(expected);
        assertEquals(7, back.a);
        assertEquals(-987654321L, back.big);
        assertEquals(3.5, back.d, 0.0);
    }

    @Extensibility(Extensibility.Kind.MUTABLE_EXTENSIBILITY)
    static final class ML {
        int a;
        long big;
        double d;
        ML() {}
        ML(int a, long big, double d) { this.a = a; this.big = big; this.d = d; }
    }

    // ==================================================================
    // createType(Class<?>) — §7.8.1.3
    // ==================================================================

    @Test
    void javapsm_7_8_1_3_create_type_reflects_member_kinds_nesting_and_key() {
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

}
