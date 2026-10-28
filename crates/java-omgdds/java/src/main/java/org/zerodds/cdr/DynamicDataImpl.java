// SPDX-License-Identifier: Apache-2.0
package org.zerodds.cdr;

import org.omg.dds.core.ServiceEnvironment;
import org.zerodds.core.PreconditionNotMetException;
import org.omg.dds.type.TypeKind;
import org.omg.dds.type.dynamic.DynamicData;
import org.omg.dds.type.dynamic.DynamicType;
import org.omg.dds.type.dynamic.DynamicTypeMember;
import org.omg.dds.type.dynamic.MemberDescriptor;

import java.math.BigDecimal;
import java.math.BigInteger;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

/**
 * ZeroDDS {@link DynamicData} (DDS-Java-PSM 1.0 §7.8.1.4): the data of a
 * {@link DynamicType}, addressed by member id or name.
 *
 * <p>Every {@code set*} method returns this object for chaining, every
 * {@code get*} returns the value cast to the Java type of Clause 8. Unsigned
 * types are reachable through the signed type of the same size, which keeps the
 * bit pattern, and through the signed type one size up, which keeps the logical
 * value ({@code UInt16} as {@code int}, {@code UInt32} as {@code long},
 * {@code UInt64} as {@link BigInteger}); FLOAT_128 is a {@link BigDecimal}
 * (§7.8.1.4). The held value of a member is the Java type of its kind:
 * {@code Short} for Int16 and UInt16, {@code Integer} for Int32, UInt32 and an
 * enumeration, {@code Long} for Int64 and UInt64.
 *
 * <p>{@link #clone()} copies the values deeply, and {@link #equals} compares
 * the types and the values as XTypes 1.3 §7.5.2.11.5 does: members without
 * regard to their order, and a non-optional member that one sample omits equals
 * the default value in the other.
 */
public final class DynamicDataImpl implements DynamicData {

    /**
     * The key under which the discriminator of a union sample is kept: it is
     * not a member, so no member id can be it.
     */
    public static final int DISCRIMINATOR_ID = -1;

    private final ServiceEnvironment environment;
    private final DynamicType type;
    private final Map<Integer, Object> values = new LinkedHashMap<>();

    /** The value lent by {@link #loanValue}, or {@code null} (§7.5.2.11.7: one loan at a time). */
    private DynamicDataImpl loan;
    private int loanId;
    private boolean loanIsWrapper;

    public DynamicDataImpl(ServiceEnvironment environment, DynamicType type) {
        this.environment = environment;
        this.type = type;
    }

    private DynamicDataImpl(DynamicDataImpl other) {
        this.environment = other.environment;
        this.type = other.type;
        for (Map.Entry<Integer, Object> e : other.values.entrySet()) {
            this.values.put(e.getKey(), deepCopy(e.getValue()));
        }
    }

    @Override
    public ServiceEnvironment getEnvironment() {
        return environment;
    }

    @Override
    public DynamicType getType() {
        return type;
    }

    @Override
    public List<MemberDescriptor> getDescriptors() {
        List<MemberDescriptor> descriptors = new ArrayList<>();
        for (DynamicTypeMember member : DynamicDataTypeSupport.membersOf(type)) {
            descriptors.add(member.getDescriptor());
        }
        return descriptors;
    }

    @Override
    public int getMemberIdByName(String name) {
        if (name == null) {
            throw new IllegalArgumentException("a member name must not be null");
        }
        for (DynamicTypeMember member : DynamicDataTypeSupport.membersOf(type)) {
            if (member.getName().equals(name)) {
                return member.getId();
            }
        }
        throw new PreconditionNotMetException(
                "the type " + type.getName() + " has no member named " + name);
    }

    @Override
    public int getMemberIdAtIndex(int index) {
        List<MemberDescriptor> descriptors = getDescriptors();
        if (index < 0 || index >= descriptors.size()) {
            throw new PreconditionNotMetException("member index out of range: " + index);
        }
        return descriptors.get(index).getId();
    }

    /** The member {@code id} of the type or of one of its base types, or {@code null}. */
    private DynamicTypeMember memberOf(int id) {
        for (DynamicTypeMember member : DynamicDataTypeSupport.membersOf(type)) {
            if (member.getId() == id) {
                return member;
            }
        }
        return null;
    }

    /** The kind of member {@code id}, aliases resolved, or {@code null} for no such member. */
    private TypeKind kindOf(int id) {
        DynamicTypeMember member = memberOf(id);
        if (member == null || member.getDescriptor().getType() == null) {
            return null;
        }
        return DynamicDataTypeSupport.resolve(member.getDescriptor().getType()).getKind();
    }

    private boolean isOptional(int id) {
        DynamicTypeMember member = memberOf(id);
        return member != null && DynamicDataTypeSupport.has(member.getAnnotations(), "Optional");
    }

    private boolean isKey(int id) {
        DynamicTypeMember member = memberOf(id);
        return member != null && DynamicDataTypeSupport.has(member.getAnnotations(), "Key");
    }

    @Override
    public void clearAllValues() {
        for (Integer id : new ArrayList<>(values.keySet())) {
            clear(id);
        }
    }

    @Override
    public void clearNonkeyValues() {
        for (Integer id : new ArrayList<>(values.keySet())) {
            if (!isKey(id)) {
                clear(id);
            }
        }
    }

    @Override
    public void clearValue(int id) {
        clear(id);
    }

    @Override
    public void clearValue(String name) {
        clear(getMemberIdByName(name));
    }

    /**
     * XTypes 1.3 §7.5.2.11.3: an optional member is removed, a non-optional member
     * takes the default value of its type (zero, {@code false}, the empty string, the
     * empty collection). A member of a structured or array type, the member of a union and
     * the discriminator have no value afterwards, which a sample treats as the default.
     */
    private void clear(int id) {
        DynamicTypeMember member = memberOf(id);
        Object fallback = null;
        if (member != null && member.getDescriptor().getType() != null
                && !DynamicDataTypeSupport.has(member.getAnnotations(), "Optional")
                && type.getKind() != TypeKind.UNION_TYPE) {
            fallback = defaultOf(member.getDescriptor().getType());
        }
        if (fallback == null) {
            values.remove(id);
        } else {
            values.put(id, fallback);
        }
    }

    /** The default of a primitive, string, sequence or map type; {@code null} for others. */
    private static Object defaultOf(DynamicType declared) {
        DynamicType t = DynamicDataTypeSupport.resolve(declared);
        switch (t.getKind()) {
            case BOOLEAN_TYPE:
                return Boolean.FALSE;
            case BYTE_TYPE:
                return (byte) 0;
            case INT_16_TYPE:
            case UINT_16_TYPE:
                return (short) 0;
            case INT_32_TYPE:
            case UINT_32_TYPE:
            case ENUMERATION_TYPE:
            case CHAR_32_TYPE:
                return 0;
            case INT_64_TYPE:
            case UINT_64_TYPE:
            case BITSET_TYPE:
                return 0L;
            case FLOAT_32_TYPE:
                return 0f;
            case FLOAT_64_TYPE:
                return 0d;
            case FLOAT_128_TYPE:
                return BigDecimal.ZERO;
            case CHAR_8_TYPE:
                return (char) 0;
            case STRING_TYPE:
                return "";
            case SEQUENCE_TYPE:
                return DynamicDataTypeSupport.collection(t.getDescriptor().getElementType(),
                        new ArrayList<Object>());
            case MAP_TYPE:
                return new LinkedHashMap<Object, Object>();
            default:
                return null;
        }
    }

    @Override
    public <T> void setValue(int id, T value, Class<T> type) {
        values.put(id, value);
    }

    @Override
    public <T> void setValue(String name, T value, Class<T> type) {
        values.put(getMemberIdByName(name), value);
    }

    @Override
    @SuppressWarnings("unchecked")
    public <T> T getValue(int id, Class<T> type) {
        return (T) values.get(id);
    }

    @Override
    public <T> T getValue(String name, Class<T> type) {
        return getValue(getMemberIdByName(name), type);
    }

    /**
     * XTypes 1.3 §7.5.2.11.7: lends the value of member {@code id} without copying it. A
     * value of a structured type is the nested {@link DynamicData} itself, so what the caller
     * changes through it is changed in this sample. Any other value is lent in a
     * {@code DynamicData} of the member's type that holds it under
     * {@code MEMBER_ID_INVALID}, and {@link #returnLoanedValue} writes it back.
     *
     * @return the value, or {@code null} if the member has no value or another loan has not
     *     been returned yet
     */
    @Override
    public DynamicData loanValue(int id) {
        if (loan != null) {
            return null;
        }
        Object current = values.get(id);
        if (current == null) {
            return null;
        }
        if (current instanceof DynamicDataImpl) {
            loan = (DynamicDataImpl) current;
            loanIsWrapper = false;
        } else {
            DynamicTypeMember member = memberOf(id);
            if (member == null || member.getDescriptor().getType() == null) {
                return null;
            }
            DynamicDataImpl wrapper = new DynamicDataImpl(environment,
                    member.getDescriptor().getType());
            wrapper.values.put(MEMBER_ID_INVALID, deepCopy(current));
            loan = wrapper;
            loanIsWrapper = true;
        }
        loanId = id;
        return loan;
    }

    /**
     * @throws PreconditionNotMetException if {@code value} is not the outstanding loan of
     *     this sample
     */
    @Override
    public void returnLoanedValue(DynamicData value) {
        if (loan == null || value != loan) {
            throw new PreconditionNotMetException(
                    "the DynamicData is not an outstanding loan of this sample");
        }
        if (loanIsWrapper) {
            values.put(loanId, loan.values.get(MEMBER_ID_INVALID));
        }
        loan = null;
    }

    /** {@code MEMBER_ID_INVALID} (XTypes 1.3 §7.5.2.5), as the PSM declares it in {@code Member}. */
    static final int MEMBER_ID_INVALID = org.omg.dds.type.typeobject.Member.MEMBER_ID_INVALID;

    // --- Typed accessors: the primitive and collection forms of §7.8.1.4 ---

    @Override
    public int getInt32Value(int id) {
        Object held = value(id);
        if (held instanceof Integer) {
            return (Integer) held;
        }
        if (held instanceof Short) {
            return (int) widen((Short) held, id, TypeKind.UINT_16_TYPE, 0xffffL);
        }
        throw notHeld(id, held, "an int (Int32, UInt32 or Int16, UInt16)");
    }

    @Override
    public DynamicData setInt32Value(int id, int value) {
        return putIntegral(id, value, 32);
    }

    @Override
    public short getInt16Value(int id) {
        Object held = value(id);
        if (held instanceof Short) {
            return (Short) held;
        }
        throw notHeld(id, held, "a short (Int16 or UInt16)");
    }

    @Override
    public DynamicData setInt16Value(int id, short value) {
        return putIntegral(id, value, 16);
    }

    @Override
    public long getInt64Value(int id) {
        Object held = value(id);
        if (held instanceof Long) {
            return (Long) held;
        }
        if (held instanceof Integer) {
            return widen((Integer) held, id, TypeKind.UINT_32_TYPE, 0xffffffffL);
        }
        if (held instanceof Short) {
            return widen((Short) held, id, TypeKind.UINT_16_TYPE, 0xffffL);
        }
        throw notHeld(id, held, "a long (Int64, UInt64, Int32, UInt32, Int16 or UInt16)");
    }

    @Override
    public DynamicData setInt64Value(int id, long value) {
        return putIntegral(id, value, 64);
    }

    @Override
    public BigInteger getBigIntegerValue(int id) {
        Object held = value(id);
        if (held instanceof BigInteger) {
            return (BigInteger) held;
        }
        if (held instanceof Long) {
            long bits = (Long) held;
            return kindOf(id) == TypeKind.UINT_64_TYPE
                    ? new BigInteger(Long.toUnsignedString(bits)) : BigInteger.valueOf(bits);
        }
        if (held instanceof Integer || held instanceof Short) {
            return BigInteger.valueOf(getInt64Value(id));
        }
        throw notHeld(id, held, "a BigInteger (an integer kind of 16 to 64 bits)");
    }

    @Override
    public DynamicData setBigIntegerValue(int id, BigInteger value) {
        TypeKind kind = kindOf(id);
        if (kind == TypeKind.UINT_64_TYPE) {
            if (value.signum() < 0 || value.bitLength() > 64) {
                throw new IllegalArgumentException(value + " is outside the range of UInt64");
            }
            return put(id, value.longValue());
        }
        if (widthOf(kind) != 0) {
            if (value.bitLength() > 63) {
                throw new IllegalArgumentException(value + " is outside the range of " + kind);
            }
            return putIntegral(id, value.longValue(), 64);
        }
        return put(id, value);
    }

    /** The bits of an integer kind that {@code Int16}..{@code Int64} accessors address; 0 if none. */
    private static int widthOf(TypeKind kind) {
        if (kind == null) {
            return 0;
        }
        switch (kind) {
            case INT_16_TYPE:
            case UINT_16_TYPE:
                return 16;
            case INT_32_TYPE:
            case UINT_32_TYPE:
            case ENUMERATION_TYPE:
                return 32;
            case INT_64_TYPE:
            case UINT_64_TYPE:
                return 64;
            default:
                return 0;
        }
    }

    /**
     * The logical value of a held number: for the member kind {@code unsignedKind} the
     * bits below {@code mask}, for every other kind the sign-extended value.
     */
    private long widen(Number held, int id, TypeKind unsignedKind, long mask) {
        return kindOf(id) == unsignedKind ? held.longValue() & mask : held.longValue();
    }

    /**
     * Stores {@code value} given by an accessor of {@code accessorBits} bits. For a member of
     * the same size the bit pattern is stored (Int32 for UInt32); for a smaller accessor
     * the logical value is promoted, for a larger one it is range-checked against the kind of
     * the member (Int64 for UInt32). A member of a kind without an integer width, or no member
     * of this name, takes the number in the Java type of the accessor.
     */
    private DynamicData putIntegral(int id, long value, int accessorBits) {
        int width = widthOf(kindOf(id));
        if (width == 0) {
            return put(id, accessorBits == 16 ? (Object) (short) value
                    : accessorBits == 32 ? (Object) (int) value : (Object) value);
        }
        TypeKind kind = kindOf(id);
        boolean unsigned = kind == TypeKind.UINT_16_TYPE || kind == TypeKind.UINT_32_TYPE
                || kind == TypeKind.UINT_64_TYPE;
        if (accessorBits != width) {
            boolean outside;
            if (unsigned) {
                outside = value < 0 || (width < 63 && value >= (1L << width));
            } else {
                outside = width < 64 && (value < -(1L << (width - 1))
                        || value >= (1L << (width - 1)));
            }
            if (outside) {
                throw new IllegalArgumentException(value + " is outside the range of " + kind
                        + " (member " + id + ")");
            }
        }
        return put(id, width == 16 ? (Object) (short) value
                : width == 32 ? (Object) (int) value : (Object) value);
    }

    private Object value(int id) {
        Object held = values.get(id);
        if (held == null) {
            throw new PreconditionNotMetException("member " + id + " of " + type.getName()
                    + " has no value");
        }
        return held;
    }

    private IllegalArgumentException notHeld(int id, Object held, String wanted) {
        return new IllegalArgumentException("member " + id + " holds a "
                + held.getClass().getName() + ", which this accessor does not give as " + wanted);
    }

    @Override
    public float getFloat32Value(int id) {
        return single(id, Float.class);
    }

    @Override
    public DynamicData setFloat32Value(int id, float value) {
        return put(id, value);
    }

    @Override
    public double getFloat64Value(int id) {
        Object held = value(id);
        if (held instanceof Double) {
            return (Double) held;
        }
        if (held instanceof Float) {
            return (Float) held; // Float32 promoted to the type one size up.
        }
        throw notHeld(id, held, "a double (Float64 or Float32)");
    }

    @Override
    public DynamicData setFloat64Value(int id, double value) {
        return put(id, value);
    }

    @Override
    public BigDecimal getBigDecimalValue(int id) {
        return single(id, BigDecimal.class);
    }

    @Override
    public DynamicData setBigDecimalValue(int id, BigDecimal value) {
        return put(id, value);
    }

    @Override
    public char getCharValue(int id) {
        return single(id, Character.class);
    }

    @Override
    public DynamicData setCharValue(int id, char value) {
        return put(id, value);
    }

    @Override
    public byte getByteValue(int id) {
        return single(id, Byte.class);
    }

    @Override
    public DynamicData setByteValue(int id, byte value) {
        return put(id, value);
    }

    @Override
    public boolean getBooleanValue(int id) {
        return single(id, Boolean.class);
    }

    @Override
    public DynamicData setBooleanValue(int id, boolean value) {
        return put(id, value);
    }

    @Override
    public String getStringValue(int id) {
        return single(id, String.class);
    }

    @Override
    public DynamicData setStringValue(int id, CharSequence value) {
        return put(id, value == null ? null : value.toString());
    }

    @Override
    public DynamicData getComplexValue(int id) {
        return single(id, DynamicData.class);
    }

    @Override
    public DynamicData getComplexValue(String name) {
        return getComplexValue(getMemberIdByName(name));
    }

    @Override
    public DynamicData setComplexValue(int id, DynamicData value) {
        return put(id, value);
    }

    // --- Collections ------------------------------------------------------

    @Override
    public int getInt32Values(int[] value, int offset, int length, int id) {
        int[] stored = (int[]) array(id);
        System.arraycopy(stored, 0, value, offset, Math.min(length, stored.length));
        return stored.length;
    }

    @Override
    public DynamicData setInt32Values(int id, int[] value, int offset, int length) {
        return put(id, java.util.Arrays.copyOfRange(value, offset, offset + length));
    }

    @Override
    public int getInt16Values(short[] value, int offset, int length, int id) {
        short[] stored = (short[]) array(id);
        System.arraycopy(stored, 0, value, offset, Math.min(length, stored.length));
        return stored.length;
    }

    @Override
    public DynamicData setInt16Values(int id, short[] value, int offset, int length) {
        return put(id, java.util.Arrays.copyOfRange(value, offset, offset + length));
    }

    @Override
    public int getInt64Values(long[] value, int offset, int length, int id) {
        long[] stored = (long[]) array(id);
        System.arraycopy(stored, 0, value, offset, Math.min(length, stored.length));
        return stored.length;
    }

    @Override
    public DynamicData setInt64Values(int id, long[] value, int offset, int length) {
        return put(id, java.util.Arrays.copyOfRange(value, offset, offset + length));
    }

    @Override
    public List<BigInteger> getBigIntegerValues(List<BigInteger> value, int id) {
        Object held = values.get(id);
        if (held instanceof long[]) {
            boolean unsigned = elementKind(id) == TypeKind.UINT_64_TYPE;
            for (long bits : (long[]) held) {
                value.add(unsigned ? new BigInteger(Long.toUnsignedString(bits))
                        : BigInteger.valueOf(bits));
            }
            return value;
        }
        return list(id, value);
    }

    @Override
    public DynamicData setBigIntegerValues(int id, List<BigInteger> value) {
        if (elementKind(id) == TypeKind.UINT_64_TYPE) {
            long[] bits = new long[value.size()];
            for (int i = 0; i < bits.length; i++) {
                BigInteger v = value.get(i);
                if (v.signum() < 0 || v.bitLength() > 64) {
                    throw new IllegalArgumentException(v + " is outside the range of UInt64");
                }
                bits[i] = v.longValue();
            }
            return put(id, bits);
        }
        return put(id, new ArrayList<>(value));
    }

    /** The kind of the elements of the sequence or array member {@code id}, or {@code null}. */
    private TypeKind elementKind(int id) {
        DynamicTypeMember member = memberOf(id);
        if (member == null || member.getDescriptor().getType() == null) {
            return null;
        }
        DynamicType element = DynamicDataTypeSupport.resolve(member.getDescriptor().getType())
                .getDescriptor().getElementType();
        return element == null ? null : DynamicDataTypeSupport.resolve(element).getKind();
    }

    @Override
    public int getFloat32Values(float[] value, int offset, int length, int id) {
        float[] stored = (float[]) array(id);
        System.arraycopy(stored, 0, value, offset, Math.min(length, stored.length));
        return stored.length;
    }

    @Override
    public DynamicData setFloat32Values(int id, float[] value, int offset, int length) {
        return put(id, java.util.Arrays.copyOfRange(value, offset, offset + length));
    }

    @Override
    public int getFloat64Values(double[] value, int offset, int length, int id) {
        double[] stored = (double[]) array(id);
        System.arraycopy(stored, 0, value, offset, Math.min(length, stored.length));
        return stored.length;
    }

    @Override
    public DynamicData setFloat64Values(int id, double[] value, int offset, int length) {
        return put(id, java.util.Arrays.copyOfRange(value, offset, offset + length));
    }

    @Override
    public List<BigDecimal> getBigDecimalValues(List<BigDecimal> value, int id) {
        return list(id, value);
    }

    @Override
    public DynamicData setBigDecimalValues(int id, List<BigDecimal> value) {
        return put(id, new ArrayList<>(value));
    }

    @Override
    public int getCharValues(char[] value, int offset, int length, int id) {
        char[] stored = (char[]) array(id);
        System.arraycopy(stored, 0, value, offset, Math.min(length, stored.length));
        return stored.length;
    }

    @Override
    public StringBuilder getCharValues(StringBuilder value, int id) {
        String stored = single(id, String.class);
        value.append(stored);
        return value;
    }

    @Override
    public DynamicData setCharValues(int id, char[] value, int offset, int length) {
        return put(id, new String(value, offset, length));
    }

    @Override
    public DynamicData setCharValues(int id, CharSequence value) {
        return put(id, value == null ? null : value.toString());
    }

    @Override
    public int getByteValues(byte[] value, int offset, int length, int id) {
        byte[] stored = (byte[]) array(id);
        System.arraycopy(stored, 0, value, offset, Math.min(length, stored.length));
        return stored.length;
    }

    @Override
    public DynamicData setByteValues(int id, byte[] value, int offset, int length) {
        return put(id, java.util.Arrays.copyOfRange(value, offset, offset + length));
    }

    @Override
    public int getBooleanValues(boolean[] value, int offset, int length, int id) {
        boolean[] stored = (boolean[]) array(id);
        System.arraycopy(stored, 0, value, offset, Math.min(length, stored.length));
        return stored.length;
    }

    @Override
    public DynamicData setBooleanValues(int id, boolean[] value, int offset, int length) {
        return put(id, java.util.Arrays.copyOfRange(value, offset, offset + length));
    }

    @Override
    public void getStringValues(List<String> value, int id) {
        value.addAll(list(id, new ArrayList<>()));
    }

    @Override
    public DynamicData setStringValues(int id, String[] value, int offset, int length) {
        return put(id, new ArrayList<>(java.util.Arrays.asList(
                java.util.Arrays.copyOfRange(value, offset, offset + length))));
    }

    @Override
    public DynamicData setStringValues(int id, List<String> value) {
        return put(id, new ArrayList<>(value));
    }

    /** XTypes 1.3 §7.5.2.11.4: a new sample with the same contents, sharing no value with this one. */
    @Override
    public DynamicData clone() {
        return new DynamicDataImpl(this);
    }

    private static Object deepCopy(Object value) {
        if (value == null) {
            return null;
        }
        if (value instanceof DynamicDataImpl) {
            return new DynamicDataImpl((DynamicDataImpl) value);
        }
        if (value.getClass().isArray()) {
            int n = java.lang.reflect.Array.getLength(value);
            Object copy = java.lang.reflect.Array.newInstance(value.getClass().getComponentType(), n);
            for (int i = 0; i < n; i++) {
                java.lang.reflect.Array.set(copy, i, deepCopy(java.lang.reflect.Array.get(value, i)));
            }
            return copy;
        }
        if (value instanceof List) {
            List<Object> copy = new ArrayList<>();
            for (Object element : (List<?>) value) {
                copy.add(deepCopy(element));
            }
            return copy;
        }
        if (value instanceof Map) {
            Map<Object, Object> copy = new LinkedHashMap<>();
            for (Map.Entry<?, ?> e : ((Map<?, ?>) value).entrySet()) {
                copy.put(e.getKey(), deepCopy(e.getValue()));
            }
            return copy;
        }
        return value; // numbers, booleans, characters and strings are immutable.
    }

    /**
     * XTypes 1.3 §7.5.2.11.5: equal types, and the same values for the members of the
     * aggregate without regard to their order; a non-optional member that one sample omits
     * is equal to the default value of the other.
     */
    @Override
    public boolean equals(Object o) {
        if (this == o) {
            return true;
        }
        if (!(o instanceof DynamicDataImpl)) {
            return false;
        }
        DynamicDataImpl other = (DynamicDataImpl) o;
        if (!type.equals(other.type)) {
            return false;
        }
        java.util.Set<Integer> ids = new java.util.LinkedHashSet<>(values.keySet());
        ids.addAll(other.values.keySet());
        for (Integer id : ids) {
            Object a = values.get(id);
            Object b = other.values.get(id);
            if (a == null || b == null) {
                Object present = a == null ? b : a;
                if (id == DISCRIMINATOR_ID || isOptional(id) || !isDefault(present)) {
                    return false;
                }
            } else if (!valueEquals(a, b)) {
                return false;
            }
        }
        return true;
    }

    @Override
    public int hashCode() {
        int hash = java.util.Objects.hashCode(type.getName());
        for (Map.Entry<Integer, Object> e : values.entrySet()) {
            if (e.getKey() == DISCRIMINATOR_ID || !isDefault(e.getValue())) {
                hash += e.getKey() * 31 + valueHash(e.getValue());
            }
        }
        return hash;
    }

    private static boolean isDefault(Object v) {
        if (v == null) {
            return true;
        }
        if (v instanceof Boolean) {
            return !(Boolean) v;
        }
        if (v instanceof Character) {
            return (Character) v == 0;
        }
        if (v instanceof BigDecimal) {
            return ((BigDecimal) v).signum() == 0;
        }
        if (v instanceof BigInteger) {
            return ((BigInteger) v).signum() == 0;
        }
        if (v instanceof Float || v instanceof Double) {
            return ((Number) v).doubleValue() == 0.0;
        }
        if (v instanceof Number) {
            return ((Number) v).longValue() == 0;
        }
        if (v instanceof CharSequence) {
            return ((CharSequence) v).length() == 0;
        }
        if (v instanceof java.util.Collection) {
            return ((java.util.Collection<?>) v).isEmpty();
        }
        if (v instanceof Map) {
            return ((Map<?, ?>) v).isEmpty();
        }
        if (v.getClass().isArray()) {
            return java.lang.reflect.Array.getLength(v) == 0;
        }
        return false;
    }

    private static boolean valueEquals(Object a, Object b) {
        if (a instanceof BigDecimal && b instanceof BigDecimal) {
            return ((BigDecimal) a).compareTo((BigDecimal) b) == 0;
        }
        if (a instanceof List && b instanceof List) {
            List<?> x = (List<?>) a;
            List<?> y = (List<?>) b;
            if (x.size() != y.size()) {
                return false;
            }
            for (int i = 0; i < x.size(); i++) {
                if (!valueEquals(x.get(i), y.get(i))) {
                    return false;
                }
            }
            return true;
        }
        if (a instanceof Map && b instanceof Map) {
            Map<?, ?> x = (Map<?, ?>) a;
            Map<?, ?> y = (Map<?, ?>) b;
            if (x.size() != y.size()) {
                return false;
            }
            for (Map.Entry<?, ?> e : x.entrySet()) {
                if (!y.containsKey(e.getKey()) || !valueEquals(e.getValue(), y.get(e.getKey()))) {
                    return false;
                }
            }
            return true;
        }
        return java.util.Objects.deepEquals(a, b);
    }

    private static int valueHash(Object v) {
        if (v == null) {
            return 0;
        }
        if (v instanceof BigDecimal) {
            BigDecimal d = (BigDecimal) v;
            return d.signum() == 0 ? 0 : d.stripTrailingZeros().hashCode();
        }
        if (v instanceof List) {
            int h = 1;
            for (Object element : (List<?>) v) {
                h = 31 * h + valueHash(element);
            }
            return h;
        }
        if (v instanceof Map) {
            int h = 0;
            for (Map.Entry<?, ?> e : ((Map<?, ?>) v).entrySet()) {
                h += java.util.Objects.hashCode(e.getKey()) ^ valueHash(e.getValue());
            }
            return h;
        }
        if (v.getClass().isArray()) {
            return java.util.Arrays.deepHashCode(new Object[] {v});
        }
        return v.hashCode();
    }

    // --- Raw access for the codec ------------------------------------------

    /** The value held for member {@code id} as it is stored, or {@code null}. */
    Object rawValue(int id) {
        return values.get(id);
    }

    /** Whether member {@code id} has a value. */
    boolean hasValue(int id) {
        return values.get(id) != null;
    }

    /** Stores a decoded value. */
    void putRaw(int id, Object value) {
        values.put(id, value);
    }

    // --- Storage helpers --------------------------------------------------

    private <T> T single(int id, Class<T> type) {
        Object value = values.get(id);
        if (value == null) {
            throw new PreconditionNotMetException("member " + id + " of " + type.getTypeName()
                    + " has no value");
        }
        if (!type.isInstance(value)) {
            throw new IllegalArgumentException("member " + id + " holds a "
                    + value.getClass().getName() + ", not a " + type.getName());
        }
        return type.cast(value);
    }

    /** The array stored at {@code id}; the caller casts it to the primitive type it asked for. */
    private Object array(int id) {
        Object value = values.get(id);
        if (value == null) {
            throw new PreconditionNotMetException("member " + id + " has no value");
        }
        return value;
    }

    @SuppressWarnings("unchecked")
    private <T> List<T> list(int id, List<T> into) {
        Object value = values.get(id);
        if (value != null) {
            into.addAll((List<T>) value);
        }
        return into;
    }

    private DynamicData put(int id, Object value) {
        values.put(id, value);
        return this;
    }

    @Override
    public String toString() {
        return "DynamicData[" + type.getName() + values + "]";
    }
}