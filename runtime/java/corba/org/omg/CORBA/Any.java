// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
package org.omg.CORBA;

/**
 * The IDL {@code any} on the CORBA platform: IDL4 to Java Language Mapping
 * 1.0 Annex A.1.4, a public class {@code org.omg.CORBA.Any} with the
 * operations the clause lists.
 *
 * <p>An any holds a {@link TypeCode} and a value of that type. An
 * {@code insert_<type>} sets both; {@code extract_<type>} returns the
 * value when the type code held is equivalent to that of {@code <type>}
 * (CORBA 3.3 part 1 §8.11.1, aliases unwound) and raises
 * {@code BAD_OPERATION} otherwise, "the type of an any did not match the
 * extraction" (CORBA 3.3 part 1, the standard system exceptions).
 * {@link #type(TypeCode)} sets the type code and leaves no value, which
 * an extraction then reports the same way.
 */
public class Any implements java.io.Serializable {
    private static final long serialVersionUID = 1L;

    private TypeCode type = new TypeCode();
    private java.lang.Object value;
    private boolean holds;

    /** Whether both hold equal type codes and equal values. */
    public boolean equal(Any a) {
        if (a == null || !type.equal(a.type) || holds != a.holds) {
            return false;
        }
        if (value instanceof Any && a.value instanceof Any) {
            return ((Any) value).equal((Any) a.value);
        }
        return java.util.Objects.equals(value, a.value);
    }

    /** The type code of the value held. */
    public TypeCode type() {
        return type;
    }

    /** Sets the type code; the any holds no value until one is inserted. */
    public void type(TypeCode t) {
        this.type = t;
        this.value = null;
        this.holds = false;
    }

    public void insert_short(short value) {
        hold(TCKind.tk_short, value);
    }

    public short extract_short() {
        return (Short) held(TCKind.tk_short);
    }

    public void insert_long(int value) {
        hold(TCKind.tk_long, value);
    }

    public int extract_long() {
        return (Integer) held(TCKind.tk_long);
    }

    public void insert_longlong(long value) {
        hold(TCKind.tk_longlong, value);
    }

    public long extract_longlong() {
        return (Long) held(TCKind.tk_longlong);
    }

    public void insert_ushort(short value) {
        hold(TCKind.tk_ushort, value);
    }

    public short extract_ushort() {
        return (Short) held(TCKind.tk_ushort);
    }

    public void insert_ulong(int value) {
        hold(TCKind.tk_ulong, value);
    }

    public int extract_ulong() {
        return (Integer) held(TCKind.tk_ulong);
    }

    public void insert_ulonglong(long value) {
        hold(TCKind.tk_ulonglong, value);
    }

    public long extract_ulonglong() {
        return (Long) held(TCKind.tk_ulonglong);
    }

    public void insert_float(float value) {
        hold(TCKind.tk_float, value);
    }

    public float extract_float() {
        return (Float) held(TCKind.tk_float);
    }

    public void insert_double(double value) {
        hold(TCKind.tk_double, value);
    }

    public double extract_double() {
        return (Double) held(TCKind.tk_double);
    }

    public void insert_boolean(boolean value) {
        hold(TCKind.tk_boolean, value);
    }

    public boolean extract_boolean() {
        return (Boolean) held(TCKind.tk_boolean);
    }

    public void insert_char(char value) {
        hold(TCKind.tk_char, value);
    }

    public char extract_char() {
        return (Character) held(TCKind.tk_char);
    }

    public void insert_wchar(char value) {
        hold(TCKind.tk_wchar, value);
    }

    public char extract_wchar() {
        return (Character) held(TCKind.tk_wchar);
    }

    public void insert_octet(byte value) {
        hold(TCKind.tk_octet, value);
    }

    public byte extract_octet() {
        return (Byte) held(TCKind.tk_octet);
    }

    public void insert_any(Any value) {
        hold(TCKind.tk_any, value);
    }

    public Any extract_any() {
        return (Any) held(TCKind.tk_any);
    }

    public void insert_object(Object value) {
        this.type = TypeCode.get_primitive_tc(TCKind.tk_objref);
        this.value = value;
        this.holds = true;
    }

    public Object extract_object() {
        return (Object) held(TCKind.tk_objref);
    }

    private void hold(TCKind kind, java.lang.Object value) {
        this.type = TypeCode.get_primitive_tc(kind);
        this.value = value;
        this.holds = true;
    }

    private java.lang.Object held(TCKind kind) {
        if (!holds || !type.equivalent(TypeCode.get_primitive_tc(kind))) {
            throw new BAD_OPERATION(0, completion_status.COMPLETED_NO);
        }
        return value;
    }
}
