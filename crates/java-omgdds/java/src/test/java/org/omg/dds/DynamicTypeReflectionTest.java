// SPDX-License-Identifier: Apache-2.0
package org.omg.dds;

import org.junit.jupiter.api.Test;
import org.omg.dds.core.ServiceEnvironment;
import org.omg.dds.type.Key;
import org.omg.dds.type.SerializeAs;
import org.omg.dds.type.TypeKind;
import org.omg.dds.type.UnionDiscriminator;
import org.omg.dds.type.UnionMember;
import org.omg.dds.type.dynamic.DynamicType;
import org.omg.dds.type.dynamic.DynamicTypeFactory;
import org.zerodds.cdr.DynamicTypeFactoryImpl;

import java.util.ArrayList;
import java.util.Collection;
import java.util.HashMap;
import java.util.List;
import java.util.Map;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * DDS-Java-PSM 1.0 §7.8.1.3: {@code createType(Class)} gives the DynamicType
 * that the Java Type Representation (Clause 8) assigns to a class. Each
 * primitive is checked against Table 8.1.
 */
class DynamicTypeReflectionTest {

    private static final ServiceEnvironment ENV = Qos.env();
    private static final DynamicTypeFactoryImpl TYPES =
            (DynamicTypeFactoryImpl) DynamicTypeFactory.getInstance(ENV);

    private static TypeKind kindOf(DynamicType struct, String member) {
        return struct.getMember(member).getDescriptor().getType().getKind();
    }

    @SuppressWarnings("unused")
    public static final class Primitives {
        int i;
        short s;
        long l;
        float f;
        double d;
        char c;
        byte b;
        boolean z;
        Integer boxedI;
        Short boxedS;
        Long boxedL;
        Float boxedF;
        Double boxedD;
        Character boxedC;
        Byte boxedB;
        Boolean boxedZ;
        String text;
    }

    @Test
    void everyJavaPrimitiveHasTheDdsTypeOfTable81() {
        DynamicType type = TYPES.createType(Primitives.class);
        assertEquals(TypeKind.STRUCTURE_TYPE, type.getKind());
        // INT, java.lang.Integer -> INT32 and so on, row by row of Table 8.1.
        assertEquals(TypeKind.INT_32_TYPE, kindOf(type, "i"));
        assertEquals(TypeKind.INT_16_TYPE, kindOf(type, "s"));
        assertEquals(TypeKind.INT_64_TYPE, kindOf(type, "l"));
        assertEquals(TypeKind.FLOAT_32_TYPE, kindOf(type, "f"));
        assertEquals(TypeKind.FLOAT_64_TYPE, kindOf(type, "d"));
        assertEquals(TypeKind.CHAR_8_TYPE, kindOf(type, "c"));
        assertEquals(TypeKind.BYTE_TYPE, kindOf(type, "b"));
        assertEquals(TypeKind.BOOLEAN_TYPE, kindOf(type, "z"));
        assertEquals(TypeKind.INT_32_TYPE, kindOf(type, "boxedI"));
        assertEquals(TypeKind.INT_16_TYPE, kindOf(type, "boxedS"));
        assertEquals(TypeKind.INT_64_TYPE, kindOf(type, "boxedL"));
        assertEquals(TypeKind.FLOAT_32_TYPE, kindOf(type, "boxedF"));
        assertEquals(TypeKind.FLOAT_64_TYPE, kindOf(type, "boxedD"));
        assertEquals(TypeKind.CHAR_8_TYPE, kindOf(type, "boxedC"));
        assertEquals(TypeKind.BYTE_TYPE, kindOf(type, "boxedB"));
        assertEquals(TypeKind.BOOLEAN_TYPE, kindOf(type, "boxedZ"));
        // java.lang.String -> STRING<CHAR8>, unbounded
        assertEquals(TypeKind.STRING_TYPE, kindOf(type, "text"));
        assertEquals("String", type.getMember("text").getDescriptor().getType().getName());
    }

    @SuppressWarnings("unused")
    public static final class Customized {
        @SerializeAs(TypeKind.UINT_16_TYPE) short u16;
        @SerializeAs(TypeKind.UINT_32_TYPE) long u32;
        @SerializeAs(TypeKind.UINT_64_TYPE) java.math.BigInteger u64;
        @SerializeAs(TypeKind.FLOAT_128_TYPE) java.math.BigDecimal f128;
        @SerializeAs(TypeKind.CHAR_32_TYPE) int c32;
        @SerializeAs(value = TypeKind.STRING_TYPE, collectionElementKind = TypeKind.CHAR_32_TYPE,
                bound = 8) String wide;
        @SerializeAs(value = TypeKind.SEQUENCE_TYPE, collectionElementKind = TypeKind.INT_16_TYPE,
                bound = 4) List<Short> shorts;
        @SerializeAs(value = TypeKind.ARRAY_TYPE, bound = {2, 3}) int[][] grid;
        @SerializeAs(value = TypeKind.BITSET_TYPE, bound = 12) int flags;
    }

    @Test
    void serializeAsChoosesTheKindOfTheMember() {
        DynamicType type = TYPES.createType(Customized.class);
        assertEquals(TypeKind.UINT_16_TYPE, kindOf(type, "u16"));
        assertEquals(TypeKind.UINT_32_TYPE, kindOf(type, "u32"));
        assertEquals(TypeKind.UINT_64_TYPE, kindOf(type, "u64"));
        assertEquals(TypeKind.FLOAT_128_TYPE, kindOf(type, "f128"));
        assertEquals(TypeKind.CHAR_32_TYPE, kindOf(type, "c32"));
        DynamicType wide = type.getMember("wide").getDescriptor().getType();
        assertEquals("WString", wide.getName());
        assertEquals(List.of(8), wide.getDescriptor().getBound());
        DynamicType shorts = type.getMember("shorts").getDescriptor().getType();
        assertEquals(TypeKind.SEQUENCE_TYPE, shorts.getKind());
        assertEquals(TypeKind.INT_16_TYPE, shorts.getDescriptor().getElementType().getKind());
        assertEquals(List.of(4), shorts.getDescriptor().getBound());
        DynamicType grid = type.getMember("grid").getDescriptor().getType();
        assertEquals(TypeKind.ARRAY_TYPE, grid.getKind());
        assertEquals(List.of(2, 3), grid.getDescriptor().getBound());
        assertEquals(TypeKind.INT_32_TYPE, grid.getDescriptor().getElementType().getKind());
        DynamicType flags = type.getMember("flags").getDescriptor().getType();
        assertEquals(TypeKind.BITSET_TYPE, flags.getKind());
        assertEquals(List.of(12), flags.getDescriptor().getBound());
    }

    public enum Color { RED, GREEN, BLUE }

    @SuppressWarnings("unused")
    public static final class Inner {
        int x;
    }

    @SuppressWarnings("unused")
    public static final class Outer {
        @Key int id;
        Color color;
        Inner inner;
        double[] samples;
        Collection<String> names;
        Map<String, Integer> counts;
        List<Inner> inners;
    }

    @Test
    void enumerationsStructuresCollectionsAndMapsFollowTable81() {
        DynamicType type = TYPES.createType(Outer.class);
        assertEquals(TypeKind.ENUMERATION_TYPE, kindOf(type, "color"));
        assertEquals(3, type.getMember("color").getDescriptor().getType().getAllMembers().size());
        assertEquals(TypeKind.STRUCTURE_TYPE, kindOf(type, "inner"));
        assertEquals(TypeKind.INT_32_TYPE, kindOf(type.getMember("inner").getDescriptor().getType(), "x"));
        DynamicType samples = type.getMember("samples").getDescriptor().getType();
        assertEquals(TypeKind.SEQUENCE_TYPE, samples.getKind());
        assertEquals(TypeKind.FLOAT_64_TYPE, samples.getDescriptor().getElementType().getKind());
        DynamicType names = type.getMember("names").getDescriptor().getType();
        assertEquals(TypeKind.SEQUENCE_TYPE, names.getKind());
        assertEquals(TypeKind.STRING_TYPE, names.getDescriptor().getElementType().getKind());
        DynamicType counts = type.getMember("counts").getDescriptor().getType();
        assertEquals(TypeKind.MAP_TYPE, counts.getKind());
        assertEquals(TypeKind.STRING_TYPE, counts.getDescriptor().getKeyElementType().getKind());
        assertEquals(TypeKind.INT_32_TYPE, counts.getDescriptor().getElementType().getKind());
        assertEquals(TypeKind.STRUCTURE_TYPE, type.getMember("inners").getDescriptor().getType()
                .getDescriptor().getElementType().getKind());
        // @Key and the extensibility are annotations of the type
        assertTrue(type.getMember("id").getAnnotations().stream()
                .anyMatch(a -> a.getType().getName().equals("Key")));
        assertNotNull(type.getAnnotation("Extensibility"));
    }

    @SerializeAs(TypeKind.UNION_TYPE)
    public static final class Choice {
        @UnionDiscriminator int kind;
        @UnionMember(discriminatorValue = 1) int number;
        @UnionMember(discriminatorValue = 2) String text;
    }

    @Test
    void aUnionHasItsDiscriminatorTypeAndLabels() {
        DynamicType type = TYPES.createType(Choice.class);
        assertEquals(TypeKind.UNION_TYPE, type.getKind());
        assertEquals(TypeKind.INT_32_TYPE, type.getDescriptor().getDiscriminatorType().getKind());
        assertEquals(2, type.getAllMembers().size());
        assertEquals(List.of(1), type.getMember("number").getDescriptor().getLabel());
        assertEquals(List.of(2), type.getMember("text").getDescriptor().getLabel());
    }

    @SerializeAs(TypeKind.ANNOTATION_TYPE)
    public @interface Tag {
        String label() default "none";
        int weight();
    }

    public @interface Unmarked {
        int value();
    }

    @Test
    void anAnnotationTypeIsRepresentedOnlyWhenMarked() {
        DynamicType tag = TYPES.createType(Tag.class);
        assertEquals(TypeKind.ANNOTATION_TYPE, tag.getKind());
        assertEquals(TypeKind.STRING_TYPE, kindOf(tag, "label"));
        assertEquals("none", tag.getMember("label").getDescriptor().getDefaultValue());
        assertEquals(TypeKind.INT_32_TYPE, kindOf(tag, "weight"));
        // §8.9: ignored by default
        assertThrows(IllegalArgumentException.class, () -> TYPES.createType(Unmarked.class));
    }

    @SuppressWarnings("unused")
    public static final class Raw {
        List values;
    }

    @SuppressWarnings("unused")
    public static final class Untyped {
        Object any;
    }

    @Test
    void aJavaTypeWithoutAKindIsRefused() {
        assertThrows(IllegalArgumentException.class, () -> TYPES.createType(Raw.class));
        assertThrows(IllegalArgumentException.class, () -> TYPES.createType(Untyped.class));
        assertThrows(IllegalArgumentException.class, () -> TYPES.createType((Class<?>) null));
    }

    @SuppressWarnings("unused")
    public static final class Mixed {
        ArrayList<Map<String, Long>> rows;
        HashMap<Integer, String> byId;
    }

    @Test
    void nestedGenericsAreFollowed() {
        DynamicType type = TYPES.createType(Mixed.class);
        DynamicType rows = type.getMember("rows").getDescriptor().getType();
        DynamicType row = rows.getDescriptor().getElementType();
        assertEquals(TypeKind.MAP_TYPE, row.getKind());
        assertEquals(TypeKind.INT_64_TYPE, row.getDescriptor().getElementType().getKind());
        assertEquals(TypeKind.INT_32_TYPE,
                type.getMember("byId").getDescriptor().getType().getDescriptor()
                        .getKeyElementType().getKind());
    }
}
