// SPDX-License-Identifier: Apache-2.0
package org.zerodds.cdr;

import org.omg.dds.type.SerializeAs;
import org.omg.dds.type.TypeKind;
import org.zerodds.wire.Float128;
import org.zerodds.wire.Reader;
import org.zerodds.wire.WstringLengthPolicy;
import org.zerodds.wire.WstringUnitOrder;
import org.zerodds.wire.Writer;

import java.lang.reflect.Array;
import java.lang.reflect.Type;
import java.math.BigDecimal;
import java.math.BigInteger;
import java.util.ArrayList;
import java.util.Collection;
import java.util.List;
import java.util.Map;

/**
 * What {@code @SerializeAs} (DDS-Java-PSM 1.0 §8.2) says about one Java
 * member, and the wire form that results: the DDS kind a type designer
 * chose in place of the default of Table 8.1.
 *
 * <p>Table 8.2 lists which Java types each DDS primitive kind accepts. The
 * annotation is checked against the Java type of the member when the type is
 * first introspected, and a combination Table 8.2 does not list is an
 * {@link XcdrException} at that point (not a silent default at the first
 * write):
 *
 * <pre>
 * BOOLEAN boolean | BYTE byte | INT_16 short | UINT_16 short, int
 * INT_32 int | UINT_32 int, long | INT_64 long | UINT_64 long, BigInteger
 * FLOAT_32 float | FLOAT_64 double | FLOAT_128 double, BigDecimal
 * CHAR_8 char | CHAR_32 char, int
 * </pre>
 *
 * (and the boxed types). An unsigned kind held in the signed type of the
 * same size keeps the bit pattern (§8.4, "preserve representation"); held in
 * the next larger type it keeps the logical value, and a value outside the
 * range of the kind is an {@link XcdrException}.
 *
 * <p>For collections: {@code STRING_TYPE} with
 * {@code collectionElementKind = CHAR_32_TYPE} is a wide string (§8.5.1);
 * {@code SEQUENCE_TYPE}, {@code ARRAY_TYPE} (fixed length from {@code bound},
 * several dimensions for a nested Java array) and {@code MAP_TYPE} may name
 * the kind of their elements and keys; {@code bound} bounds strings,
 * sequences and maps, and gives the dimensions of an array.
 */
final class JavaRepresentation {

    final TypeKind kind;
    final TypeKind element;
    final TypeKind key;
    final long[] bound;
    /**
     * For a bit set (§8.7) that is made of the constants of an enumeration: that
     * enumeration, whose constant with ordinal n is bit n; otherwise
     * {@code null}.
     */
    Class<?> bitEnum;
    /** For a sequence or array of bit set values (§8.7): the bit set enumeration of the elements. */
    Class<?> elementBitEnum;

    JavaRepresentation(TypeKind kind, TypeKind element, TypeKind key, long[] bound) {
        this.kind = kind;
        this.element = element;
        this.key = key;
        this.bound = bound;
    }

    /** The representation @SerializeAs states, or {@code null} for no annotation. */
    static JavaRepresentation of(SerializeAs annotation, Class<?> cls, String where) {
        if (annotation == null) {
            return null;
        }
        JavaRepresentation rep = new JavaRepresentation(annotation.value(),
                annotation.collectionElementKind(), annotation.mapKeyElementKind(),
                annotation.bound());
        rep.check(cls, where);
        return rep;
    }

    /**
     * The representation of a field: what {@code @SerializeAs} states
     * (§8.2), or, for a field marked {@code @BitSet} (§8.7), the bit set of
     * the constants of an enumeration. {@code null} for the defaults of
     * Table 8.1.
     *
     * <p>§8.7: "By default, any Java enumeration class will be considered to be
     * a DDS enumeration. ... a type that is syntactically an enumeration may be
     * annotated as a bit set type. In this case, objects of these types must
     * also be annotated in order to be serialized correctly. A type member of
     * type java.util.EnumSet or java.util.BitSet will be serialized as a bit set
     * if marked with @BitSet." A bit set type is an enumeration annotated
     * {@code @BitSet}; constant n (its ordinal) is bit n, and
     * {@code @BitBound} (default 32) is the bit bound that gives the width on
     * the wire (XTypes 1.3 §7.4.3.4.2: 1 to 8 bits one octet, up to 16 two, up
     * to 32 four, up to 64 eight). A field of the bit set enumeration itself
     * holds one constant (one bit), an {@code EnumSet} or {@code BitSet} field
     * any number. An unannotated field of a bit set type, and bits on the wire
     * that name no constant, are refused.
     */
    static JavaRepresentation ofField(java.lang.reflect.Field field, String where) {
        SerializeAs serializeAs = field.getAnnotation(SerializeAs.class);
        org.omg.dds.type.BitSet bits = field.getAnnotation(org.omg.dds.type.BitSet.class);
        Class<?> cls = field.getType();
        if (bits == null) {
            if (isBitSetType(cls)) {
                throw new XcdrException(where + ": " + cls.getName() + " is a bit set type"
                        + " (@BitSet on the enumeration) and a member of it must be annotated"
                        + " @BitSet too (DDS-Java-PSM 1.0 §8.7: \"objects of these types must"
                        + " also be annotated in order to be serialized correctly\")");
            }
            return of(serializeAs, cls, where);
        }
        if (serializeAs != null) {
            throw new XcdrException(where + ": @BitSet and @SerializeAs on one member; the bit set"
                    + " is already a bit set kind");
        }
        Class<?> elements = collectionElementType(field);
        if (elements != null) {
            // A collection or array of bit set values: each element is one constant of the
            // bit set enumeration, written as its bit set; the collection is a sequence.
            if (!isBitSetType(elements)) {
                throw new XcdrException(where + ": @BitSet on a collection of "
                        + elements.getName() + ", which is not an enumeration annotated @BitSet"
                        + " (§8.7)");
            }
            JavaRepresentation seq = new JavaRepresentation(TypeKind.SEQUENCE_TYPE,
                    TypeKind.BITSET_TYPE, TypeKind.NO_TYPE, new long[0]);
            seq.elementBitEnum = elements;
            bitBound(elements);
            return seq;
        }
        Class<?> constants;
        Class<?> named = bits.elementType();
        if (cls.isEnum()) {
            constants = cls;
            if (named != Enum.class && named != cls) {
                throw new XcdrException(where + ": @BitSet(elementType = " + named.getName()
                        + ") on a member of " + cls.getName() + "; it is that enumeration or"
                        + " unset (§8.7)");
            }
        } else if (java.util.EnumSet.class.isAssignableFrom(cls)) {
            constants = named != Enum.class ? named : typeArgument(field.getGenericType());
        } else if (cls == java.util.BitSet.class) {
            constants = named != Enum.class ? named : null;
        } else {
            throw new XcdrException(where + ": @BitSet is for an enumeration that is a bit set"
                    + " type, java.util.EnumSet and java.util.BitSet (§8.7), not for "
                    + cls.getName());
        }
        if (constants == null) {
            throw new XcdrException(where + ": a " + cls.getName() + " marked @BitSet names the"
                    + " enumeration that defines its bits with @BitSet(elementType = ...) (§8.7)");
        }
        if (!isBitSetType(constants)) {
            throw new XcdrException(where + ": " + constants.getName() + " is not an enumeration"
                    + " annotated @BitSet (§8.7: \"a type that is syntactically an enumeration may"
                    + " be annotated as a bit set type\")");
        }
        org.omg.dds.type.BitSet onType = constants.getAnnotation(org.omg.dds.type.BitSet.class);
        if (onType.elementType() != Enum.class && onType.elementType() != constants) {
            throw new XcdrException(constants.getName() + ": @BitSet(elementType = "
                    + onType.elementType().getName() + ") on the enumeration; it is the"
                    + " enumeration itself or unset (§8.7)");
        }
        int bound = bitBound(constants);
        JavaRepresentation rep = new JavaRepresentation(TypeKind.BITSET_TYPE, TypeKind.NO_TYPE,
                TypeKind.NO_TYPE, new long[] {bound});
        rep.bitEnum = constants;
        return rep;
    }

    /** The element class of a {@code Collection<E>} (other than EnumSet) or {@code E[]} field, or null. */
    private static Class<?> collectionElementType(java.lang.reflect.Field field) {
        Class<?> cls = field.getType();
        if (cls.isArray()) {
            return cls.getComponentType();
        }
        if (java.util.Collection.class.isAssignableFrom(cls)
                && !java.util.EnumSet.class.isAssignableFrom(cls)) {
            return typeArgument(field.getGenericType());
        }
        return null;
    }

    /** An enumeration annotated {@code @BitSet} (§8.7). */
    static boolean isBitSetType(Class<?> cls) {
        return cls.isEnum() && cls.isAnnotationPresent(org.omg.dds.type.BitSet.class);
    }

    /** The bit bound of a bit set type: {@code @BitBound}, default 32, 1 to 64, at least its constants. */
    static int bitBound(Class<?> bitSetType) {
        org.omg.dds.type.BitBound annotation = bitSetType.getAnnotation(org.omg.dds.type.BitBound.class);
        int bound = annotation == null ? 32 : annotation.value();
        int constants = bitSetType.getEnumConstants().length;
        if (bound < 1 || bound > 64) {
            throw new XcdrException(bitSetType.getName() + ": @BitBound(" + bound
                    + "); a bit bound is 1 to 64");
        }
        if (constants > bound) {
            throw new XcdrException(bitSetType.getName() + " has " + constants
                    + " constants and the bit bound is " + bound);
        }
        return bound;
    }

    private static Class<?> typeArgument(Type generic) {
        if (generic instanceof java.lang.reflect.ParameterizedType) {
            Type[] args = ((java.lang.reflect.ParameterizedType) generic).getActualTypeArguments();
            if (args.length == 1 && args[0] instanceof Class) {
                return (Class<?>) args[0];
            }
        }
        return null;
    }

    private long bitsOfConstants(Object value) {
        long bits = 0;
        if (value instanceof Enum) {
            bits = 1L << ((Enum<?>) value).ordinal();
        } else {
            for (Object constant : (Collection<?>) value) {
                bits |= 1L << ((Enum<?>) constant).ordinal();
            }
        }
        return bits;
    }

    // The EnumSet of a Class<?> that is known to be an enumeration: the one unchecked
    // conversion, which the runtime type of the elements makes safe.
    @SuppressWarnings({"unchecked", "rawtypes"})
    private Object constantsOfBits(long bits, Class<?> cls) {
        Object[] constants = bitEnum.getEnumConstants();
        long known = 0;
        for (int i = 0; i < constants.length; i++) {
            known |= 1L << i;
        }
        if ((bits & ~known) != 0) {
            throw new XcdrException(Long.toHexString(bits) + " has bits that no constant of "
                    + bitEnum.getName() + " names");
        }
        if (cls.isEnum()) {
            if (Long.bitCount(bits) != 1) {
                throw new XcdrException(Long.toHexString(bits) + " is not one bit and a "
                        + cls.getName() + " is one constant");
            }
            return constants[Long.numberOfTrailingZeros(bits)];
        }
        if (cls == java.util.BitSet.class) {
            return java.util.BitSet.valueOf(new long[] {bits});
        }
        java.util.EnumSet set = java.util.EnumSet.noneOf((Class) bitEnum);
        for (int i = 0; i < constants.length; i++) {
            if ((bits & (1L << i)) != 0) {
                set.add(constants[i]);
            }
        }
        return set;
    }

    /** The representation of the elements of a collection, or {@code null} for the default. */
    JavaRepresentation elementRep() {
        if (kind == TypeKind.STRING_TYPE || element == TypeKind.NO_TYPE) {
            return null;
        }
        if (elementBitEnum != null) {
            JavaRepresentation rep = new JavaRepresentation(TypeKind.BITSET_TYPE,
                    TypeKind.NO_TYPE, TypeKind.NO_TYPE, new long[] {bitBound(elementBitEnum)});
            rep.bitEnum = elementBitEnum;
            return rep;
        }
        return new JavaRepresentation(element, TypeKind.NO_TYPE, TypeKind.NO_TYPE, new long[0]);
    }

    JavaRepresentation keyRep() {
        return key == TypeKind.NO_TYPE ? null
                : new JavaRepresentation(key, TypeKind.NO_TYPE, TypeKind.NO_TYPE, new long[0]);
    }

    // ------------------------------------------------------------------
    // Checking against Table 8.2
    // ------------------------------------------------------------------

    private static boolean one(Class<?> cls, Class<?>... allowed) {
        for (Class<?> a : allowed) {
            if (cls == a) {
                return true;
            }
        }
        return false;
    }

    private void check(Class<?> cls, String where) {
        boolean ok;
        String permitted;
        switch (kind) {
            case BOOLEAN_TYPE:
                ok = one(cls, boolean.class, Boolean.class);
                permitted = "boolean";
                break;
            case BYTE_TYPE:
                ok = one(cls, byte.class, Byte.class);
                permitted = "byte";
                break;
            case INT_16_TYPE:
                ok = one(cls, short.class, Short.class);
                permitted = "short";
                break;
            case UINT_16_TYPE:
                ok = one(cls, short.class, Short.class, int.class, Integer.class);
                permitted = "short, int";
                break;
            case INT_32_TYPE:
                ok = one(cls, int.class, Integer.class);
                permitted = "int";
                break;
            case UINT_32_TYPE:
                ok = one(cls, int.class, Integer.class, long.class, Long.class);
                permitted = "int, long";
                break;
            case INT_64_TYPE:
                ok = one(cls, long.class, Long.class);
                permitted = "long";
                break;
            case UINT_64_TYPE:
                ok = one(cls, long.class, Long.class, BigInteger.class);
                permitted = "long, java.math.BigInteger";
                break;
            case FLOAT_32_TYPE:
                ok = one(cls, float.class, Float.class);
                permitted = "float";
                break;
            case FLOAT_64_TYPE:
                ok = one(cls, double.class, Double.class);
                permitted = "double";
                break;
            case FLOAT_128_TYPE:
                ok = one(cls, double.class, Double.class, BigDecimal.class);
                permitted = "double, java.math.BigDecimal";
                break;
            case CHAR_8_TYPE:
                ok = one(cls, char.class, Character.class);
                permitted = "char";
                break;
            case CHAR_32_TYPE:
                ok = one(cls, char.class, Character.class, int.class, Integer.class);
                permitted = "char, int";
                break;
            case ENUMERATION_TYPE:
                ok = cls.isEnum();
                permitted = "an enum";
                break;
            case STRING_TYPE:
                ok = cls == String.class;
                permitted = "java.lang.String";
                if (ok && element != TypeKind.NO_TYPE && element != TypeKind.CHAR_8_TYPE
                        && element != TypeKind.CHAR_32_TYPE) {
                    throw new XcdrException(where + ": a string has elements of kind CHAR_8_TYPE"
                            + " or CHAR_32_TYPE, not " + element);
                }
                if (ok && bound.length > 1) {
                    throw new XcdrException(where + ": a string has one bound");
                }
                break;
            case SEQUENCE_TYPE:
                ok = Collection.class.isAssignableFrom(cls) || cls.isArray();
                permitted = "a java.util.Collection or an array";
                if (ok && bound.length > 1) {
                    throw new XcdrException(where + ": a sequence has one bound");
                }
                break;
            case ARRAY_TYPE:
                ok = Collection.class.isAssignableFrom(cls) || cls.isArray();
                permitted = "a java.util.Collection or an array";
                if (ok && bound.length == 0) {
                    throw new XcdrException(where + ": an array needs its dimensions in"
                            + " @SerializeAs(bound = ...)");
                }
                break;
            case MAP_TYPE:
                ok = Map.class.isAssignableFrom(cls);
                permitted = "a java.util.Map";
                if (ok && bound.length > 1) {
                    throw new XcdrException(where + ": a map has one bound");
                }
                break;
            case BITSET_TYPE:
                ok = one(cls, long.class, Long.class, int.class, Integer.class, short.class,
                        Short.class, byte.class, Byte.class, java.util.BitSet.class);
                permitted = "long, int, short, byte or java.util.BitSet";
                if (ok && bound.length != 1) {
                    throw new XcdrException(where + ": a bit set has exactly one bound");
                }
                break;
            case STRUCTURE_TYPE:
            case UNION_TYPE:
                ok = !cls.isPrimitive() && !cls.isArray() && !cls.isEnum()
                        && !Collection.class.isAssignableFrom(cls)
                        && !Map.class.isAssignableFrom(cls) && cls != String.class;
                permitted = "a class";
                break;
            case ALIAS_TYPE:
                throw new XcdrException(where + ": @SerializeAs(ALIAS_TYPE) on a member has no Java"
                        + " representation: DDS-Java-PSM 1.0 Clause 8 gives a Java type to the"
                        + " primitive kinds (Table 8.2), strings (§8.5.1), maps (§8.5.2),"
                        + " sequences and arrays (§8.5.3), structures (§8.6.1), unions (§8.6.2)"
                        + " and bit sets (§8.7), and no sentence says which Java type an alias"
                        + " stands for; a member has the wire form of the type it is declared as");
            case ANNOTATION_TYPE:
                throw new XcdrException(where + ": @SerializeAs(ANNOTATION_TYPE) is not a kind of a"
                        + " member: §8.9 applies it to a Java annotation type (\"Java annotations"
                        + " that are intended to be represented explicitly within the DDS Type"
                        + " System must be so annotated with @SerializeAs\"), which has no data"
                        + " to put on the wire");
            default:
                throw new XcdrException(where + ": @SerializeAs(" + kind + ") is not a kind of"
                        + " a member");
        }
        if (!ok) {
            throw new XcdrException(where + ": @SerializeAs(" + kind + ") does not apply to a "
                    + cls.getName() + " (DDS-Java-PSM 1.0 Table 8.2 permits " + permitted + ")");
        }
    }

    // ------------------------------------------------------------------
    // Kinds written without a DHEADER around each element
    // ------------------------------------------------------------------

    static boolean primitiveKind(TypeKind k) {
        switch (k) {
            case BOOLEAN_TYPE:
            case BYTE_TYPE:
            case INT_16_TYPE:
            case UINT_16_TYPE:
            case INT_32_TYPE:
            case UINT_32_TYPE:
            case INT_64_TYPE:
            case UINT_64_TYPE:
            case FLOAT_32_TYPE:
            case FLOAT_64_TYPE:
            case FLOAT_128_TYPE:
            case CHAR_8_TYPE:
            case CHAR_32_TYPE:
            case ENUMERATION_TYPE:
            case BITSET_TYPE:
                return true;
            default:
                return false;
        }
    }

    // ------------------------------------------------------------------
    // Encoding
    // ------------------------------------------------------------------

    void encode(Writer w, Class<?> cls, Type generic, Object value) {
        try {
            encodeKind(w, cls, generic, value);
        } catch (org.zerodds.wire.WireException e) {
            // a value the stated kind or bound cannot hold (a bound, a range of the wire type)
            throw new XcdrException(e.getMessage(), e);
        }
    }

    private void encodeKind(Writer w, Class<?> cls, Type generic, Object value) {
        switch (kind) {
            case BOOLEAN_TYPE:
                w.writeBool((Boolean) value);
                break;
            case BYTE_TYPE:
                w.writeU8(((Number) value).intValue() & 0xff);
                break;
            case INT_16_TYPE:
                w.writeI16(((Number) value).shortValue());
                break;
            case UINT_16_TYPE: {
                long v = ((Number) value).longValue();
                if (cls == short.class || cls == Short.class) {
                    w.writeU16((int) (v & 0xffff)); // the bit pattern (§8.4)
                } else {
                    require(v >= 0 && v <= 0xffffL, "UINT_16", value);
                    w.writeU16((int) v);
                }
                break;
            }
            case INT_32_TYPE:
                w.writeI32(((Number) value).intValue());
                break;
            case UINT_32_TYPE: {
                long v = ((Number) value).longValue();
                if (cls == int.class || cls == Integer.class) {
                    w.writeU32(v & 0xffffffffL); // the bit pattern (§8.4)
                } else {
                    require(v >= 0 && v <= 0xffffffffL, "UINT_32", value);
                    w.writeU32(v);
                }
                break;
            }
            case INT_64_TYPE:
                w.writeI64(((Number) value).longValue());
                break;
            case UINT_64_TYPE:
                if (value instanceof BigInteger) {
                    BigInteger v = (BigInteger) value;
                    require(v.signum() >= 0 && v.bitLength() <= 64, "UINT_64", value);
                    w.writeU64(v.longValue());
                } else {
                    w.writeU64(((Number) value).longValue());
                }
                break;
            case FLOAT_32_TYPE:
                w.writeF32(((Number) value).floatValue());
                break;
            case FLOAT_64_TYPE:
                w.writeF64(((Number) value).doubleValue());
                break;
            case FLOAT_128_TYPE:
                w.writeF128(value instanceof BigDecimal
                        ? Float128.fromBigDecimal((BigDecimal) value)
                        : Float128.fromDouble(((Number) value).doubleValue()));
                break;
            case CHAR_8_TYPE:
                w.writeChar((Character) value);
                break;
            case CHAR_32_TYPE: {
                long v = value instanceof Character ? (Character) value : ((Number) value).longValue();
                require(v >= 0 && v <= 0x10ffffL, "CHAR_32", value);
                w.writeU32(v);
                break;
            }
            case ENUMERATION_TYPE:
                w.writeI32(((Enum<?>) value).ordinal());
                break;
            case STRING_TYPE:
                encodeString(w, (String) value);
                break;
            case SEQUENCE_TYPE:
            case ARRAY_TYPE:
                encodeCollection(w, cls, generic, value);
                break;
            case MAP_TYPE:
                encodeMap(w, generic, (Map<?, ?>) value);
                break;
            case BITSET_TYPE:
                if (bitEnum != null && !(value instanceof java.util.BitSet)) {
                    w.writeBitset(octets(), bitsOfConstants(value));
                } else {
                    w.writeBitset(octets(), bits(value));
                }
                break;
            case STRUCTURE_TYPE:
            case UNION_TYPE:
                ReflectionTypeSupport.encodeStruct(w, ReflectionTypeSupport.infoOf(cls), value);
                break;
            default:
                throw new XcdrException("no wire form for " + kind);
        }
    }

    private static void require(boolean ok, String what, Object value) {
        if (!ok) {
            throw new XcdrException(value + " is outside the range of " + what);
        }
    }

    private int octets() {
        long bits = bound[0];
        return bits <= 8 ? 1 : bits <= 16 ? 2 : bits <= 32 ? 4 : 8;
    }

    private static long bits(Object value) {
        if (value instanceof java.util.BitSet) {
            long[] words = ((java.util.BitSet) value).toLongArray();
            if (words.length > 1) {
                throw new XcdrException("a bit set of more than 64 bits");
            }
            return words.length == 0 ? 0 : words[0];
        }
        return ((Number) value).longValue();
    }

    private void encodeString(Writer w, String value) {
        boolean wide = element == TypeKind.CHAR_32_TYPE;
        if (wide) {
            WideStrings.write(w, value, bound.length == 1 ? (int) bound[0] : 0);
        } else {
            NarrowStrings.write(w, value, bound.length == 1 ? (int) bound[0] : 0);
        }
    }

    private void encodeCollection(Writer w, Class<?> cls, Type generic, Object value) {
        boolean array = kind == TypeKind.ARRAY_TYPE;
        int dims = array ? bound.length : 1;
        Class<?> elemCls;
        Type elemGeneric;
        List<Object> values;
        if (cls.isArray()) {
            values = flatten(value, dims);
            elemCls = cls;
            for (int i = 0; i < dims; i++) {
                elemCls = elemCls.getComponentType();
            }
            elemGeneric = elemCls;
        } else {
            values = new ArrayList<>((Collection<?>) value);
            elemCls = ReflectionTypeSupport.elementClass(generic, 0);
            elemGeneric = ReflectionTypeSupport.elementType(generic, 0);
        }
        JavaRepresentation elementRep = elementRep();
        boolean primitive = elementRep != null ? primitiveKind(elementRep.kind)
                : ReflectionTypeSupport.isPrimitiveElement(elemCls);
        if (array) {
            long length = 1;
            for (long b : bound) {
                length *= b;
            }
            if (values.size() != length) {
                throw new XcdrException("an array of " + length + " elements holds "
                        + values.size());
            }
            Writer.DHeader dh = primitive ? null : w.writeDheader();
            for (Object v : values) {
                encodeElement(w, elementRep, elemCls, elemGeneric, v);
            }
            if (dh != null) {
                w.finishDheader(dh);
            }
            return;
        }
        if (bound.length == 1) {
            w.checkSequenceBound(values.size(), (int) bound[0]);
        }
        if (primitive) {
            w.writeU32(values.size());
            for (Object v : values) {
                encodeElement(w, elementRep, elemCls, elemGeneric, v);
            }
        } else {
            Writer.DHeader dh = w.writeDheader();
            w.writeU32(values.size());
            for (Object v : values) {
                encodeElement(w, elementRep, elemCls, elemGeneric, v);
            }
            w.finishDheader(dh);
        }
    }

    private static void encodeElement(Writer w, JavaRepresentation rep, Class<?> cls,
            Type generic, Object value) {
        if (rep != null) {
            rep.encode(w, cls, generic, value);
        } else {
            ReflectionTypeSupport.encodeValue(w, cls, generic, value);
        }
    }

    /** The elements of a (possibly multidimensional) Java array, row-major. */
    private static List<Object> flatten(Object array, int dims) {
        List<Object> out = new ArrayList<>();
        flatten(array, dims, out);
        return out;
    }

    private static void flatten(Object array, int dims, List<Object> out) {
        int n = Array.getLength(array);
        for (int i = 0; i < n; i++) {
            Object e = Array.get(array, i);
            if (dims > 1) {
                flatten(e, dims - 1, out);
            } else {
                out.add(e);
            }
        }
    }

    private void encodeMap(Writer w, Type generic, Map<?, ?> map) {
        Class<?> kCls = ReflectionTypeSupport.elementClass(generic, 0);
        Class<?> vCls = ReflectionTypeSupport.elementClass(generic, 1);
        Type kGen = ReflectionTypeSupport.elementType(generic, 0);
        Type vGen = ReflectionTypeSupport.elementType(generic, 1);
        if (bound.length == 1) {
            w.checkMapBound(map.size(), (int) bound[0]);
        }
        Writer.DHeader dh = w.writeDheader();
        w.writeU32(map.size());
        for (Map.Entry<?, ?> e : map.entrySet()) {
            encodeElement(w, keyRep(), kCls, kGen, e.getKey());
            encodeElement(w, elementRep(), vCls, vGen, e.getValue());
        }
        w.finishDheader(dh);
    }

    // ------------------------------------------------------------------
    // Decoding
    // ------------------------------------------------------------------

    Object decode(Reader r, Class<?> cls, Type generic) {
        switch (kind) {
            case BOOLEAN_TYPE:
                return r.readBool();
            case BYTE_TYPE:
                return (byte) r.readU8();
            case INT_16_TYPE:
                return (short) r.readI16();
            case UINT_16_TYPE: {
                int v = r.readU16();
                return cls == short.class || cls == Short.class ? (Object) (short) v : (Object) v;
            }
            case INT_32_TYPE:
                return r.readI32();
            case UINT_32_TYPE: {
                long v = r.readU32();
                return cls == int.class || cls == Integer.class ? (Object) (int) v : (Object) v;
            }
            case INT_64_TYPE:
                return r.readI64();
            case UINT_64_TYPE: {
                long v = r.readU64();
                return cls == BigInteger.class ? (Object) new BigInteger(Long.toUnsignedString(v))
                        : (Object) v;
            }
            case FLOAT_32_TYPE:
                return r.readF32();
            case FLOAT_64_TYPE:
                return r.readF64();
            case FLOAT_128_TYPE: {
                Float128 v = r.readF128();
                return cls == BigDecimal.class ? (Object) v.toBigDecimal() : (Object) v.toDouble();
            }
            case CHAR_8_TYPE:
                return (char) r.readChar();
            case CHAR_32_TYPE: {
                long v = r.readU32();
                if (cls == char.class || cls == Character.class) {
                    require(v <= 0xffffL, "a Java char", v);
                    return (char) v;
                }
                require(v <= 0x10ffffL, "CHAR_32", v);
                return (int) v;
            }
            case ENUMERATION_TYPE:
                return ReflectionTypeSupport.enumConstant(cls, r.readI32());
            case STRING_TYPE:
                if (element == TypeKind.CHAR_32_TYPE) {
                    return WideStrings.read(r, bound.length == 1 ? (int) bound[0] : 0);
                }
                return NarrowStrings.read(r, bound.length == 1 ? (int) bound[0] : 0);
            case SEQUENCE_TYPE:
            case ARRAY_TYPE:
                return decodeCollection(r, cls, generic);
            case MAP_TYPE:
                return decodeMap(r, cls, generic);
            case BITSET_TYPE: {
                long bits = r.readBitset(octets());
                if (bitEnum != null) {
                    return constantsOfBits(bits, cls);
                }
                if (cls == java.util.BitSet.class) {
                    return java.util.BitSet.valueOf(new long[] {bits});
                }
                if (cls == long.class || cls == Long.class) {
                    return bits;
                }
                if (cls == int.class || cls == Integer.class) {
                    return (int) bits;
                }
                if (cls == short.class || cls == Short.class) {
                    return (short) bits;
                }
                return (byte) bits;
            }
            case STRUCTURE_TYPE:
            case UNION_TYPE:
                return ReflectionTypeSupport.decodeStruct(r, ReflectionTypeSupport.infoOf(cls));
            default:
                throw new XcdrException("no wire form for " + kind);
        }
    }

    private static Object decodeElement(Reader r, JavaRepresentation rep, Class<?> cls,
            Type generic) {
        return rep != null ? rep.decode(r, cls, generic)
                : ReflectionTypeSupport.decodeValue(r, cls, generic);
    }

    private Object decodeCollection(Reader r, Class<?> cls, Type generic) {
        boolean array = kind == TypeKind.ARRAY_TYPE;
        Class<?> elemCls;
        Type elemGeneric;
        int dims = array ? bound.length : 1;
        if (cls.isArray()) {
            elemCls = cls;
            for (int i = 0; i < dims; i++) {
                elemCls = elemCls.getComponentType();
            }
            elemGeneric = elemCls;
        } else {
            elemCls = ReflectionTypeSupport.elementClass(generic, 0);
            elemGeneric = ReflectionTypeSupport.elementType(generic, 0);
        }
        JavaRepresentation elementRep = elementRep();
        boolean primitive = elementRep != null ? primitiveKind(elementRep.kind)
                : ReflectionTypeSupport.isPrimitiveElement(elemCls);
        List<Object> values = new ArrayList<>();
        if (array) {
            long length = 1;
            for (long b : bound) {
                length *= b;
            }
            if (!primitive) {
                r.readDheader();
            }
            for (long i = 0; i < length; i++) {
                values.add(decodeElement(r, elementRep, elemCls, elemGeneric));
            }
        } else {
            if (!primitive) {
                r.readDheader();
            }
            int n = r.readSequenceLength();
            if (bound.length == 1 && n > bound[0]) {
                throw new XcdrException("a sequence of at most " + bound[0]
                        + " elements has " + n);
            }
            for (int i = 0; i < n; i++) {
                values.add(decodeElement(r, elementRep, elemCls, elemGeneric));
            }
        }
        if (cls.isArray()) {
            return nest(cls, dims, values, new int[] {0}, array ? bound : null);
        }
        return ReflectionTypeSupport.collection(cls, elemCls, values);
    }

    /** Rebuilds a Java array of {@code dims} dimensions from the flat list. */
    private static Object nest(Class<?> cls, int dims, List<Object> flat, int[] at, long[] sizes) {
        Class<?> component = cls.getComponentType();
        int length = dims == 1 && sizes == null ? flat.size()
                : sizes == null ? flat.size() : (int) sizes[sizes.length - dims];
        Object out = Array.newInstance(component, length);
        for (int i = 0; i < length; i++) {
            Object e = dims > 1 ? nest(component, dims - 1, flat, at, sizes) : flat.get(at[0]++);
            Array.set(out, i, e);
        }
        return out;
    }

    private Object decodeMap(Reader r, Class<?> cls, Type generic) {
        Class<?> kCls = ReflectionTypeSupport.elementClass(generic, 0);
        Class<?> vCls = ReflectionTypeSupport.elementClass(generic, 1);
        Type kGen = ReflectionTypeSupport.elementType(generic, 0);
        Type vGen = ReflectionTypeSupport.elementType(generic, 1);
        r.readDheader();
        int n = r.readSequenceLength();
        if (bound.length == 1 && n > bound[0]) {
            throw new XcdrException("a map of at most " + bound[0] + " entries has " + n);
        }
        Map<Object, Object> out = ReflectionTypeSupport.map(cls, kCls);
        for (int i = 0; i < n; i++) {
            Object k = decodeElement(r, keyRep(), kCls, kGen);
            ReflectionTypeSupport.put(out, k, decodeElement(r, elementRep(), vCls, vGen));
        }
        return out;
    }
}
