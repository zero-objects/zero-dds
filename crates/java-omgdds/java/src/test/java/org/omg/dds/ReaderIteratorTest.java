// SPDX-License-Identifier: Apache-2.0
package org.omg.dds;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.omg.dds.pub.DataWriter;
import org.omg.dds.sub.DataReader;
import org.omg.dds.sub.Sample;
import org.omg.dds.sub.SampleState;
import org.zerodds.internal.InProcessBus;

import java.util.NoSuchElementException;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;
import static org.omg.dds.ReaderRig.Keyed;
import static org.omg.dds.ReaderRig.sample;

/**
 * DDS-Java-PSM 1.0 §7.7.2 and §7.7.3: the {@code Sample.Iterator} that read and take return:
 * a read-only {@code ListIterator} that is {@code Closeable} (the mapping of return_loan).
 */
class ReaderIteratorTest {

    private static int dom = 1050;

    private static int nextDom() {
        return dom++;
    }

    @BeforeEach
    void setUp() {
        InProcessBus.instance().reset();
    }

    @AfterEach
    void tearDown() {
        InProcessBus.instance().reset();
    }

    private static DataReader<Keyed> readerWithThreeSamples(ReaderRig rig) throws Exception {
        DataReader<Keyed> reader = rig.keepAllReader();
        DataWriter<Keyed> writer = rig.writer();
        writer.write(sample(1, 0));
        writer.write(sample(1, 1));
        writer.write(sample(1, 2));
        return reader;
    }

    @Test
    void theIteratorWalksTheSamplesInBothDirections() throws Exception {
        ReaderRig rig = new ReaderRig(nextDom());
        DataReader<Keyed> reader = readerWithThreeSamples(rig);
        try (Sample.Iterator<Keyed> it = reader.read()) {
            assertFalse(it.hasPrevious());
            assertEquals(0, it.nextIndex());
            assertEquals(-1, it.previousIndex());
            assertEquals(0, it.next().getData().seq());
            assertEquals(1, it.next().getData().seq());
            assertEquals(2, it.nextIndex());
            assertEquals(2, it.next().getData().seq());
            assertFalse(it.hasNext());
            assertThrows(NoSuchElementException.class, it::next);
            // previous returns the sample before the cursor
            assertEquals(2, it.previous().getData().seq());
            assertEquals(1, it.previous().getData().seq());
            assertEquals(0, it.previous().getData().seq());
            assertFalse(it.hasPrevious());
            assertThrows(NoSuchElementException.class, it::previous);
        }
    }

    @Test
    void theIteratorIsReadOnly() throws Exception {
        ReaderRig rig = new ReaderRig(nextDom());
        DataReader<Keyed> reader = readerWithThreeSamples(rig);
        try (Sample.Iterator<Keyed> it = reader.read()) {
            Sample<Keyed> first = it.next();
            assertThrows(UnsupportedOperationException.class, it::remove);
            assertThrows(UnsupportedOperationException.class, () -> it.set(first));
            assertThrows(UnsupportedOperationException.class, () -> it.add(first));
            // nothing was changed
            assertSame(first, it.previous());
            assertEquals(3, count(reader.read()));
        }
    }

    private static int count(Sample.Iterator<Keyed> it) throws Exception {
        try (Sample.Iterator<Keyed> iterator = it) {
            int n = 0;
            while (iterator.hasNext()) {
                iterator.next();
                n++;
            }
            return n;
        }
    }

    @Test
    void closeEndsTheLoanAndClosingAgainIsHarmless() throws Exception {
        ReaderRig rig = new ReaderRig(nextDom());
        DataReader<Keyed> reader = readerWithThreeSamples(rig);
        Sample.Iterator<Keyed> it = reader.read();
        Sample<Keyed> first = it.next();
        it.close();
        it.close(); // Closeable: closing again has no effect
        // the samples are copies the application owns: they stay readable
        assertEquals(0, first.getData().seq());
        // and the reader is not affected by the loan: it still holds the samples
        assertEquals(3, reader.read(new java.util.ArrayList<>()).size());
    }

    @Test
    void takeReturnsTheSamplesInTheIteratorAndRemovesThemFromTheReader() throws Exception {
        ReaderRig rig = new ReaderRig(nextDom());
        DataReader<Keyed> reader = readerWithThreeSamples(rig);
        try (Sample.Iterator<Keyed> it = reader.take()) {
            assertEquals(0, it.next().getData().seq());
            assertEquals(SampleState.NOT_READ, it.previous().getSampleState());
        }
        assertTrue(Qos.read(reader).isEmpty());
        // the iterator forms with a selector and with a maximum
        DataWriter<Keyed> writer = rig.writer();
        writer.write(sample(2, 0));
        writer.write(sample(2, 1));
        try (Sample.Iterator<Keyed> it = reader.take(1)) {
            assertEquals(0, it.next().getData().seq());
            assertFalse(it.hasNext());
        }
        try (Sample.Iterator<Keyed> it = reader.take(reader.select().maxSamples(5))) {
            assertEquals(1, it.next().getData().seq());
            assertFalse(it.hasNext());
        }
    }

    @Test
    void readSelectorAndIntFormsReturnTheSameSamplesAsTheListForm() throws Exception {
        ReaderRig rig = new ReaderRig(nextDom());
        DataReader<Keyed> reader = readerWithThreeSamples(rig);
        assertEquals(2, count(reader.read(2)));
        assertEquals(3, count(reader.read(reader.select())));
        assertEquals(2, count(reader.read(reader.select().maxSamples(2))));
        assertEquals(3, Qos.read(reader).size());
    }
}
