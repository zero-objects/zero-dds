// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
package org.omg.type;

/**
 * The IDL {@code any}: IDL4 to Java Language Mapping 1.0 §7.3, "middleware
 * specific, and should include operations that allow programmers to insert
 * and access the value contained in an any instance as well as the actual
 * type of that value".
 *
 * <p>The value is held as the Java value of the IDL type it was inserted
 * as; {@link #type()} answers that Java type.
 */
public final class Any implements java.io.Serializable {
    private static final long serialVersionUID = 1L;

    private java.io.Serializable value;

    /** An any holding nothing. */
    public Any() {
    }

    /** An any holding {@code value}. */
    public Any(java.io.Serializable value) {
        this.value = value;
    }

    /** Holds {@code value} from now on. */
    public void insert(java.io.Serializable value) {
        this.value = value;
    }

    /** The value held, {@code null} for none. */
    public java.lang.Object extract() {
        return value;
    }

    /**
     * The value held as a {@code type}.
     *
     * @param type the Java type of the value
     * @param <T> the Java type of the value
     * @return the value
     * @throws ClassCastException when the value held is no {@code type}
     */
    public <T> T extract(Class<T> type) {
        return type.cast(value);
    }

    /** The Java type of the value held, {@code null} for none. */
    public Class<?> type() {
        return value == null ? null : value.getClass();
    }

    @Override
    public boolean equals(java.lang.Object other) {
        return other instanceof Any && java.util.Objects.equals(value, ((Any) other).value);
    }

    @Override
    public int hashCode() {
        return java.util.Objects.hashCode(value);
    }

    @Override
    public String toString() {
        return "Any(" + value + ")";
    }
}
