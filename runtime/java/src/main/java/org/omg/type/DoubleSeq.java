// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
package org.omg.type;

/**
 * A sequence of IDL double: IDL4 to Java Language Mapping 1.0
 * §7.2.4.2.1.1, Table 7.4.
 *
 * <p>The clause lists this as an interface extending
 * {@code java.util.List<Double>} with two constructors; an interface has no
 * constructor, so it is a class that is a {@code List<Double>}. Its elements
 * are held as {@code double} values. The clause's {@code double get(int)} cannot
 * be declared beside the {@code Double get(int)} of {@code List<Double>} (JLS
 * 8.4.8.3: one signature, two return types); the list's own {@code get}
 * answers it, and {@code double x = seq.get(i);} compiles unchanged through
 * unboxing.
 */
public class DoubleSeq extends java.util.AbstractList<Double>
        implements java.util.RandomAccess, java.io.Serializable {
    private static final long serialVersionUID = 1L;

    private double[] elements;
    private int size;

    /** An empty sequence with room for {@code initialCapacity} elements. */
    public DoubleSeq(int initialCapacity) {
        if (initialCapacity < 0) {
            throw new IllegalArgumentException("initialCapacity " + initialCapacity);
        }
        this.elements = new double[initialCapacity];
    }

    /** A sequence holding a copy of {@code array}. */
    public DoubleSeq(double[] array) {
        this.elements = array.clone();
        this.size = array.length;
    }

    private void ensure(int capacity) {
        if (capacity > elements.length) {
            this.elements = java.util.Arrays.copyOf(
                elements, Math.max(capacity, elements.length * 2 + 1));
        }
    }

    private void within(int index) {
        if (index < 0 || index >= size) {
            throw new IndexOutOfBoundsException("index " + index + ", size " + size);
        }
    }

    @Override
    public int size() {
        return size;
    }

    @Override
    public Double get(int index) {
        within(index);
        return elements[index];
    }

    @Override
    public Double set(int index, Double element) {
        within(index);
        double previous = elements[index];
        elements[index] = element.doubleValue();
        return previous;
    }

    @Override
    public void add(int index, Double element) {
        add(index, element.doubleValue());
    }

    @Override
    public Double remove(int index) {
        within(index);
        double previous = elements[index];
        System.arraycopy(elements, index + 1, elements, index, size - index - 1);
        size--;
        modCount++;
        return previous;
    }

    @Override
    public void clear() {
        size = 0;
        modCount++;
    }

    /** Appends {@code element}. */
    public void add(double element) {
        add(size, element);
    }

    /** Inserts {@code element} at {@code index}. */
    public void add(int index, double element) {
        if (index < 0 || index > size) {
            throw new IndexOutOfBoundsException("index " + index + ", size " + size);
        }
        ensure(size + 1);
        System.arraycopy(elements, index, elements, index + 1, size - index);
        elements[index] = element;
        size++;
        modCount++;
    }

    /** Appends every element of {@code elements}; whether any was added. */
    public boolean add(double[] elements) {
        return add(elements, 0, elements.length);
    }

    /** Appends the elements of {@code elements} from {@code index} on. */
    public boolean add(double[] elements, int index) {
        return add(elements, index, elements.length - index);
    }

    /** Appends {@code count} elements of {@code elements} from {@code index}. */
    public boolean add(double[] elements, int index, int count) {
        if (index < 0 || count < 0 || index > elements.length - count) {
            throw new IndexOutOfBoundsException(
                "index " + index + ", count " + count + ", length " + elements.length);
        }
        ensure(size + count);
        System.arraycopy(elements, index, this.elements, size, count);
        size += count;
        modCount++;
        return count > 0;
    }

    /** Replaces the element at {@code index}. */
    public void set(int index, double element) {
        within(index);
        elements[index] = element;
    }

    /**
     * Replaces {@code length} elements from {@code dstIndex} with those of
     * {@code elements} from {@code srcIndex}.
     */
    public void set(int dstIndex, double[] elements, int srcIndex, int length) {
        if (length < 0 || dstIndex < 0 || dstIndex > size - length) {
            throw new IndexOutOfBoundsException(
                "dstIndex " + dstIndex + ", length " + length + ", size " + size);
        }
        System.arraycopy(elements, srcIndex, this.elements, dstIndex, length);
    }

    /** The elements, in {@code array} if it has room, in a new array if not. */
    public double[] toArray(double[] array) {
        double[] out = array.length >= size ? array : new double[size];
        System.arraycopy(elements, 0, out, 0, size);
        return out;
    }
}
