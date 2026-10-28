// SPDX-License-Identifier: Apache-2.0
package org.omg.dds;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.omg.dds.core.InstanceHandle;
import org.omg.dds.core.PreconditionNotMetException;
import org.omg.dds.domain.DomainParticipant;
import org.omg.dds.pub.DataWriter;
import org.omg.dds.sub.DataReader;
import org.omg.dds.sub.InstanceState;
import org.omg.dds.sub.Sample;
import org.omg.dds.topic.Topic;
import org.omg.dds.type.Key;
import org.zerodds.internal.InProcessBus;

import java.util.ArrayList;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotEquals;
import static org.junit.jupiter.api.Assertions.assertThrows;

/**
 * DDS 1.4 §2.2.2.4.2 and §2.2.2.5.3: the operations that take an instance handle.
 */
class InstanceHandleTest {

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

    @BeforeEach
    void setUp() {
        InProcessBus.instance().reset();
    }

    @AfterEach
    void tearDown() {
        InProcessBus.instance().reset();
    }

    @Test
    void aWriterDisposesAndUnregistersTheInstancesItRegistered() throws Exception {
        DomainParticipant participant = Qos.factory().createParticipant(980);
        Topic<Item> topic = participant.createTopic("Items", Item.class);
        DataReader<Item> reader = participant.createSubscriber().createDataReader(topic,
                Qos.readerKeepAll());
        DataWriter<Item> writer = participant.createPublisher().createDataWriter(topic);
        InstanceHandle one = writer.registerInstance(new Item(1, 0));
        InstanceHandle two = writer.registerInstance(new Item(2, 0));
        assertNotEquals(one, two);
        writer.write(new Item(1, 10));
        writer.write(new Item(2, 20));

        writer.dispose(one);
        writer.dispose(two, new Item(2, 0));
        // the instance state belongs to the instance (DDS 1.4 §2.2.2.5.1.8): every sample of the
        // two instances reports it, the written one and the one that tells of the dispose
        List<Sample<Item>> got = reader.read(new ArrayList<>());
        boolean sawOne = false;
        boolean sawTwo = false;
        for (Sample<Item> sample : got) {
            assertEquals(InstanceState.NOT_ALIVE_DISPOSED, sample.getInstanceState());
            sawOne |= sample.getInstanceHandle().equals(one);
            sawTwo |= sample.getInstanceHandle().equals(two);
        }
        assertEquals(true, sawOne);
        assertEquals(true, sawTwo);

        writer.unregisterInstance(one);
        // the handle is not an instance of the writer any more: BAD_PARAMETER
        assertThrows(IllegalArgumentException.class, () -> writer.unregisterInstance(one));
        // a sample of another instance than the handle names
        assertThrows(PreconditionNotMetException.class,
                () -> writer.dispose(two, new Item(1, 0)));
        // neither handle nor sample
        assertThrows(IllegalArgumentException.class,
                () -> writer.dispose(org.zerodds.core.Values.nil(), null));
    }

    @Test
    void aHandleGivenWithAWriteIsTheHandleOfTheSamplesInstance() throws Exception {
        DomainParticipant participant = Qos.factory().createParticipant(981);
        Topic<Item> topic = participant.createTopic("ItemsWrite", Item.class);
        DataWriter<Item> writer = participant.createPublisher().createDataWriter(topic);
        InstanceHandle one = writer.registerInstance(new Item(1, 0));
        InstanceHandle two = writer.registerInstance(new Item(2, 0));
        writer.write(new Item(1, 5), one);
        assertThrows(PreconditionNotMetException.class, () -> writer.write(new Item(1, 5), two));
    }

    @Test
    void getKeyValueReturnsTheInstanceOfTheHandle() throws Exception {
        DomainParticipant participant = Qos.factory().createParticipant(982);
        Topic<Item> topic = participant.createTopic("ItemsKey", Item.class);
        DataWriter<Item> writer = participant.createPublisher().createDataWriter(topic);
        InstanceHandle two = writer.registerInstance(new Item(2, 0));
        writer.write(new Item(2, 7));
        Item key = writer.getKeyValue(new Item(), two);
        assertEquals(2, key.id);
        assertEquals(two, writer.lookupInstance(key));
    }
}
