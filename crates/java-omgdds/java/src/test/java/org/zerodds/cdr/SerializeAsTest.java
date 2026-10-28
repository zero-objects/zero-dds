// SPDX-License-Identifier: Apache-2.0
package org.zerodds.cdr;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;
import org.omg.dds.type.Extensibility;
import org.omg.dds.type.SerializeAs;
import org.omg.dds.type.TypeKind;
import org.omg.dds.type.UnionDiscriminator;
import org.omg.dds.type.UnionMember;

import java.math.BigInteger;
import java.util.ArrayList;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * DDS-Java-PSM 1.0 §8.2 {@code @SerializeAs}, §8.5.1 strings and §8.6.2
 * unions, with the expected octets written out from the clauses and from
 * XTypes 1.3 (XCDR2, little-endian).
 */
class SerializeAsTest {

    @AfterEach
    void clearSetting() {
        System.clearProperty(NarrowStrings.PROPERTY);
    }

    private static byte[] bytes(int... values) {
        byte[] out = new byte[values.length];
        for (int i = 0; i < out.length; i++) {
            out[i] = (byte) values[i];
        }
        return out;
    }

    private static <T> byte[] encode(T value) {
        @SuppressWarnings("unchecked")
        Class<T> type = (Class<T>) value.getClass();
        return ReflectionTypeSupport.of(type).encode(value);
    }

    private static <T> T roundTrip(T value) {
        @SuppressWarnings("unchecked")
        Class<T> type = (Class<T>) value.getClass();
        ReflectionTypeSupport<T> ts = ReflectionTypeSupport.of(type);
        return ts.decode(ts.encode(value));
    }

    // --- Table 8.2: primitive kinds ----------------------------------------

    public static final class Unsigned32 {
        @SerializeAs(TypeKind.UINT_32_TYPE)
        public long value;
    }

    @Test
    void uint32HeldInALongKeepsTheLogicalValue() {
        Unsigned32 v = new Unsigned32();
        v.value = 4_000_000_000L; // 0xEE6B2800
        assertArrayEquals(bytes(0x00, 0x28, 0x6b, 0xee), encode(v));
        assertEquals(4_000_000_000L, roundTrip(v).value);
        v.value = 4_294_967_296L;
        assertThrows(XcdrException.class, () -> encode(v));
        v.value = -1;
        assertThrows(XcdrException.class, () -> encode(v));
    }

    public static final class Unsigned32Pattern {
        @SerializeAs(TypeKind.UINT_32_TYPE)
        public int value;
    }

    @Test
    void uint32HeldInAnIntKeepsTheBitPattern() {
        Unsigned32Pattern v = new Unsigned32Pattern();
        v.value = -1;
        assertArrayEquals(bytes(0xff, 0xff, 0xff, 0xff), encode(v));
        assertEquals(-1, roundTrip(v).value);
    }

    public static final class Unsigned16 {
        @SerializeAs(TypeKind.UINT_16_TYPE)
        public int wide;
        @SerializeAs(TypeKind.UINT_16_TYPE)
        public short narrow;
    }

    @Test
    void uint16IntKeepsTheValueAndShortTheBitPattern() {
        Unsigned16 v = new Unsigned16();
        v.wide = 65535;
        v.narrow = -2;
        assertArrayEquals(bytes(0xff, 0xff, 0xfe, 0xff), encode(v));
        Unsigned16 back = roundTrip(v);
        assertEquals(65535, back.wide);
        assertEquals((short) -2, back.narrow);
        v.wide = 65536;
        assertThrows(XcdrException.class, () -> encode(v));
    }

    public static final class Unsigned64 {
        @SerializeAs(TypeKind.UINT_64_TYPE)
        public BigInteger big;
        @SerializeAs(TypeKind.UINT_64_TYPE)
        public long pattern;
    }

    @Test
    void uint64TakesABigIntegerOrALong() {
        Unsigned64 v = new Unsigned64();
        v.big = new BigInteger("18446744073709551615");
        v.pattern = -1L;
        byte[] expected = new byte[16];
        java.util.Arrays.fill(expected, (byte) 0xff);
        assertArrayEquals(expected, encode(v));
        Unsigned64 back = roundTrip(v);
        assertEquals(v.big, back.big);
        assertEquals(-1L, back.pattern);
        v.big = BigInteger.ONE.shiftLeft(64);
        assertThrows(XcdrException.class, () -> encode(v));
    }

    public static final class Quad {
        @SerializeAs(TypeKind.FLOAT_128_TYPE)
        public double value;
    }

    @Test
    void float128HeldInADouble() {
        Quad v = new Quad();
        v.value = 1.0;
        // binary128 1.0: sign 0, exponent 0x3fff, fraction 0 -- little-endian.
        byte[] expected = new byte[16];
        expected[14] = (byte) 0xff;
        expected[15] = 0x3f;
        assertArrayEquals(expected, encode(v));
        assertEquals(1.0, roundTrip(v).value);
    }

    public static final class Wide32 {
        @SerializeAs(TypeKind.CHAR_32_TYPE)
        public int codePoint;
    }

    @Test
    void char32IsAFourOctetCodePoint() {
        Wide32 v = new Wide32();
        v.codePoint = 0x1F600;
        assertArrayEquals(bytes(0x00, 0xf6, 0x01, 0x00), encode(v));
        assertEquals(0x1F600, roundTrip(v).codePoint);
    }

    public static final class WrongKind {
        @SerializeAs(TypeKind.INT_32_TYPE)
        public long value;
    }

    @Test
    void aKindTable82DoesNotListForTheTypeIsRefused() {
        XcdrException e = assertThrows(XcdrException.class,
                () -> ReflectionTypeSupport.of(WrongKind.class));
        assertEquals(true, e.getMessage().contains("INT_32_TYPE"));
    }

    // --- §8.5.1 strings -----------------------------------------------------

    public static final class Text {
        public String value;
    }

    @Test
    void aNarrowStringIsTruncatedToItsLowOctetByDefault() {
        // DDS-Java-PSM 1.0 8.5.1: "each Java character shall be truncated to its
        // least-significant byte"
        Text t = new Text();
        t.value = "été"; // U+00E9 U+0074 U+00E9
        assertArrayEquals(bytes(4, 0, 0, 0, 0xe9, 0x74, 0xe9, 0), encode(t));
        assertEquals("été", roundTrip(t).value);
        // U+0141 is truncated to 0x41, as the clause says.
        t.value = "AŁ";
        assertArrayEquals(bytes(3, 0, 0, 0, 0x41, 0x41, 0), encode(t));
    }

    @Test
    void theUtf8SettingWritesTheFormOfXTypes13() {
        System.setProperty(NarrowStrings.PROPERTY, "utf8");
        Text t = new Text();
        t.value = "été";
        // XTypes 1.3 7.4.1.1.2; tests/interop-corpus/string-adjacent-widths:
        // 06000000 c3a9 74 c3a9 00
        assertArrayEquals(bytes(6, 0, 0, 0, 0xc3, 0xa9, 0x74, 0xc3, 0xa9, 0), encode(t));
        assertEquals("été", roundTrip(t).value);
        // a character above U+00FF survives
        t.value = "AŁ";
        assertArrayEquals(bytes(4, 0, 0, 0, 0x41, 0xc5, 0x81, 0), encode(t));
        assertEquals("AŁ", roundTrip(t).value);
        System.setProperty(NarrowStrings.PROPERTY, "something else");
        assertThrows(XcdrException.class, () -> encode(t));
    }

    @Test
    void theTwoFormsAgreeOnTheCharactersBelowU0080() {
        Text t = new Text();
        t.value = "plain ASCII";
        byte[] truncated = encode(t);
        System.setProperty(NarrowStrings.PROPERTY, "utf8");
        assertArrayEquals(truncated, encode(t));
    }

    public static final class WideText {
        @SerializeAs(value = TypeKind.STRING_TYPE, collectionElementKind = TypeKind.CHAR_32_TYPE)
        public String value;
    }

    @Test
    void aWideStringIsMarkedWithSerializeAs() {
        WideText t = new WideText();
        t.value = "été";
        // DDS-Java-PSM 1.0 8.5.1: one 32-bit character per code point
        assertArrayEquals(bytes(12, 0, 0, 0, 0xe9, 0, 0, 0, 0x74, 0, 0, 0, 0xe9, 0, 0, 0), encode(t));
        assertEquals("été", roundTrip(t).value);
        // the setting utf16 is the wstring of the corpus (tests/interop-corpus/string-adjacent-widths)
        System.setProperty(WideStrings.PROPERTY, "utf16");
        try {
            assertArrayEquals(bytes(6, 0, 0, 0, 0xe9, 0, 0x74, 0, 0xe9, 0), encode(t));
            assertEquals("été", roundTrip(t).value);
        } finally {
            System.clearProperty(WideStrings.PROPERTY);
        }
    }

    public static final class BoundedText {
        @SerializeAs(value = TypeKind.STRING_TYPE, bound = {3})
        public String value;
    }

    @Test
    void aBoundedStringRefusesALongerValue() {
        BoundedText t = new BoundedText();
        t.value = "abc";
        assertEquals("abc", roundTrip(t).value);
        t.value = "abcd";
        assertThrows(XcdrException.class, () -> encode(t));
    }

    // --- Collections --------------------------------------------------------

    public static final class Collections {
        @SerializeAs(value = TypeKind.ARRAY_TYPE, bound = {3})
        public int[] fixed = new int[3];
        @SerializeAs(value = TypeKind.SEQUENCE_TYPE,
                collectionElementKind = TypeKind.UINT_16_TYPE, bound = {4})
        public List<Integer> narrow = new ArrayList<>();
    }

    @Test
    void anArrayHasNoLengthAndASequenceMayNameItsElementKind() {
        Collections c = new Collections();
        c.fixed = new int[] {1, 2, 3};
        c.narrow.add(1);
        c.narrow.add(65535);
        // int[3]: three int32; then the sequence: length 2 and two uint16.
        assertArrayEquals(bytes(1, 0, 0, 0, 2, 0, 0, 0, 3, 0, 0, 0,
                2, 0, 0, 0, 1, 0, 0xff, 0xff), encode(c));
        Collections back = roundTrip(c);
        assertArrayEquals(new int[] {1, 2, 3}, back.fixed);
        assertEquals(List.of(1, 65535), back.narrow);

        c.fixed = new int[] {1, 2};
        assertThrows(XcdrException.class, () -> encode(c));
        c.fixed = new int[] {1, 2, 3};
        for (int i = 0; i < 5; i++) {
            c.narrow.add(i);
        }
        assertThrows(XcdrException.class, () -> encode(c));
    }

    public static final class Matrix {
        @SerializeAs(value = TypeKind.ARRAY_TYPE, bound = {2, 3})
        public int[][] cells = new int[2][3];
    }

    @Test
    void anArrayOfSeveralDimensionsIsWrittenRowMajor() {
        Matrix m = new Matrix();
        m.cells = new int[][] {{1, 2, 3}, {4, 5, 6}};
        assertArrayEquals(bytes(1, 0, 0, 0, 2, 0, 0, 0, 3, 0, 0, 0, 4, 0, 0, 0, 5, 0, 0, 0,
                6, 0, 0, 0), encode(m));
        assertArrayEquals(m.cells, roundTrip(m).cells);
    }

    // --- §8.6.2 unions --------------------------------------------------------

    @SerializeAs(TypeKind.UNION_TYPE)
    public static final class Choice {
        @UnionDiscriminator
        public int kind;
        @UnionMember(discriminatorValue = 1)
        public int number;
        @UnionMember(discriminatorValue = 2)
        public String text;
    }

    @Test
    void aUnionIsItsDiscriminatorAndTheSelectedMember() {
        Choice c = new Choice();
        c.kind = 2;
        c.text = "hi";
        assertArrayEquals(bytes(2, 0, 0, 0, 3, 0, 0, 0, 'h', 'i', 0), encode(c));
        Choice back = roundTrip(c);
        assertEquals(2, back.kind);
        assertEquals("hi", back.text);
        assertEquals(0, back.number);

        c = new Choice();
        c.kind = 1;
        c.number = 77;
        assertArrayEquals(bytes(1, 0, 0, 0, 77, 0, 0, 0), encode(c));
        assertEquals(77, roundTrip(c).number);
    }

    @Test
    void aDiscriminatorThatSelectsNoMemberIsWrittenAlone() {
        Choice c = new Choice();
        c.kind = 9;
        c.number = 5; // not selected, not written
        assertArrayEquals(bytes(9, 0, 0, 0), encode(c));
        Choice back = roundTrip(c);
        assertEquals(9, back.kind);
        assertEquals(0, back.number);
        assertNull(back.text);
    }

    @Extensibility(Extensibility.Kind.MUTABLE_EXTENSIBILITY)
    @SerializeAs(TypeKind.UNION_TYPE)
    public static final class MutableChoice {
        @UnionDiscriminator
        public int kind;
        @UnionMember(discriminatorValue = 1)
        public int number;
        @UnionMember(discriminatorValue = 2)
        public String text;
    }

    @Test
    void aMutableUnionCarriesItsDiscriminatorAsMemberZero() {
        MutableChoice c = new MutableChoice();
        c.kind = 2;
        c.text = "x";
        MutableChoice back = roundTrip(c);
        assertEquals(2, back.kind);
        assertEquals("x", back.text);
    }

    public static final class NotAUnion {
        @UnionDiscriminator
        public int kind;
    }

    @SerializeAs(TypeKind.UNION_TYPE)
    public static final class NoDiscriminator {
        @UnionMember(discriminatorValue = 1)
        public int number;
    }

    @SerializeAs(TypeKind.UNION_TYPE)
    public static final class TwoDiscriminators {
        @UnionDiscriminator
        public int a;
        @UnionDiscriminator
        public int b;
    }

    @SerializeAs(TypeKind.UNION_TYPE)
    public static final class UnmarkedMember {
        @UnionDiscriminator
        public int kind;
        public int number;
    }

    @SerializeAs(TypeKind.UNION_TYPE)
    public static final class DuplicateLabel {
        @UnionDiscriminator
        public int kind;
        @UnionMember(discriminatorValue = 1)
        public int a;
        @UnionMember(discriminatorValue = 1)
        public int b;
    }

    @Test
    void aMisplacedUnionAnnotationIsRefused() {
        for (Class<?> bad : new Class<?>[] {NotAUnion.class, NoDiscriminator.class,
                TwoDiscriminators.class, UnmarkedMember.class, DuplicateLabel.class}) {
            assertThrows(XcdrException.class, () -> ReflectionTypeSupport.of(bad), bad.getName());
        }
    }

    // --- §8.7 bit sets ---------------------------------------------------------

    @org.omg.dds.type.BitSet
    @org.omg.dds.type.BitBound(8)
    public enum Flag { READ, WRITE, EXEC }

    @org.omg.dds.type.BitSet
    public enum Wide { A, B, C, D }

    public enum Plain { X, Y }

    public static final class Flags {
        @org.omg.dds.type.BitSet
        public java.util.EnumSet<Flag> set = java.util.EnumSet.noneOf(Flag.class);
    }

    @Test
    void anEnumSetMarkedBitSetIsAMaskWithTheBitBoundOfItsEnumeration() {
        Flags v = new Flags();
        v.set.add(Flag.READ);
        v.set.add(Flag.EXEC);
        // @BitBound(8): one octet; READ is bit 0 and EXEC bit 2
        assertArrayEquals(bytes(0x05), encode(v));
        assertEquals(v.set, roundTrip(v).set);
        v.set.clear();
        assertArrayEquals(bytes(0x00), encode(v));
        assertEquals(java.util.EnumSet.noneOf(Flag.class), roundTrip(v).set);
    }

    public static final class WideFlags {
        @org.omg.dds.type.BitSet(elementType = Wide.class)
        public java.util.BitSet bits = new java.util.BitSet();
        @org.omg.dds.type.BitSet
        public Wide one = Wide.C;
    }

    @Test
    void theDefaultBitBoundIs32AndABitSetFieldNamesItsEnumeration() {
        WideFlags v = new WideFlags();
        v.bits.set(1);
        v.bits.set(3);
        // 32 bits: four octets; 0x0a; then the single constant C = bit 2 = 0x04
        assertArrayEquals(bytes(0x0a, 0, 0, 0, 0x04, 0, 0, 0), encode(v));
        WideFlags back = roundTrip(v);
        assertEquals(v.bits, back.bits);
        assertEquals(Wide.C, back.one);
    }

    public static final class UnannotatedBitSetMember {
        public Wide one;
    }

    public static final class NotABitSetEnum {
        @org.omg.dds.type.BitSet
        public java.util.EnumSet<Plain> set;
    }

    public static final class BitSetOnString {
        @org.omg.dds.type.BitSet
        public String text;
    }

    public static final class UnnamedBitSet {
        @org.omg.dds.type.BitSet
        public java.util.BitSet bits;
    }

    @Test
    void aMisplacedBitSetAnnotationIsRefused() {
        for (Class<?> bad : new Class<?>[] {UnannotatedBitSetMember.class, NotABitSetEnum.class,
                BitSetOnString.class, UnnamedBitSet.class}) {
            assertThrows(XcdrException.class, () -> ReflectionTypeSupport.of(bad), bad.getName());
        }
    }

    @Test
    void aBitWithoutAConstantIsRefusedOnDecode() {
        ReflectionTypeSupport<Flags> ts = ReflectionTypeSupport.of(Flags.class);
        assertThrows(XcdrException.class, () -> ts.decode(bytes(0x08)));
    }

    // --- kinds without a Java meaning on a member --------------------------------

    public static final class AliasMember {
        @SerializeAs(TypeKind.ALIAS_TYPE)
        public int value;
    }

    public static final class AnnotationMember {
        @SerializeAs(TypeKind.ANNOTATION_TYPE)
        public int value;
    }

    @Test
    void aliasAndAnnotationAreNotKindsOfAMemberAndTheMessageSaysWhy() {
        XcdrException alias = assertThrows(XcdrException.class,
                () -> ReflectionTypeSupport.of(AliasMember.class));
        assertTrue(alias.getMessage().contains("ALIAS_TYPE"));
        XcdrException annotation = assertThrows(XcdrException.class,
                () -> ReflectionTypeSupport.of(AnnotationMember.class));
        assertTrue(annotation.getMessage().contains("§8.9"));
    }

    public static final class FlagList {
        @org.omg.dds.type.BitSet
        public List<Flag> flags = new ArrayList<>();
        @org.omg.dds.type.BitSet
        public Flag[] array = new Flag[0];
    }

    @Test
    void aCollectionOfBitSetValuesIsASequenceOfBitSets() {
        FlagList v = new FlagList();
        v.flags.add(Flag.READ);
        v.flags.add(Flag.EXEC);
        v.array = new Flag[] {Flag.WRITE};
        // @BitBound(8): one octet per element; sequence length 2, then 01 04; length 1, then 02
        assertArrayEquals(bytes(2, 0, 0, 0, 0x01, 0x04, 0, 0, 1, 0, 0, 0, 0x02), encode(v));
        FlagList back = roundTrip(v);
        assertEquals(v.flags, back.flags);
        assertEquals(1, back.array.length);
        assertEquals(Flag.WRITE, back.array[0]);
    }

    public static final class PlainList {
        @org.omg.dds.type.BitSet
        public List<Plain> flags;
    }

    @Test
    void aCollectionOfNonBitSetValuesMarkedBitSetIsRefused() {
        assertThrows(XcdrException.class, () -> ReflectionTypeSupport.of(PlainList.class));
    }

    @Extensibility(Extensibility.Kind.MUTABLE_EXTENSIBILITY)
    public static final class WithMemberId {
        @org.omg.dds.type.ID(5)
        public int value = 1;
    }

    @Test
    void theIdAnnotationOfThePsmGivesTheMemberIdOfAMutableMember() {
        // DHEADER 12; EMHEADER LC=4 (NEXTINT) and member id 5; NEXTINT 4; the value
        assertArrayEquals(bytes(12, 0, 0, 0, 5, 0, 0, 0x40, 4, 0, 0, 0, 1, 0, 0, 0),
                encode(new WithMemberId()));
    }

    public static final class StringKeyed {
        @org.omg.dds.type.Key
        public String key = "ab";
        public int value;
    }

    @Test
    void aKeyOfUnboundedSizeHashesWithMd5EvenWhenTheHolderIsShort() {
        // RTPS 2.5 §9.6.3.8 step 5: the maximum serialized size of the key holder decides.
        // The holder is the big-endian string: length 3 (with the NUL), 'a', 'b', NUL.
        byte[] hash = ReflectionTypeSupport.of(StringKeyed.class).keyHash(new StringKeyed());
        assertArrayEquals(bytes(0x18, 0x65, 0x94, 0xb7, 0x20, 0x5d, 0x08, 0xac, 0x2f, 0xf8, 0xe1, 0xac, 0x47, 0xfb, 0x4b, 0x2a), hash);
    }
}
