// SPDX-License-Identifier: Apache-2.0
package org.omg.dds;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.omg.dds.core.InstanceHandle;
import org.omg.dds.core.ModifiableInstanceHandle;
import org.omg.dds.core.PreconditionNotMetException;
import org.omg.dds.domain.DomainParticipant;
import org.omg.dds.pub.DataWriter;
import org.omg.dds.sub.DataReader;
import org.omg.dds.topic.Topic;
import org.omg.dds.type.Key;
import org.omg.dds.type.builtin.KeyedString;
import org.zerodds.internal.InProcessBus;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotSame;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * DDS-Java-PSM 1.0 §7.2.5 and DDS 1.4 §2.2.2.5.3.29: getKeyValue(keyHolder, handle)
 * writes the key fields of the instance into the key holder, leaves its other
 * fields alone and returns the holder; lookupInstance(handle, keyHolder) fills
 * the handle it is given.
 */
class CoreKeyValueTest {

    private static final int DOMAIN = 1506;

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

    private DomainParticipant participant;

    @BeforeEach
    void setUp() {
        InProcessBus.instance().reset();
        participant = Qos.factory().createParticipant(DOMAIN);
    }

    @AfterEach
    void tearDown() {
        participant.close();
        InProcessBus.instance().reset();
    }

    @Test
    void aWriterFillsTheKeyFieldsOfTheHolderOnly() throws Exception {
        Topic<Item> topic = participant.createTopic("CoreKeyValueWriter", Item.class);
        DataWriter<Item> writer = participant.createPublisher().createDataWriter(topic);
        InstanceHandle two = writer.registerInstance(new Item(2, 0));
        writer.write(new Item(2, 7));
        Item holder = new Item(0, 99);
        Item result = writer.getKeyValue(holder, two);
        assertSame(holder, result);
        assertEquals(2, holder.id);
        // the field that is not part of the key keeps what the holder had
        assertEquals(99, holder.value);
        // without a holder the writer makes a new object
        Item fresh = writer.getKeyValue(two);
        assertEquals(2, fresh.id);
        assertNotSame(holder, fresh);
        // a null holder gives a new object, too
        assertEquals(2, writer.getKeyValue(null, two).id);
    }

    @Test
    void aReaderFillsTheKeyFieldsOfTheHolderOnly() throws Exception {
        Topic<Item> topic = participant.createTopic("CoreKeyValueReader", Item.class);
        DataReader<Item> reader = participant.createSubscriber().createDataReader(topic, Qos.readerKeepAll());
        DataWriter<Item> writer = participant.createPublisher().createDataWriter(topic);
        writer.write(new Item(5, 50));
        InstanceHandle five = reader.lookupInstance(new Item(5, 0));
        assertFalse(five.isNil());
        Item holder = new Item(0, 99);
        assertSame(holder, reader.getKeyValue(holder, five));
        assertEquals(5, holder.id);
        assertEquals(99, holder.value);
        // the reader keeps the instance after its samples were taken
        reader.take(new java.util.ArrayList<org.omg.dds.sub.Sample<Item>>());
        Item again = new Item(0, 1);
        reader.getKeyValue(again, five);
        assertEquals(5, again.id);
    }

    @Test
    void aHandleOfNoInstanceIsABadParameter() throws Exception {
        Topic<Item> topic = participant.createTopic("CoreKeyValueUnknown", Item.class);
        DataReader<Item> reader = participant.createSubscriber().createDataReader(topic);
        DataWriter<Item> writer = participant.createPublisher().createDataWriter(topic);
        InstanceHandle registered = writer.registerInstance(new Item(8, 0));
        // the reader has not seen the instance: no sample arrived
        assertThrows(IllegalArgumentException.class, () -> reader.getKeyValue(new Item(), registered));
        assertThrows(IllegalArgumentException.class,
                () -> writer.getKeyValue(new Item(), reader.lookupInstance(new Item(8, 0))));
    }

    @Test
    void lookupInstanceFillsTheHandleItIsGiven() throws Exception {
        Topic<Item> topic = participant.createTopic("CoreKeyValueLookup", Item.class);
        DataReader<Item> reader = participant.createSubscriber().createDataReader(topic);
        DataWriter<Item> writer = participant.createPublisher().createDataWriter(topic);
        InstanceHandle registered = writer.registerInstance(new Item(3, 0));
        writer.write(new Item(3, 30));

        ModifiableInstanceHandle given = Qos.env().getSPI().newInstanceHandle();
        assertTrue(given.isNil());
        assertSame(given, writer.lookupInstance(given, new Item(3, 0)));
        assertEquals(registered, given);

        ModifiableInstanceHandle readerHandle = Qos.env().getSPI().newInstanceHandle();
        assertSame(readerHandle, reader.lookupInstance(readerHandle, new Item(3, 0)));
        assertFalse(readerHandle.isNil());
        assertEquals(registered, readerHandle);

        // an instance nobody registered gives the nil handle in the given object
        ModifiableInstanceHandle none = Qos.env().getSPI().newInstanceHandle();
        none.copyFrom(registered);
        writer.lookupInstance(none, new Item(77, 0));
        assertTrue(none.isNil());
    }

    @Test
    void theKeyHolderOfABuiltInKeyedTypeKeepsItsValue() throws Exception {
        Topic<KeyedString> topic = participant.createTopic("CoreKeyValueKeyedString", KeyedString.class);
        DataWriter<KeyedString> writer = participant.createPublisher().createDataWriter(topic);
        InstanceHandle handle = writer.registerInstance(KeyedString.newKeyedString(Qos.env()).setKey("k1"));
        writer.write(KeyedString.newKeyedString(Qos.env()).setKey("k1").setValue("sent"));
        KeyedString holder = KeyedString.newKeyedString(Qos.env()).setKey("old").setValue("keep");
        assertSame(holder, writer.getKeyValue(holder, handle));
        assertEquals("k1", holder.getKey());
        assertEquals("keep", holder.getValue());
    }

    @Test
    void aTypeWithoutAKeyHasNoKeyValue() throws Exception {
        Topic<byte[]> topic = participant.createTopic("CoreKeyValueUnkeyed", byte[].class);
        DataWriter<byte[]> writer = participant.createPublisher().createDataWriter(topic);
        DataReader<byte[]> reader = participant.createSubscriber().createDataReader(topic);
        writer.write(new byte[] {1});
        InstanceHandle handle = reader.lookupInstance(new byte[] {1});
        assertThrows(PreconditionNotMetException.class, () -> reader.getKeyValue(new byte[0], handle));
        assertThrows(PreconditionNotMetException.class, () -> writer.getKeyValue(new byte[0], handle));
    }
}
