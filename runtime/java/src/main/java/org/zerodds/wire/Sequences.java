// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
package org.zerodds.wire;

import java.lang.reflect.Array;
import java.util.ArrayList;
import java.util.Collection;
import java.util.Iterator;
import java.util.List;

/**
 * What a generated codec does to a sequence without knowing what holds it.
 *
 * <p>Under the DDS-Java-PSM 1.0 Clause 9 mapping ({@code --java-mapping psm}
 * of {@code idlc}) an unbounded {@code sequence<T>} is a
 * {@code java.util.Collection<E>}, and a bounded sequence or an array is a
 * Java array (§9.1.2). The generated codec is the same walk for both, so the
 * five operations it needs are overloaded here instead of spelled twice: the
 * static type of the argument chooses the array or the {@code Collection}
 * form, and nothing is decided at run time that javac could decide.
 *
 * <ul>
 *   <li>{@code count(v)}: the length;</li>
 *   <li>{@code at(v, i)}: the element at {@code i}; a {@code Collection} that
 *       is not a {@link List} is walked by its iterator;</li>
 *   <li>{@code alloc(like, n)}: a new empty container of the kind {@code like}
 *       is, with room for {@code n} elements. An array has {@code n} default
 *       elements, a {@code Collection} none: it is filled by {@code put};</li>
 *   <li>{@code put(v, i, e)}: stores the element for position {@code i}; for a
 *       {@code Collection} that is appending, so the positions must come in
 *       order and a gap is an {@link IllegalStateException};</li>
 *   <li>{@code first(v, n)}: the first {@code n} elements, in a container of
 *       the same kind.</li>
 * </ul>
 */
public final class Sequences {

    private Sequences() {}

    /**
     * A new array of {@code component} with the given dimensions, typed by the
     * assignment: {@code Collection<Integer>[] v = Sequences.newArray(Collection.class, 3)}.
     * Java refuses {@code new Collection<Integer>[3]}; the array of a
     * parameterized type is created from its erasure here.
     */
    @SuppressWarnings("unchecked")
    public static <A> A newArray(Class<?> component, int... lengths) {
        return (A) Array.newInstance(component, lengths);
    }

    /** The identity; marks the container a codec indexes into. */
    public static <T> T view(T value) {
        return value;
    }

    // -- Collection ------------------------------------------------------

    public static int count(Collection<?> v) {
        return v.size();
    }

    public static <E> E at(Collection<E> v, int index) {
        if (index < 0 || index >= v.size()) {
            throw new IndexOutOfBoundsException("element " + index + " of " + v.size());
        }
        if (v instanceof List) {
            return ((List<E>) v).get(index);
        }
        Iterator<E> walk = v.iterator();
        for (int skipped = 0; skipped < index; skipped++) {
            walk.next();
        }
        return walk.next();
    }

    public static <E> void put(Collection<E> v, int index, E element) {
        if (index != v.size()) {
            throw new IllegalStateException("a Collection is filled in order: element " + index
                    + " after " + v.size());
        }
        v.add(element);
    }

    public static <E> Collection<E> alloc(Collection<E> like, int count) {
        return new ArrayList<E>(count);
    }

    public static <E> Collection<E> first(Collection<E> v, int count) {
        List<E> kept = new ArrayList<E>(Math.max(0, Math.min(count, v.size())));
        Iterator<E> walk = v.iterator();
        for (int taken = 0; taken < count && walk.hasNext(); taken++) {
            kept.add(walk.next());
        }
        return kept;
    }

    // -- reference arrays --------------------------------------------------

    public static int count(Object[] v) {
        return v.length;
    }

    public static <T> T at(T[] v, int index) {
        return v[index];
    }

    public static <T> void put(T[] v, int index, T element) {
        v[index] = element;
    }

    @SuppressWarnings("unchecked")
    public static <T> T[] alloc(T[] like, int count) {
        return (T[]) Array.newInstance(like.getClass().getComponentType(), count);
    }

    public static <T> T[] first(T[] v, int count) {
        return java.util.Arrays.copyOf(v, count);
    }

    // -- boolean[] ---------------------------------------------------------------

    public static int count(boolean[] v) {
        return v.length;
    }

    public static boolean at(boolean[] v, int index) {
        return v[index];
    }

    public static void put(boolean[] v, int index, boolean element) {
        v[index] = element;
    }

    public static boolean[] alloc(boolean[] like, int count) {
        return new boolean[count];
    }

    public static boolean[] first(boolean[] v, int count) {
        return java.util.Arrays.copyOf(v, count);
    }

    // -- byte[] ---------------------------------------------------------------

    public static int count(byte[] v) {
        return v.length;
    }

    public static byte at(byte[] v, int index) {
        return v[index];
    }

    public static void put(byte[] v, int index, byte element) {
        v[index] = element;
    }

    public static byte[] alloc(byte[] like, int count) {
        return new byte[count];
    }

    public static byte[] first(byte[] v, int count) {
        return java.util.Arrays.copyOf(v, count);
    }

    // -- char[] ---------------------------------------------------------------

    public static int count(char[] v) {
        return v.length;
    }

    public static char at(char[] v, int index) {
        return v[index];
    }

    public static void put(char[] v, int index, char element) {
        v[index] = element;
    }

    public static char[] alloc(char[] like, int count) {
        return new char[count];
    }

    public static char[] first(char[] v, int count) {
        return java.util.Arrays.copyOf(v, count);
    }

    // -- short[] ---------------------------------------------------------------

    public static int count(short[] v) {
        return v.length;
    }

    public static short at(short[] v, int index) {
        return v[index];
    }

    public static void put(short[] v, int index, short element) {
        v[index] = element;
    }

    public static short[] alloc(short[] like, int count) {
        return new short[count];
    }

    public static short[] first(short[] v, int count) {
        return java.util.Arrays.copyOf(v, count);
    }

    // -- int[] ---------------------------------------------------------------

    public static int count(int[] v) {
        return v.length;
    }

    public static int at(int[] v, int index) {
        return v[index];
    }

    public static void put(int[] v, int index, int element) {
        v[index] = element;
    }

    public static int[] alloc(int[] like, int count) {
        return new int[count];
    }

    public static int[] first(int[] v, int count) {
        return java.util.Arrays.copyOf(v, count);
    }

    // -- long[] ---------------------------------------------------------------

    public static int count(long[] v) {
        return v.length;
    }

    public static long at(long[] v, int index) {
        return v[index];
    }

    public static void put(long[] v, int index, long element) {
        v[index] = element;
    }

    public static long[] alloc(long[] like, int count) {
        return new long[count];
    }

    public static long[] first(long[] v, int count) {
        return java.util.Arrays.copyOf(v, count);
    }

    // -- float[] ---------------------------------------------------------------

    public static int count(float[] v) {
        return v.length;
    }

    public static float at(float[] v, int index) {
        return v[index];
    }

    public static void put(float[] v, int index, float element) {
        v[index] = element;
    }

    public static float[] alloc(float[] like, int count) {
        return new float[count];
    }

    public static float[] first(float[] v, int count) {
        return java.util.Arrays.copyOf(v, count);
    }

    // -- double[] ---------------------------------------------------------------

    public static int count(double[] v) {
        return v.length;
    }

    public static double at(double[] v, int index) {
        return v[index];
    }

    public static void put(double[] v, int index, double element) {
        v[index] = element;
    }

    public static double[] alloc(double[] like, int count) {
        return new double[count];
    }

    public static double[] first(double[] v, int count) {
        return java.util.Arrays.copyOf(v, count);
    }
}
