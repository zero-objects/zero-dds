// SPDX-License-Identifier: Apache-2.0
package org.zerodds.wire;

import org.junit.jupiter.api.Test;

import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collection;
import java.util.LinkedHashSet;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertThrows;

/**
 * What the Clause 9 codec of {@code idlc --java-mapping psm} asks of a
 * sequence that is an array or a {@code Collection}
 * (DDS-Java-PSM 1.0 §9.1.2).
 */
class SequencesTest {

    @Test
    void anArrayIsCountedIndexedAndFilledInPlace() {
        int[] v = Sequences.alloc(new int[0], 3);
        assertEquals(3, Sequences.count(v));
        Sequences.put(v, 0, 7);
        Sequences.put(v, 2, 9);
        assertEquals(7, Sequences.at(v, 0));
        assertEquals(0, Sequences.at(v, 1));
        assertArrayEquals(new int[] {7, 0}, Sequences.first(v, 2));

        String[] names = Sequences.alloc(new String[0], 2);
        Sequences.put(names, 1, "b");
        assertEquals("b", Sequences.at(names, 1));
        assertEquals(2, Sequences.count(names));
        assertEquals(String[].class, names.getClass());
    }

    @Test
    void everyPrimitiveArrayHasItsOverloads() {
        boolean[] b = Sequences.alloc(new boolean[0], 1);
        Sequences.put(b, 0, true);
        assertEquals(true, Sequences.at(b, 0));
        byte[] y = Sequences.alloc(new byte[0], 1);
        Sequences.put(y, 0, (byte) -3);
        assertEquals((byte) -3, Sequences.at(y, 0));
        char[] c = Sequences.alloc(new char[0], 1);
        Sequences.put(c, 0, 'x');
        assertEquals('x', Sequences.at(c, 0));
        short[] s = Sequences.alloc(new short[0], 1);
        Sequences.put(s, 0, (short) 5);
        assertEquals((short) 5, Sequences.at(s, 0));
        long[] l = Sequences.alloc(new long[0], 1);
        Sequences.put(l, 0, 1L << 40);
        assertEquals(1L << 40, Sequences.at(l, 0));
        float[] f = Sequences.alloc(new float[0], 1);
        Sequences.put(f, 0, 1.5f);
        assertEquals(1.5f, Sequences.at(f, 0));
        double[] d = Sequences.alloc(new double[0], 1);
        Sequences.put(d, 0, 2.5);
        assertEquals(2.5, Sequences.at(d, 0));
    }

    @Test
    void aCollectionIsFilledInOrderAndCountedByItsSize() {
        Collection<Integer> v = Sequences.alloc(new ArrayList<Integer>(), 3);
        assertEquals(0, Sequences.count(v), "room for three, none yet");
        Sequences.put(v, 0, 10);
        Sequences.put(v, 1, 20);
        assertEquals(2, Sequences.count(v));
        assertEquals(20, Sequences.at(v, 1));
        assertEquals(Arrays.asList(10), new ArrayList<>(Sequences.first(v, 1)));
        assertEquals(Arrays.asList(10, 20), new ArrayList<>(Sequences.first(v, 5)));
        // A gap is a defect of the caller, not a hole in the sequence.
        assertThrows(IllegalStateException.class, () -> Sequences.put(v, 5, 1));
        assertThrows(IndexOutOfBoundsException.class, () -> Sequences.at(v, 2));
    }

    @Test
    void aCollectionThatIsNotAListIsWalkedByItsIterator() {
        Collection<String> v = new LinkedHashSet<>(Arrays.asList("a", "b", "c"));
        assertEquals("c", Sequences.at(v, 2));
        assertEquals(3, Sequences.count(v));
    }

    @Test
    @SuppressWarnings("unchecked")
    void anArrayOfAParameterizedTypeIsCreatedFromItsErasure() {
        Collection<Integer>[] rows = Sequences.newArray(Collection.class, 3);
        assertEquals(3, rows.length);
        rows[0] = new ArrayList<>(Arrays.asList(1, 2));
        assertEquals(2, rows[0].size());
        Collection<Integer>[][] grid = Sequences.newArray(Collection.class, 2, 2);
        assertEquals(2, grid[1].length);
        // An array of such arrays copies its kind.
        Collection<Integer>[] more = Sequences.alloc(rows, 4);
        assertEquals(4, more.length);
        assertEquals(Collection[].class, more.getClass());
    }
}
