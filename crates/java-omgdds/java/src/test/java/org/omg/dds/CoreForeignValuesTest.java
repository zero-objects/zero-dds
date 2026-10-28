// SPDX-License-Identifier: Apache-2.0
package org.omg.dds;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.omg.dds.core.InstanceHandle;
import org.omg.dds.core.ModifiableInstanceHandle;
import org.omg.dds.core.ServiceEnvironment;
import org.omg.dds.core.Time;
import org.omg.dds.domain.DomainParticipant;
import org.omg.dds.pub.DataWriter;
import org.omg.dds.sub.DataReader;
import org.omg.dds.sub.InstanceState;
import org.omg.dds.sub.Sample;
import org.omg.dds.sub.SampleState;
import org.omg.dds.sub.ViewState;
import org.omg.dds.topic.Topic;
import org.omg.dds.type.Key;
import org.zerodds.internal.InProcessBus;

import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.TimeUnit;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * DDS-Java-PSM 1.0 §7.2.2: a value type or a sample of another implementation
 * is accepted by the operations of a ZeroDDS DataWriter. The foreign classes
 * are written against the abstract classes and interfaces of the OMG API only.
 */
class CoreForeignValuesTest {

    private static final int DOMAIN = 1502;

    /** A keyed sample type. */
    public static final class Item {
        @Key
        public int id;
        public int value;

        public Item() {}

        public Item(int id, int value) {
            this.id = id;
            this.value = value;
        }
    }

    /** A ModifiableInstanceHandle of another implementation. */
    static final class ForeignModifiableHandle extends ModifiableInstanceHandle {
        private static final long serialVersionUID = 1L;
        private boolean nil;
        private transient InstanceHandle lastCopied;

        ForeignModifiableHandle(boolean nil) {
            this.nil = nil;
        }

        @Override
        public void copyFrom(InstanceHandle src) {
            this.nil = src.isNil();
            this.lastCopied = src;
        }

        @Override
        public InstanceHandle immutableCopy() {
            return new CoreValuesTest.ForeignHandle(nil ? 0 : 1);
        }

        @Override
        public boolean isNil() {
            return nil;
        }

        @Override
        public ModifiableInstanceHandle modifiableCopy() {
            return new ForeignModifiableHandle(nil);
        }

        @Override
        public int compareTo(InstanceHandle other) {
            return 0;
        }

        @Override
        public ServiceEnvironment getEnvironment() {
            return null;
        }

        @Override
        public boolean equals(Object o) {
            return o instanceof ForeignModifiableHandle && ((ForeignModifiableHandle) o).nil == nil;
        }

        @Override
        public int hashCode() {
            return nil ? 0 : 1;
        }
    }

    /** A Sample as the DataReader of another implementation returns it. */
    static final class ForeignSample implements Sample<Item> {
        private static final long serialVersionUID = 1L;
        private final Item data;
        private final Time timestamp;
        private final ModifiableInstanceHandle handle;

        ForeignSample(Item data, Time timestamp, ModifiableInstanceHandle handle) {
            this.data = data;
            this.timestamp = timestamp;
            this.handle = handle;
        }

        @Override
        public Item getData() {
            return data;
        }

        @Override
        public SampleState getSampleState() {
            return SampleState.NOT_READ;
        }

        @Override
        public ViewState getViewState() {
            return ViewState.NEW;
        }

        @Override
        public InstanceState getInstanceState() {
            return InstanceState.ALIVE;
        }

        @Override
        public Time getSourceTimestamp() {
            return timestamp;
        }

        @Override
        public ModifiableInstanceHandle getInstanceHandle() {
            return handle;
        }

        @Override
        public ModifiableInstanceHandle getPublicationHandle() {
            return handle;
        }

        @Override
        public int getDisposedGenerationCount() {
            return 0;
        }

        @Override
        public int getNoWritersGenerationCount() {
            return 0;
        }

        @Override
        public int getSampleRank() {
            return 0;
        }

        @Override
        public int getGenerationRank() {
            return 0;
        }

        @Override
        public int getAbsoluteGenerationRank() {
            return 0;
        }

        @Override
        public Sample<Item> clone() {
            return new ForeignSample(data, timestamp, handle);
        }

        @Override
        public ServiceEnvironment getEnvironment() {
            return null;
        }
    }

    private DomainParticipant participant;
    private DataWriter<Item> writer;
    private DataReader<Item> reader;

    @BeforeEach
    void setUp() throws Exception {
        InProcessBus.instance().reset();
        participant = Qos.factory().createParticipant(DOMAIN);
        Topic<Item> topic = participant.createTopic("CoreForeignItems", Item.class);
        reader = participant.createSubscriber().createDataReader(topic, Qos.readerKeepAll());
        writer = participant.createPublisher().createDataWriter(topic);
    }

    @AfterEach
    void tearDown() throws Exception {
        participant.close();
        InProcessBus.instance().reset();
    }

    private InstanceState stateOf(int id) {
        InstanceState state = null;
        for (Sample<Item> sample : reader.read(new ArrayList<Sample<Item>>())) {
            Item item = sample.getData();
            // a sample without data (dispose, unregister) carries no key: use the handle
            if (item != null && item.id == id) {
                state = sample.getInstanceState();
            }
        }
        return state;
    }

    @Test
    void aSampleOfAnotherImplementationIsWrittenWithItsTimestampAndHandle() throws Exception {
        long nanos = 1_700_000_000_123_000_000L;
        ForeignSample foreign = new ForeignSample(new Item(1, 10),
                new CoreValuesTest.ForeignTime(nanos), new ForeignModifiableHandle(true));
        // write: the data, the foreign (nil) handle and the foreign timestamp
        writer.write(foreign.getData(), foreign.getInstanceHandle(), foreign.getSourceTimestamp());
        List<Sample<Item>> taken = reader.take(new ArrayList<Sample<Item>>());
        assertEquals(1, taken.size());
        assertEquals(10, taken.get(0).getData().value);
        assertEquals(1_700_000_000_123L, taken.get(0).getSourceTimestamp().getTime(TimeUnit.MILLISECONDS));
    }

    @Test
    void aSampleOfAnotherImplementationIsDisposedAndUnregistered() throws Exception {
        ForeignSample one = new ForeignSample(new Item(1, 10),
                new CoreValuesTest.ForeignTime(5_000_000_000L), new ForeignModifiableHandle(true));
        ForeignSample two = new ForeignSample(new Item(2, 20),
                new CoreValuesTest.ForeignTime(6_000_000_000L), new ForeignModifiableHandle(true));
        writer.write(one.getData());
        writer.write(two.getData());
        writer.dispose(one.getInstanceHandle(), one.getData(), one.getSourceTimestamp());
        writer.unregisterInstance(two.getInstanceHandle(), two.getData(), two.getSourceTimestamp());
        assertEquals(InstanceState.NOT_ALIVE_DISPOSED, stateOf(1));
        assertNotNull(stateOf(2));
    }

    @Test
    void aTimeOfAnotherImplementationIsTheSourceTimestamp() throws Exception {
        writer.write(new Item(3, 30), new CoreValuesTest.ForeignTime(7_000_000_000L));
        List<Sample<Item>> taken = reader.take(new ArrayList<Sample<Item>>());
        assertEquals(1, taken.size());
        assertEquals(7000, taken.get(0).getSourceTimestamp().getTime(TimeUnit.MILLISECONDS));
    }

    @Test
    void aForeignHandleThatNamesNoInstanceOfTheWriterIsABadParameter() throws Exception {
        writer.write(new Item(4, 40));
        InstanceHandle strange = new CoreValuesTest.ForeignHandle(9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9);
        // DDS 1.4 2.2.2.4.2.11: an invalid handle is BAD_PARAMETER (Table 7.1: IllegalArgumentException),
        // not a ClassCastException of the handle type
        assertThrows(IllegalArgumentException.class, () -> writer.dispose(strange));
        assertThrows(IllegalArgumentException.class, () -> writer.unregisterInstance(strange));
        assertThrows(IllegalArgumentException.class, () -> writer.write(new Item(4, 41), strange));
        assertThrows(IllegalArgumentException.class,
                () -> writer.dispose(strange, new Item(4, 41)));
    }

    @Test
    void aNilHandleOfAnotherImplementationNamesTheInstanceOfTheSample() throws Exception {
        writer.write(new Item(5, 50));
        InstanceHandle nil = new CoreValuesTest.ForeignHandle(0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0);
        writer.write(new Item(5, 51), nil);
        writer.dispose(nil, new Item(5, 51));
        assertEquals(InstanceState.NOT_ALIVE_DISPOSED, stateOf(5));
    }

    @Test
    void lookupInstanceFillsAModifiableHandleOfAnotherImplementation() throws Exception {
        InstanceHandle registered = writer.registerInstance(new Item(6, 60));
        assertTrue(!registered.isNil());
        ForeignModifiableHandle target = new ForeignModifiableHandle(true);
        ModifiableInstanceHandle result = writer.lookupInstance(target, new Item(6, 0));
        assertEquals(target, result);
        assertTrue(!target.isNil());
        assertEquals(registered, target.lastCopied);
        // an instance that was not registered gives the nil handle
        ForeignModifiableHandle other = new ForeignModifiableHandle(false);
        writer.lookupInstance(other, new Item(99, 0));
        assertTrue(other.isNil());
    }
}
