// SPDX-License-Identifier: Apache-2.0
package org.omg.dds;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.omg.dds.core.InstanceHandle;
import org.omg.dds.core.ModifiableInstanceHandle;
import org.omg.dds.core.PreconditionNotMetException;
import org.omg.dds.core.Time;
import org.omg.dds.domain.DomainParticipant;
import org.omg.dds.pub.DataWriter;
import org.omg.dds.sub.DataReader;
import org.omg.dds.sub.InstanceState;
import org.omg.dds.sub.Sample;
import org.omg.dds.topic.Topic;
import org.omg.dds.type.Key;
import org.zerodds.internal.InProcessBus;

import java.util.List;
import java.util.concurrent.TimeUnit;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotEquals;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertThrows;

/**
 * The operations of the DataWriter that take a source timestamp or an instance handle
 * (DDS 1.4 §2.2.2.4.2.5 to §2.2.2.4.2.14, DDS-Java-PSM 1.0 §7.6.2): what the reader sees of
 * them. Domain ids 1380 to 1399.
 */
class WriterHandleTest {

    /** A keyed type: the instance is the {@code id}. */
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

    private static final long STAMP = 1_700_000_000_000L;

    @BeforeEach
    void setUp() {
        InProcessBus.instance().reset();
    }

    @AfterEach
    void tearDown() {
        InProcessBus.instance().reset();
    }

    private static Time time(long millis) {
        return Qos.env().getSPI().newTime(millis, TimeUnit.MILLISECONDS);
    }

    /** A handle that is not nil and that no writer registered. */
    private static InstanceHandle foreignHandle() {
        byte[] bytes = new byte[16];
        java.util.Arrays.fill(bytes, (byte) 0x5A);
        return org.zerodds.core.Values.of(Qos.env(), bytes);
    }

    private static long millis(Sample<?> sample) {
        return sample.getSourceTimestamp().getTime(TimeUnit.MILLISECONDS);
    }

    @Test
    void registerInstanceTakesATimestampAndLookupInstanceFindsTheInstanceByItsKey() throws Exception {
        DomainParticipant participant = Qos.factory().createParticipant(1380);
        Topic<Item> topic = participant.createTopic("RegisterStamp", Item.class);
        DataWriter<Item> writer = participant.createPublisher().createDataWriter(topic);

        InstanceHandle one = writer.registerInstance(new Item(1, 0), time(STAMP));
        InstanceHandle two = writer.registerInstance(new Item(2, 0), STAMP, TimeUnit.MILLISECONDS);
        assertNotEquals(one, two);
        // the key names the instance, the other members do not
        assertEquals(one, writer.lookupInstance(new Item(1, 99)));
        assertEquals(two, writer.lookupInstance(new Item(2, 99)));
        assertEquals(Qos.env().getSPI().nilHandle(), writer.lookupInstance(new Item(3, 0)));

        ModifiableInstanceHandle holder = Qos.env().getSPI().newInstanceHandle();
        assertSame(holder, writer.lookupInstance(holder, new Item(2, 5)));
        assertEquals(two, holder);
        // registering again gives the same handle
        assertEquals(one, writer.registerInstance(new Item(1, 7)));
    }

    @Test
    void everyWriteOverloadCarriesItsSourceTimestampToTheSample() throws Exception {
        DomainParticipant participant = Qos.factory().createParticipant(1381);
        Topic<Item> topic = participant.createTopic("WriteStamp", Item.class);
        // an unregister disposes first when autodispose_unregistered_instances is true
        // (DDS 1.4 2.2.3.21); this test is about the instance state after the unregister alone
        DataWriter<Item> writer = participant.createPublisher().createDataWriter(topic,
                Qos.writer().withPolicy(
                        Qos.pf().WriterDataLifecycle().withAutDisposeUnregisteredInstances(false)));
        DataReader<Item> reader = participant.createSubscriber().createDataReader(topic,
                Qos.readerKeepAll());
        InstanceHandle one = writer.registerInstance(new Item(1, 0));

        writer.write(new Item(1, 1), time(STAMP + 1));
        writer.write(new Item(1, 2), STAMP + 2, TimeUnit.MILLISECONDS);
        writer.write(new Item(1, 3), one, time(STAMP + 3));
        writer.write(new Item(1, 4), one, STAMP + 4, TimeUnit.MILLISECONDS);
        writer.write(new Item(1, 5), one);

        List<Sample<Item>> samples = Qos.take(reader);
        assertEquals(5, samples.size());
        for (int i = 0; i < 4; i++) {
            assertEquals(i + 1, samples.get(i).getData().value);
            assertEquals(STAMP + i + 1, millis(samples.get(i)));
        }
        // write(sample, handle) is stamped with the time of the write
        assertEquals(5, samples.get(4).getData().value);
        assertEquals(true, millis(samples.get(4)) > STAMP + 4);
        for (Sample<Item> sample : samples) {
            assertEquals(one, sample.getInstanceHandle());
        }
    }

    @Test
    void anUnkeyedTypeIsWrittenWithTheNilHandle() throws Exception {
        DomainParticipant participant = Qos.factory().createParticipant(1382);
        Topic<byte[]> topic = participant.createTopic("NilHandle", byte[].class);
        DataWriter<byte[]> writer = participant.createPublisher().createDataWriter(topic);
        DataReader<byte[]> reader = participant.createSubscriber().createDataReader(topic,
                Qos.readerKeepAll());
        InstanceHandle nil = Qos.env().getSPI().nilHandle();

        writer.write(new byte[] {1}, nil);
        writer.write(new byte[] {2}, nil, time(STAMP));
        writer.write(new byte[] {3}, nil, STAMP + 1, TimeUnit.MILLISECONDS);
        List<Sample<byte[]>> samples = Qos.take(reader);
        assertEquals(3, samples.size());
        assertEquals(1, samples.get(0).getData()[0]);
        assertEquals(STAMP, millis(samples.get(1)));
        assertEquals(STAMP + 1, millis(samples.get(2)));
    }

    @Test
    void unregisterByHandleOrBySampleMakesTheReaderSeeNoWriters() throws Exception {
        DomainParticipant participant = Qos.factory().createParticipant(1383);
        Topic<Item> topic = participant.createTopic("UnregisterHandle", Item.class);
        // an unregister disposes first when autodispose_unregistered_instances is true
        // (DDS 1.4 2.2.3.21); this test is about the instance state after the unregister alone
        DataWriter<Item> writer = participant.createPublisher().createDataWriter(topic,
                Qos.writer().withPolicy(
                        Qos.pf().WriterDataLifecycle().withAutDisposeUnregisteredInstances(false)));
        DataReader<Item> reader = participant.createSubscriber().createDataReader(topic,
                Qos.readerKeepAll());
        InstanceHandle one = writer.registerInstance(new Item(1, 0));
        InstanceHandle two = writer.registerInstance(new Item(2, 0));
        writer.write(new Item(1, 10));
        writer.write(new Item(2, 20));

        writer.unregisterInstance(one);
        writer.unregisterInstance(Qos.env().getSPI().nilHandle(), new Item(2, 0));
        // the instance state is the one of the instance: all its samples report it
        boolean sawOne = false;
        boolean sawTwo = false;
        for (Sample<Item> sample : reader.read(new java.util.ArrayList<>())) {
            assertEquals(InstanceState.NOT_ALIVE_NO_WRITERS, sample.getInstanceState());
            sawOne |= sample.getInstanceHandle().equals(one);
            sawTwo |= sample.getInstanceHandle().equals(two);
        }
        assertEquals(true, sawOne);
        assertEquals(true, sawTwo);
        // both were unregistered: the writer has no instance any more
        assertEquals(Qos.env().getSPI().nilHandle(), writer.lookupInstance(new Item(1, 0)));
    }

    @Test
    void disposeByHandleOrBySampleMakesTheReaderSeeTheInstanceDisposed() throws Exception {
        DomainParticipant participant = Qos.factory().createParticipant(1384);
        Topic<Item> topic = participant.createTopic("DisposeHandle", Item.class);
        DataWriter<Item> writer = participant.createPublisher().createDataWriter(topic);
        DataReader<Item> reader = participant.createSubscriber().createDataReader(topic,
                Qos.readerKeepAll());
        InstanceHandle one = writer.registerInstance(new Item(1, 0));
        InstanceHandle two = writer.registerInstance(new Item(2, 0));
        writer.write(new Item(1, 10));
        writer.write(new Item(2, 20));

        writer.dispose(one, new Item(1, 0), time(STAMP));
        writer.dispose(Qos.env().getSPI().nilHandle(), new Item(2, 0), STAMP + 1, TimeUnit.MILLISECONDS);
        boolean sawOne = false;
        boolean sawTwo = false;
        for (Sample<Item> sample : reader.read(new java.util.ArrayList<>())) {
            assertEquals(InstanceState.NOT_ALIVE_DISPOSED, sample.getInstanceState());
            sawOne |= sample.getInstanceHandle().equals(one);
            sawTwo |= sample.getInstanceHandle().equals(two);
        }
        assertEquals(true, sawOne);
        assertEquals(true, sawTwo);
        // a handle of no instance of the writer is BAD_PARAMETER, the handle of another
        // instance than the sample's PRECONDITION_NOT_MET (DDS 1.4 2.2.2.4.2.13)
        InstanceHandle foreign = foreignHandle();
        assertThrows(IllegalArgumentException.class, () -> writer.dispose(foreign, new Item(1, 0)));
        assertThrows(PreconditionNotMetException.class, () -> writer.dispose(two, new Item(1, 0)));
        assertThrows(IllegalArgumentException.class, () -> writer.write(new Item(1, 1), foreign));
        assertThrows(PreconditionNotMetException.class, () -> writer.write(new Item(1, 1), two));
    }

    @Test
    void getKeyValueRefusesAnUnkeyedTypeAndAnUnknownHandle() throws Exception {
        DomainParticipant participant = Qos.factory().createParticipant(1385);
        DataWriter<byte[]> unkeyed = participant.createPublisher().createDataWriter(
                participant.createTopic("KeyUnkeyed", byte[].class));
        assertThrows(PreconditionNotMetException.class,
                () -> unkeyed.getKeyValue(new byte[0], Qos.env().getSPI().nilHandle()));

        DataWriter<Item> keyed = participant.createPublisher().createDataWriter(
                participant.createTopic("KeyKeyed", Item.class));
        InstanceHandle known = keyed.registerInstance(new Item(4, 0));
        keyed.write(new Item(4, 1));
        // the key is returned in the sample given
        Item holder = keyed.getKeyValue(new Item(), known);
        assertEquals(4, holder.id);
        // BAD_PARAMETER for a handle this writer did not register
        assertThrows(IllegalArgumentException.class,
                () -> keyed.getKeyValue(new Item(), foreignHandle()));
    }
}
