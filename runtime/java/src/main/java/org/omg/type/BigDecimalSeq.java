// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
package org.omg.type;

/**
 * A sequence of IDL long double: IDL4 to Java Language Mapping 1.0
 * §7.2.4.2.1.1, Table 7.4.
 *
 * <p>Its element type is {@code java.math.BigDecimal}, already a reference
 * type: the list's own {@code add}, {@code get} and {@code set} are the
 * clause's element operations, and a second {@code void add(BigDecimal)}
 * beside the list's {@code boolean add(BigDecimal)} cannot be declared (JLS
 * 8.4.8.3). The array operations of the clause are here.
 */
public class BigDecimalSeq extends java.util.ArrayList<java.math.BigDecimal> {
    private static final long serialVersionUID = 1L;

    /** An empty sequence with room for {@code initialCapacity} elements. */
    public BigDecimalSeq(int initialCapacity) {
        super(initialCapacity);
    }

    /** A sequence holding the elements of {@code array}. */
    public BigDecimalSeq(java.math.BigDecimal[] array) {
        super(java.util.Arrays.asList(array));
    }

    /** Appends every element of {@code elements}; whether any was added. */
    public boolean add(java.math.BigDecimal[] elements) {
        return add(elements, 0, elements.length);
    }

    /** Appends the elements of {@code elements} from {@code index} on. */
    public boolean add(java.math.BigDecimal[] elements, int index) {
        return add(elements, index, elements.length - index);
    }

    /** Appends {@code count} elements of {@code elements} from {@code index}. */
    public boolean add(java.math.BigDecimal[] elements, int index, int count) {
        if (index < 0 || count < 0 || index > elements.length - count) {
            throw new IndexOutOfBoundsException(
                "index " + index + ", count " + count + ", length " + elements.length);
        }
        return addAll(java.util.Arrays.asList(elements).subList(index, index + count));
    }

    /**
     * Replaces {@code length} elements from {@code dstIndex} with those of
     * {@code elements} from {@code srcIndex}.
     */
    public void set(int dstIndex, java.math.BigDecimal[] elements, int srcIndex, int length) {
        if (length < 0 || dstIndex < 0 || dstIndex > size() - length) {
            throw new IndexOutOfBoundsException(
                "dstIndex " + dstIndex + ", length " + length + ", size " + size());
        }
        for (int i = 0; i < length; i++) {
            set(dstIndex + i, elements[srcIndex + i]);
        }
    }
}
