// SPDX-License-Identifier: Apache-2.0
package org.omg.dds;

import org.junit.jupiter.api.Test;
import org.omg.dds.core.ServiceEnvironment;
import org.omg.dds.type.TypeKind;
import org.omg.dds.type.dynamic.AnnotationDescriptor;
import org.omg.dds.type.dynamic.DynamicData;
import org.omg.dds.type.dynamic.DynamicDataFactory;
import org.omg.dds.type.dynamic.DynamicType;
import org.omg.dds.type.dynamic.DynamicTypeFactory;
import org.omg.dds.type.dynamic.MemberDescriptor;
import org.omg.dds.type.typeobject.Member;
import org.zerodds.cdr.DynamicDataTypeSupport;
import org.zerodds.core.PreconditionNotMetException;

import java.math.BigDecimal;
import java.math.BigInteger;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertNotSame;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * DDS-Java-PSM 1.0 §7.8.1.4: {@code DynamicData}. The accessors of every primitive kind, the
 * unsigned kinds through the signed type of the same size (the bit pattern) and the one
 * size up (the logical value), {@code FLOAT_128} as {@code java.math.BigDecimal},
 * {@code clearValue}, {@code clearAllValues}, {@code clearNonkeyValues}, the loan,
 * and {@code equals} and {@code clone} as {@code Object.equals} and {@code Object.clone}
 * with the semantics of XTypes 1.3 §7.5.2.11.
 */
class TypesDynamicDataTest {

    private static final ServiceEnvironment ENV = Qos.env();
    private static final DynamicTypeFactory TYPES = DynamicTypeFactory.getInstance(ENV);
    private static final DynamicDataFactory DATA = DynamicDataFactory.getInstance(ENV);

    private static DynamicType prim(TypeKind kind) {
        return TYPES.getPrimitiveType(kind);
    }

    private static AnnotationDescriptor annotation(String name) {
        DynamicType type = TYPES.createType(TYPES.newTypeDescriptor()
                .withKind(TypeKind.ANNOTATION_TYPE).withName(name));
        return TYPES.newAnnotationDescriptor().withType(type);
    }

    private static MemberDescriptor descriptor(String name, int id, DynamicType type) {
        return TYPES.newMemberDescriptor().withName(name).withId(id).withType(type);
    }

    private static DynamicType struct(String name) {
        return TYPES.createType(TYPES.newTypeDescriptor()
                .withKind(TypeKind.STRUCTURE_TYPE).withName(name));
    }

    /** struct TypesInner { long n; }. */
    private static DynamicType inner() {
        DynamicType type = struct("TypesInner");
        type.addMember(descriptor("n", 1, prim(TypeKind.INT_32_TYPE)));
        return type;
    }

    /**
     * struct TypesWide { @key long id; short s; long long l; float f; double d; char ch;
     * octet by; boolean flag; string name; sequence&lt;long&gt; seq; TypesInner in;
     * @optional long opt; } with the ids 1 to 12.
     */
    private static DynamicType wide() {
        DynamicType type = struct("TypesWide");
        type.addMember(descriptor("id", 1, prim(TypeKind.INT_32_TYPE)))
                .setAnnotation(annotation("Key"));
        type.addMember(descriptor("s", 2, prim(TypeKind.INT_16_TYPE)));
        type.addMember(descriptor("l", 3, prim(TypeKind.INT_64_TYPE)));
        type.addMember(descriptor("f", 4, prim(TypeKind.FLOAT_32_TYPE)));
        type.addMember(descriptor("d", 5, prim(TypeKind.FLOAT_64_TYPE)));
        type.addMember(descriptor("ch", 6, prim(TypeKind.CHAR_8_TYPE)));
        type.addMember(descriptor("by", 7, prim(TypeKind.BYTE_TYPE)));
        type.addMember(descriptor("flag", 8, prim(TypeKind.BOOLEAN_TYPE)));
        type.addMember(descriptor("name", 9, TYPES.createStringType(0)));
        type.addMember(descriptor("seq", 10, TYPES.createSequenceType(prim(TypeKind.INT_32_TYPE))));
        type.addMember(descriptor("in", 11, inner()));
        type.addMember(descriptor("opt", 12, prim(TypeKind.INT_32_TYPE)))
                .setAnnotation(annotation("Optional"));
        return type;
    }

    private static DynamicData innerData(int n) {
        return DATA.newDynamicData(inner()).setInt32Value(1, n);
    }

    // --- The accessors of the primitive kinds --------------------------------

    @Test
    void everyPrimitiveKindIsReadBackWhatWasSet() {
        DynamicType type = wide();
        DynamicData data = DATA.newDynamicData(type)
                .setInt32Value(1, 5)
                .setInt16Value(2, (short) -2)
                .setInt64Value(3, 1L << 40)
                .setFloat32Value(4, 1.5f)
                .setFloat64Value(5, 2.25)
                .setCharValue(6, 'A')
                .setByteValue(7, (byte) 0xfe)
                .setBooleanValue(8, true)
                .setStringValue(9, "text");
        assertEquals(5, data.getInt32Value(1));
        assertEquals((short) -2, data.getInt16Value(2));
        assertEquals(1L << 40, data.getInt64Value(3));
        assertEquals(1.5f, data.getFloat32Value(4));
        assertEquals(2.25, data.getFloat64Value(5));
        assertEquals('A', data.getCharValue(6));
        assertEquals((byte) 0xfe, data.getByteValue(7));
        assertTrue(data.getBooleanValue(8));
        assertEquals("text", data.getStringValue(9));

        // Through the codec: the same values come out of the bytes.
        DynamicDataTypeSupport codec = new DynamicDataTypeSupport(ENV, type);
        DynamicData back = codec.decode(codec.encode(data));
        assertEquals(5, back.getInt32Value(1));
        assertEquals((short) -2, back.getInt16Value(2));
        assertEquals(1L << 40, back.getInt64Value(3));
        assertEquals(1.5f, back.getFloat32Value(4));
        assertEquals(2.25, back.getFloat64Value(5));
        assertEquals('A', back.getCharValue(6));
        assertEquals((byte) 0xfe, back.getByteValue(7));
        assertTrue(back.getBooleanValue(8));
        assertEquals("text", back.getStringValue(9));
    }

    @Test
    void aSmallerSignedKindIsPromotedToTheLargerAccessor() {
        DynamicData data = DATA.newDynamicData(wide())
                .setInt16Value(2, (short) -2)
                .setFloat32Value(4, 1.5f);
        assertEquals(-2, data.getInt32Value(2));
        assertEquals(-2L, data.getInt64Value(2));
        assertEquals(BigInteger.valueOf(-2), data.getBigIntegerValue(2));
        assertEquals(1.5, data.getFloat64Value(4));
        // A larger kind is not narrowed.
        data.setInt32Value(1, 5);
        assertThrows(IllegalArgumentException.class, () -> data.getInt16Value(1));
        // An accessor larger than the member promotes the value stored in it.
        data.setInt64Value(1, 6);
        assertEquals(6, data.getInt32Value(1));
    }

    @Test
    void aValueOutsideTheRangeOfTheMemberIsRefused() {
        DynamicData data = DATA.newDynamicData(wide());
        assertThrows(IllegalArgumentException.class, () -> data.setInt64Value(1, 1L << 40));
        assertThrows(IllegalArgumentException.class, () -> data.setInt32Value(2, 70000));
        data.setInt32Value(2, -32768);
        assertEquals((short) -32768, data.getInt16Value(2));
    }

    // --- Descriptors and member ids ------------------------------------------

    @Test
    void descriptorsAndMemberIdsFollowTheType() {
        DynamicData data = DATA.newDynamicData(wide());
        List<MemberDescriptor> descriptors = data.getDescriptors();
        assertEquals(12, descriptors.size());
        assertEquals("id", descriptors.get(0).getName());
        assertEquals(1, descriptors.get(0).getId());
        assertEquals("opt", descriptors.get(11).getName());
        assertEquals(TypeKind.STRING_TYPE, descriptors.get(8).getType().getKind());

        assertEquals(1, data.getMemberIdByName("id"));
        assertEquals(9, data.getMemberIdByName("name"));
        assertEquals(12, data.getMemberIdByName("opt"));
        assertEquals(1, data.getMemberIdAtIndex(0));
        assertEquals(9, data.getMemberIdAtIndex(8));
        assertEquals(12, data.getMemberIdAtIndex(11));
        assertThrows(PreconditionNotMetException.class, () -> data.getMemberIdByName("missing"));
        assertThrows(PreconditionNotMetException.class, () -> data.getMemberIdAtIndex(12));
        assertThrows(PreconditionNotMetException.class, () -> data.getMemberIdAtIndex(-1));
        assertThrows(IllegalArgumentException.class, () -> data.getMemberIdByName(null));
        assertSame(data.getType(), data.getType());
    }

    @Test
    void membersByNameAreTheMembersById() {
        DynamicData data = DATA.newDynamicData(wide());
        data.setValue("name", "by name", String.class);
        assertEquals("by name", data.getStringValue(9));
        assertEquals("by name", data.getValue("name", String.class));
        assertEquals("by name", data.getValue(9, String.class));
        data.setValue(1, 77, Integer.class);
        assertEquals(77, data.getValue("id", Integer.class));
        data.setComplexValue(11, innerData(3));
        assertEquals(3, data.getComplexValue("in").getInt32Value(1));
    }

    // --- Clearing values (XTypes 1.3 §7.5.2.11.3) -------------------------

    @Test
    void clearingANonOptionalMemberSetsItToItsDefault() {
        DynamicData data = DATA.newDynamicData(wide())
                .setInt32Value(1, 5).setInt16Value(2, (short) 3).setStringValue(9, "x")
                .setBooleanValue(8, true).setFloat64Value(5, 2.5)
                .setInt32Values(10, new int[] {1, 2, 3}, 0, 3);
        data.clearValue(1);
        data.clearValue("s");
        data.clearValue(9);
        data.clearValue(8);
        data.clearValue(5);
        data.clearValue(10);
        assertEquals(0, data.getInt32Value(1));
        assertEquals((short) 0, data.getInt16Value(2));
        assertEquals("", data.getStringValue(9));
        assertEquals(false, data.getBooleanValue(8));
        assertEquals(0.0, data.getFloat64Value(5));
        assertEquals(0, data.getInt32Values(new int[3], 0, 3, 10), "an empty sequence");
    }

    @Test
    void clearingAnOptionalMemberRemovesIt() {
        DynamicData data = DATA.newDynamicData(wide()).setInt32Value(12, 9);
        assertEquals(9, data.getInt32Value(12));
        data.clearValue(12);
        assertThrows(PreconditionNotMetException.class, () -> data.getInt32Value(12));
    }

    @Test
    void clearAllValuesClearsEveryMemberAndClearNonkeyValuesKeepsTheKey() {
        DynamicType type = wide();
        DynamicData data = DATA.newDynamicData(type)
                .setInt32Value(1, 5).setInt16Value(2, (short) 3).setStringValue(9, "x")
                .setInt32Value(12, 9);
        data.clearNonkeyValues();
        assertEquals(5, data.getInt32Value(1), "the value of the key member is retained");
        assertEquals((short) 0, data.getInt16Value(2));
        assertEquals("", data.getStringValue(9));
        assertThrows(PreconditionNotMetException.class, () -> data.getInt32Value(12));

        data.setInt16Value(2, (short) 4);
        data.clearAllValues();
        assertEquals(0, data.getInt32Value(1), "clearAllValues clears the key too");
        assertEquals((short) 0, data.getInt16Value(2));
        assertThrows(PreconditionNotMetException.class, () -> data.getInt32Value(12));
    }

    // --- The loan (XTypes 1.3 §7.5.2.11.7) ----------------------------------

    @Test
    void aLoanedStructureIsTheNestedValueAndChangesToItAreChangesToTheSample() {
        DynamicData data = DATA.newDynamicData(wide());
        DynamicData nested = innerData(1);
        data.setComplexValue(11, nested);

        DynamicData loan = data.loanValue(11);
        assertSame(nested, loan, "lent without a copy");
        loan.setInt32Value(1, 42);
        assertEquals(42, data.getComplexValue(11).getInt32Value(1));

        // One loan at a time: another loan before the return gives nothing.
        assertNull(data.loanValue(11));
        assertNull(data.loanValue(1));
        data.returnLoanedValue(loan);
        assertSame(nested, data.loanValue(11));
    }

    @Test
    void theLoanIsReturnedOnlyOnceAndOnlyBySampleItCameFrom() {
        DynamicData data = DATA.newDynamicData(wide());
        data.setComplexValue(11, innerData(1));
        DynamicData other = DATA.newDynamicData(wide());
        other.setComplexValue(11, innerData(2));

        // Nothing is lent yet.
        assertThrows(PreconditionNotMetException.class, () -> data.returnLoanedValue(innerData(1)));
        DynamicData loan = data.loanValue(11);
        // A value that is not the outstanding loan of this sample is refused.
        assertThrows(PreconditionNotMetException.class, () -> data.returnLoanedValue(other.getComplexValue(11)));
        data.returnLoanedValue(loan);
        assertThrows(PreconditionNotMetException.class, () -> data.returnLoanedValue(loan));
    }

    @Test
    void aValueThatDoesNotExistIsNotLoaned() {
        DynamicData data = DATA.newDynamicData(wide());
        assertNull(data.loanValue(11));
        assertNull(data.loanValue(1));
        assertNull(data.loanValue(99));
        // The failed attempts did not take the loan.
        data.setComplexValue(11, innerData(1));
        assertNotNull(data.loanValue(11));
    }

    @Test
    void aLoanedPrimitiveIsWrittenBackOnReturn() {
        DynamicData data = DATA.newDynamicData(wide()).setInt32Value(1, 5);
        DynamicData loan = data.loanValue(1);
        assertEquals(TypeKind.INT_32_TYPE, loan.getType().getKind());
        // The value of an object of a primitive type is addressed by MEMBER_ID_INVALID.
        assertEquals(5, loan.getInt32Value(Member.MEMBER_ID_INVALID));
        loan.setInt32Value(Member.MEMBER_ID_INVALID, 6);
        assertEquals(5, data.getInt32Value(1), "not changed before the loan is returned");
        data.returnLoanedValue(loan);
        assertEquals(6, data.getInt32Value(1));
    }

    // --- clone and equals (XTypes 1.3 §7.5.2.11.4 and §7.5.2.11.5) ----------

    @Test
    void cloneIsAnEqualSampleThatSharesNoValue() {
        DynamicType type = wide();
        DynamicData data = DATA.newDynamicData(type)
                .setInt32Value(1, 5).setStringValue(9, "x")
                .setInt32Values(10, new int[] {1, 2, 3}, 0, 3)
                .setComplexValue(11, innerData(7));
        DynamicData copy = data.clone();
        assertNotSame(data, copy);
        assertEquals(data, copy);
        assertEquals(data.hashCode(), copy.hashCode());
        assertSame(data.getType(), copy.getType());

        // The nested value is a copy: a change to the original's is not seen in the clone.
        data.getComplexValue(11).setInt32Value(1, 99);
        assertEquals(7, copy.getComplexValue(11).getInt32Value(1));
        assertNotEquals(data, copy);
        // And the other way round, and the scalar and sequence values.
        copy.getComplexValue(11).setInt32Value(1, 99);
        assertEquals(data, copy);
        copy.setInt32Values(10, new int[] {9}, 0, 1);
        copy.setStringValue(9, "y");
        int[] seq = new int[3];
        assertEquals(3, data.getInt32Values(seq, 0, 3, 10));
        assertArrayEquals(new int[] {1, 2, 3}, seq);
        assertEquals("x", data.getStringValue(9));
    }

    @Test
    void aClonedArrayValueIsACopy() {
        DynamicData data = DATA.newDynamicData(wide())
                .setInt32Values(10, new int[] {1, 2, 3}, 0, 3);
        DynamicData copy = data.clone();
        // getValue hands out the stored array; changing it changes that sample only.
        int[] stored = data.getValue(10, int[].class);
        stored[0] = 100;
        assertArrayEquals(new int[] {1, 2, 3}, copy.getValue(10, int[].class));
        assertNotEquals(data, copy);
    }

    @Test
    void sampleAreEqualWhenTypeAndValuesAreWithoutRegardToTheOrderOfTheMembers() {
        DynamicType type = wide();
        DynamicData a = DATA.newDynamicData(type).setInt32Value(1, 5).setStringValue(9, "x");
        DynamicData b = DATA.newDynamicData(type).setStringValue(9, "x").setInt32Value(1, 5);
        assertEquals(a, b);
        assertEquals(b, a);
        assertEquals(a.hashCode(), b.hashCode());
        assertEquals(a, a);
        assertNotEquals(a, null);
        assertNotEquals(a, "x");

        b.setStringValue(9, "y");
        assertNotEquals(a, b);
        // Another type, whatever the values.
        assertNotEquals(DATA.newDynamicData(inner()), DATA.newDynamicData(wide()));
        // Equal types that are not the same object compare equal.
        assertEquals(DATA.newDynamicData(wide()).setInt32Value(1, 1),
                DATA.newDynamicData(wide()).setInt32Value(1, 1));
    }

    @Test
    void aNonOptionalMemberOmittedByOneSampleEqualsItsDefaultInTheOther() {
        DynamicType type = wide();
        DynamicData omitted = DATA.newDynamicData(type).setInt32Value(1, 5);
        DynamicData withDefaults = DATA.newDynamicData(type).setInt32Value(1, 5)
                .setInt16Value(2, (short) 0).setStringValue(9, "").setBooleanValue(8, false)
                .setInt32Values(10, new int[0], 0, 0);
        assertEquals(omitted, withDefaults);
        assertEquals(withDefaults, omitted);
        assertEquals(omitted.hashCode(), withDefaults.hashCode());

        // A value that is not the default is a difference.
        assertNotEquals(omitted, DATA.newDynamicData(type).setInt32Value(1, 5)
                .setInt16Value(2, (short) 1));
        // An optional member that is present is a difference even with the value zero.
        assertNotEquals(omitted, DATA.newDynamicData(type).setInt32Value(1, 5).setInt32Value(12, 0));
    }

    @Test
    void nestedValuesAreComparedByValue() {
        DynamicType type = wide();
        DynamicData a = DATA.newDynamicData(type).setComplexValue(11, innerData(1));
        DynamicData b = DATA.newDynamicData(type).setComplexValue(11, innerData(1));
        assertEquals(a, b);
        assertEquals(a.hashCode(), b.hashCode());
        b.getComplexValue(11).setInt32Value(1, 2);
        assertNotEquals(a, b);
    }

    // --- §7.8.1.4 Unsigned kinds ---------------------------------------------

    /** struct TypesUnsigned { unsigned short u16; unsigned long u32; unsigned long long u64; }. */
    private static DynamicType unsigned() {
        DynamicType type = struct("TypesUnsigned");
        type.addMember(descriptor("u16", 1, prim(TypeKind.UINT_16_TYPE)));
        type.addMember(descriptor("u32", 2, prim(TypeKind.UINT_32_TYPE)));
        type.addMember(descriptor("u64", 3, prim(TypeKind.UINT_64_TYPE)));
        return type;
    }

    private static final BigInteger TWO_POW_64 = BigInteger.ONE.shiftLeft(64);

    @Test
    void anUnsignedKindIsReadAsTheSignedKindOfTheSameSizeAndAsTheOneSizeUp() {
        DynamicData data = DATA.newDynamicData(unsigned())
                .setInt16Value(1, (short) -1)       // UInt16 65535, as its Int16 bit pattern
                .setInt32Value(2, -1)               // UInt32 4294967295, as its Int32 pattern
                .setInt64Value(3, -1L);             // UInt64 2^64-1, as its Int64 pattern
        // The signed type of the same size keeps the bit pattern.
        assertEquals((short) -1, data.getInt16Value(1));
        assertEquals(-1, data.getInt32Value(2));
        assertEquals(-1L, data.getInt64Value(3));
        // The signed type one size up keeps the logical value.
        assertEquals(65535, data.getInt32Value(1));
        assertEquals(65535L, data.getInt64Value(1));
        assertEquals(4294967295L, data.getInt64Value(2));
        assertEquals(BigInteger.valueOf(65535), data.getBigIntegerValue(1));
        assertEquals(BigInteger.valueOf(4294967295L), data.getBigIntegerValue(2));
        assertEquals(TWO_POW_64.subtract(BigInteger.ONE), data.getBigIntegerValue(3));
    }

    @Test
    void anUnsignedKindIsSetThroughTheOneSizeUpAsALogicalValue() {
        DynamicData data = DATA.newDynamicData(unsigned());
        data.setInt32Value(1, 65535);
        assertEquals((short) -1, data.getInt16Value(1));
        data.setInt64Value(2, 4294967295L);
        assertEquals(-1, data.getInt32Value(2));
        data.setBigIntegerValue(3, TWO_POW_64.subtract(BigInteger.ONE));
        assertEquals(-1L, data.getInt64Value(3));
        data.setBigIntegerValue(3, BigInteger.valueOf(5));
        assertEquals(5L, data.getInt64Value(3));
        assertEquals(BigInteger.valueOf(5), data.getBigIntegerValue(3));

        // The range check of the kind, which a logical value needs.
        assertThrows(IllegalArgumentException.class, () -> data.setInt32Value(1, 65536));
        assertThrows(IllegalArgumentException.class, () -> data.setInt32Value(1, -1));
        assertThrows(IllegalArgumentException.class, () -> data.setInt64Value(2, 4294967296L));
        assertThrows(IllegalArgumentException.class, () -> data.setInt64Value(2, -1L));
        assertThrows(IllegalArgumentException.class, () -> data.setBigIntegerValue(3, TWO_POW_64));
        assertThrows(IllegalArgumentException.class,
                () -> data.setBigIntegerValue(3, BigInteger.valueOf(-1)));
    }

    @Test
    void unsignedMembersAreEncodedAndDecodedWithTheirFullRange() {
        DynamicType type = unsigned();
        DynamicData data = DATA.newDynamicData(type)
                .setInt32Value(1, 65535)
                .setInt64Value(2, 4294967295L)
                .setBigIntegerValue(3, TWO_POW_64.subtract(BigInteger.ONE));
        DynamicDataTypeSupport codec = new DynamicDataTypeSupport(ENV, type);
        byte[] wire = codec.encode(data);
        // APPENDABLE: the DHEADER 16, u16 at offset 4 (FF FF), two padding octets, u32 at 8,
        // u64 at 12 (the 8-octet alignment is capped to 4 by XCDR2).
        assertArrayEquals(new byte[] {
                16, 0, 0, 0,
                -1, -1, 0, 0,
                -1, -1, -1, -1,
                -1, -1, -1, -1, -1, -1, -1, -1}, wire);

        DynamicData back = codec.decode(wire);
        assertEquals(65535, back.getInt32Value(1));
        assertEquals(4294967295L, back.getInt64Value(2));
        assertEquals(TWO_POW_64.subtract(BigInteger.ONE), back.getBigIntegerValue(3));
        assertEquals((short) -1, back.getInt16Value(1));
        assertEquals(-1, back.getInt32Value(2));
        assertEquals(-1L, back.getInt64Value(3));
        assertEquals(data, back);
    }

    @Test
    void aSequenceOfUnsigned64IsReadAsBigIntegers() {
        DynamicType type = struct("TypesUnsignedSeq");
        type.addMember(descriptor("v", 1, TYPES.createSequenceType(prim(TypeKind.UINT_64_TYPE))));
        BigInteger max = TWO_POW_64.subtract(BigInteger.ONE);
        DynamicData data = DATA.newDynamicData(type)
                .setBigIntegerValues(1, Arrays.asList(max, BigInteger.valueOf(5)));

        DynamicDataTypeSupport codec = new DynamicDataTypeSupport(ENV, type);
        DynamicData back = codec.decode(codec.encode(data));
        assertEquals(Arrays.asList(max, BigInteger.valueOf(5)),
                back.getBigIntegerValues(new ArrayList<BigInteger>(), 1));
        long[] bits = new long[2];
        assertEquals(2, back.getInt64Values(bits, 0, 2, 1));
        assertArrayEquals(new long[] {-1L, 5L}, bits);
        assertThrows(IllegalArgumentException.class, () -> DATA.newDynamicData(type)
                .setBigIntegerValues(1, Arrays.asList(TWO_POW_64)));
    }

    // --- §7.8.1.4 Float128 as java.math.BigDecimal --------------------------

    /** struct TypesQuad { long double q; }. */
    private static DynamicType quad() {
        DynamicType type = struct("TypesQuad");
        type.addMember(descriptor("q", 1, prim(TypeKind.FLOAT_128_TYPE)));
        return type;
    }

    @Test
    void aFloat128MemberIsABigDecimalOnTheWireAndBack() {
        DynamicType type = quad();
        DynamicData data = DATA.newDynamicData(type).setBigDecimalValue(1, new BigDecimal("1.5"));
        DynamicDataTypeSupport codec = new DynamicDataTypeSupport(ENV, type);
        byte[] wire = codec.encode(data);
        // IEEE 754 binary128 of 1.5: sign 0, exponent 0x3FFF, fraction 0x8000...0, so the
        // big-endian octets are 3F FF 80 00 ..., and the little-endian stream has them
        // reversed. APPENDABLE: a DHEADER of 16 before the member at offset 4.
        byte[] expected = new byte[20];
        expected[0] = 16;
        expected[4 + 13] = (byte) 0x80;
        expected[4 + 14] = (byte) 0xff;
        expected[4 + 15] = 0x3f;
        assertArrayEquals(expected, wire);

        DynamicData back = codec.decode(wire);
        assertEquals(0, new BigDecimal("1.5").compareTo(back.getBigDecimalValue(1)));
        assertEquals(data, back);
    }

    @Test
    void aFloat128MemberKeepsADigitStringADoubleCannotHold() {
        // 2^100 + 1 has 101 significant bits: exact in a binary128 (113 bits), not in a double.
        BigDecimal value = new BigDecimal(BigInteger.ONE.shiftLeft(100).add(BigInteger.ONE));
        assertNotEquals(0, value.compareTo(new BigDecimal(value.doubleValue())));
        DynamicType type = quad();
        DynamicDataTypeSupport codec = new DynamicDataTypeSupport(ENV, type);
        DynamicData back = codec.decode(codec.encode(
                DATA.newDynamicData(type).setBigDecimalValue(1, value)));
        assertEquals(0, value.compareTo(back.getBigDecimalValue(1)));
        assertEquals("1267650600228229401496703205377",
                back.getBigDecimalValue(1).toBigInteger().toString());
        // Zero and a negative value.
        assertEquals(0, BigDecimal.ZERO.compareTo(codec.decode(codec.encode(
                DATA.newDynamicData(type).setBigDecimalValue(1, BigDecimal.ZERO)))
                .getBigDecimalValue(1)));
        assertEquals(0, new BigDecimal("-0.25").compareTo(codec.decode(codec.encode(
                DATA.newDynamicData(type).setBigDecimalValue(1, new BigDecimal("-0.25"))))
                .getBigDecimalValue(1)));
    }

    @Test
    void samplesWithTheSameFloat128ValueAreEqualWhateverTheScale() {
        DynamicType type = quad();
        assertEquals(DATA.newDynamicData(type).setBigDecimalValue(1, new BigDecimal("1.5")),
                DATA.newDynamicData(type).setBigDecimalValue(1, new BigDecimal("1.50")));
        assertEquals(DATA.newDynamicData(type).setBigDecimalValue(1, new BigDecimal("1.5")).hashCode(),
                DATA.newDynamicData(type).setBigDecimalValue(1, new BigDecimal("1.50")).hashCode());
    }
}
