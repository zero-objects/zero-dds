// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
package org.omg.type;

/**
 * A sequence of IDL long, int32, unsigned long and uint32: IDL4 to Java Language Mapping 1.0
 * §7.2.4.2.1.1, Table 7.4.
 *
 * <p>The clause lists this as an interface extending
 * {@code java.util.List<Integer>} with two constructors; an interface has no
 * constructor, so it is a class that is a {@code List<Integer>}. Its elements
 * are held as {@code int} values. The clause's {@code int get(int)} cannot
 * be declared beside the {@code Integer get(int)} of {@code List<Integer>} (JLS
 * 8.4.8.3: one signature, two return types); the list's own {@code get}
 * answers it, and {@code int x = seq.get(i);} compiles unchanged through
 * unboxing.
 */
public class IntegerSeq extends java.util.AbstractList<Integer>
        implements java.util.RandomAccess, java.io.Serializable {
    private static final long serialVersionUID = 1L;

    private int[] elements;
    private int size;

    /** An empty sequence with room for {@code initialCapacity} elements. */
    public IntegerSeq(int initialCapacity) {
        if (initialCapacity < 0) {
            throw new IllegalArgumentException("initialCapacity " + initialCapacity);
        }
        this.elements = new int[initialCapacity];
    }

    /** A sequence holding a copy of {@code array}. */
    public IntegerSeq(int[] array) {
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
    public Integer get(int index) {
        within(index);
        return elements[index];
    }

    @Override
    public Integer set(int index, Integer element) {
        within(index);
        int previous = elements[index];
        elements[index] = element.intValue();
        return previous;
    }

    @Override
    public void add(int index, Integer element) {
        add(index, element.intValue());
    }

    @Override
    public Integer remove(int index) {
        within(index);
        int previous = elements[index];
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
    public void add(int element) {
        add(size, element);
    }

    /** Inserts {@code element} at {@code index}. */
    public void add(int index, int element) {
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
    public boolean add(int[] elements) {
        return add(elements, 0, elements.length);
    }

    /** Appends the elements of {@code elements} from {@code index} on. */
    public boolean add(int[] elements, int index) {
        return add(elements, index, elements.length - index);
    }

    /** Appends {@code count} elements of {@code elements} from {@code index}. */
    public boolean add(int[] elements, int index, int count) {
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
    public void set(int index, int element) {
        within(index);
        elements[index] = element;
    }

    /**
     * Replaces {@code length} elements from {@code dstIndex} with those of
     * {@code elements} from {@code srcIndex}.
     */
    public void set(int dstIndex, int[] elements, int srcIndex, int length) {
        if (length < 0 || dstIndex < 0 || dstIndex > size - length) {
            throw new IndexOutOfBoundsException(
                "dstIndex " + dstIndex + ", length " + length + ", size " + size);
        }
        System.arraycopy(elements, srcIndex, this.elements, dstIndex, length);
    }

    /** The elements, in {@code array} if it has room, in a new array if not. */
    public int[] toArray(int[] array) {
        int[] out = array.length >= size ? array : new int[size];
        System.arraycopy(elements, 0, out, 0, size);
        return out;
    }
}
