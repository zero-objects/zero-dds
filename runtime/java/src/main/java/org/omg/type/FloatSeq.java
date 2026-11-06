// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
package org.omg.type;

/**
 * A sequence of IDL float: IDL4 to Java Language Mapping 1.0
 * §7.2.4.2.1.1, Table 7.4.
 *
 * <p>The clause lists this as an interface extending
 * {@code java.util.List<Float>} with two constructors; an interface has no
 * constructor, so it is a class that is a {@code List<Float>}. Its elements
 * are held as {@code float} values. The clause's {@code float get(int)} cannot
 * be declared beside the {@code Float get(int)} of {@code List<Float>} (JLS
 * 8.4.8.3: one signature, two return types); the list's own {@code get}
 * answers it, and {@code float x = seq.get(i);} compiles unchanged through
 * unboxing.
 */
public class FloatSeq extends java.util.AbstractList<Float>
        implements java.util.RandomAccess, java.io.Serializable {
    private static final long serialVersionUID = 1L;

    private float[] elements;
    private int size;

    /** An empty sequence with room for {@code initialCapacity} elements. */
    public FloatSeq(int initialCapacity) {
        if (initialCapacity < 0) {
            throw new IllegalArgumentException("initialCapacity " + initialCapacity);
        }
        this.elements = new float[initialCapacity];
    }

    /** A sequence holding a copy of {@code array}. */
    public FloatSeq(float[] array) {
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
    public Float get(int index) {
        within(index);
        return elements[index];
    }

    @Override
    public Float set(int index, Float element) {
        within(index);
        float previous = elements[index];
        elements[index] = element.floatValue();
        return previous;
    }

    @Override
    public void add(int index, Float element) {
        add(index, element.floatValue());
    }

    @Override
    public Float remove(int index) {
        within(index);
        float previous = elements[index];
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
    public void add(float element) {
        add(size, element);
    }

    /** Inserts {@code element} at {@code index}. */
    public void add(int index, float element) {
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
    public boolean add(float[] elements) {
        return add(elements, 0, elements.length);
    }

    /** Appends the elements of {@code elements} from {@code index} on. */
    public boolean add(float[] elements, int index) {
        return add(elements, index, elements.length - index);
    }

    /** Appends {@code count} elements of {@code elements} from {@code index}. */
    public boolean add(float[] elements, int index, int count) {
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
    public void set(int index, float element) {
        within(index);
        elements[index] = element;
    }

    /**
     * Replaces {@code length} elements from {@code dstIndex} with those of
     * {@code elements} from {@code srcIndex}.
     */
    public void set(int dstIndex, float[] elements, int srcIndex, int length) {
        if (length < 0 || dstIndex < 0 || dstIndex > size - length) {
            throw new IndexOutOfBoundsException(
                "dstIndex " + dstIndex + ", length " + length + ", size " + size);
        }
        System.arraycopy(elements, srcIndex, this.elements, dstIndex, length);
    }

    /** The elements, in {@code array} if it has room, in a new array if not. */
    public float[] toArray(float[] array) {
        float[] out = array.length >= size ? array : new float[size];
        System.arraycopy(elements, 0, out, 0, size);
        return out;
    }
}
