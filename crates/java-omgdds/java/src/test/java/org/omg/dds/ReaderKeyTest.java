// SPDX-License-Identifier: Apache-2.0
package org.omg.dds;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.omg.dds.core.InstanceHandle;
import org.omg.dds.core.ModifiableInstanceHandle;
import org.omg.dds.domain.DomainParticipant;
import org.omg.dds.pub.DataWriter;
import org.omg.dds.sub.DataReader;
import org.omg.dds.sub.Sample;
import org.omg.dds.topic.Topic;
import org.omg.dds.type.Key;
import org.zerodds.core.PreconditionNotMetException;
import org.zerodds.core.Values;
import org.zerodds.internal.InProcessBus;

import java.util.ArrayList;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertNotSame;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;
import static org.omg.dds.ReaderRig.Keyed;
import static org.omg.dds.ReaderRig.sample;

/**
 * DDS 1.4 §2.2.2.5.3.29 get_key_value and §2.2.2.5.3.30 lookup_instance on a reader, as the PSM
 * maps them (DDS-Java-PSM 1.0 §7.7.3).
 */
class ReaderKeyTest {

    private static int dom = 1150;

    private static int nextDom() {
        return dom++;
    }

    /** A bean the reflective type support (DDS-Java-PSM 1.0 §8) handles: {@code id} is the key. */
    public static final class Track {
        @Key
        public int id;
        public int speed;

        public Track() {
        }

        public Track(int id, int speed) {
            this.id = id;
            this.speed = speed;
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
    void getKeyValueReturnsASampleOfTheInstanceThatHasTheKey() throws Exception {
        ReaderRig rig = new ReaderRig(nextDom());
        DataReader<Keyed> reader = rig.keepAllReader();
        DataWriter<Keyed> writer = rig.writer();
        writer.write(new Keyed(7, 3, 9.5));
        InstanceHandle handle = reader.lookupInstance(sample(7, 0));
        // a record has no setters: the result is a copy of the last sample, key set
        Keyed key = reader.getKeyValue(new Keyed(0, 0, 0.0), handle);
        assertEquals(7, key.id());
        // the reader keeps the sample of the instance for this after the samples are taken
        Qos.take(reader);
        assertEquals(7, reader.getKeyValue(new Keyed(0, 0, 0.0), handle).id());
    }

    @Test
    void getKeyValueSetsTheKeyFieldsOfTheHolderAndLeavesTheOthers() throws Exception {
        DomainParticipant participant = Qos.factory().createParticipant(nextDom());
        Topic<Track> topic = participant.createTopic("ReaderTrack", Track.class);
        DataReader<Track> reader = participant.createSubscriber().createDataReader(topic);
        DataWriter<Track> writer = participant.createPublisher().createDataWriter(topic);
        writer.write(new Track(5, 100));
        InstanceHandle handle = reader.lookupInstance(new Track(5, 0));
        assertFalse(handle.isNil());
        Track holder = new Track(0, 55);
        Track result = reader.getKeyValue(holder, handle);
        assertSame(holder, result);
        assertEquals(5, holder.id);
        assertEquals(55, holder.speed, "only the key fields are set");
    }

    @Test
    void getKeyValueOfAHandleTheReaderDoesNotKnowIsAnIllegalArgument() throws Exception {
        ReaderRig rig = new ReaderRig(nextDom());
        DataReader<Keyed> reader = rig.keepAllReader();
        rig.writer().write(sample(1, 0));
        byte[] other = new byte[16];
        other[15] = 99;
        InstanceHandle unknown = Values.of(other);
        assertThrows(IllegalArgumentException.class,
                () -> reader.getKeyValue(new Keyed(0, 0, 0.0), unknown));
        assertThrows(IllegalArgumentException.class,
                () -> reader.getKeyValue(new Keyed(0, 0, 0.0), null));
    }

    @Test
    void getKeyValueOnATopicWithoutAKeyIsPreconditionNotMet() throws Exception {
        DomainParticipant participant = Qos.factory().createParticipant(nextDom());
        Topic<byte[]> topic = participant.createTopic("ReaderUnkeyed", byte[].class);
        DataReader<byte[]> reader = participant.createSubscriber().createDataReader(topic);
        participant.createPublisher().createDataWriter(topic).write(new byte[] {1});
        InstanceHandle handle = reader.read(new ArrayList<>()).get(0).getInstanceHandle();
        assertThrows(PreconditionNotMetException.class,
                () -> reader.getKeyValue(new byte[0], handle));
    }

    @Test
    void lookupInstanceIsNilBeforeTheInstanceIsKnownAndTheHandleOfTheSampleAfterwards()
            throws Exception {
        ReaderRig rig = new ReaderRig(nextDom());
        DataReader<Keyed> reader = rig.keepAllReader();
        DataWriter<Keyed> writer = rig.writer();
        assertTrue(reader.lookupInstance(sample(4, 0)).isNil());
        writer.write(sample(4, 1));
        List<Sample<Keyed>> got = Qos.read(reader);
        InstanceHandle handle = reader.lookupInstance(sample(4, 99));
        assertFalse(handle.isNil());
        assertEquals(got.get(0).getInstanceHandle(), handle);
        // another key is another instance, which the reader does not know yet
        assertTrue(reader.lookupInstance(sample(5, 0)).isNil());
    }

    @Test
    void lookupInstanceWithAHandleFillsItAndWithoutOneReturnsANewObject() throws Exception {
        ReaderRig rig = new ReaderRig(nextDom());
        DataReader<Keyed> reader = rig.keepAllReader();
        rig.writer().write(sample(4, 1));
        InstanceHandle expected = reader.lookupInstance(sample(4, 0));

        ModifiableInstanceHandle mine = InstanceHandle.newInstanceHandle(Qos.env());
        assertSame(mine, reader.lookupInstance(mine, sample(4, 0)));
        assertEquals(expected, mine);

        ModifiableInstanceHandle created = reader.lookupInstance(null, sample(4, 0));
        assertNotNull(created);
        assertNotSame(mine, created);
        assertEquals(expected, created);
        // an instance the reader does not know: nil, also in a new object
        assertTrue(reader.lookupInstance(null, sample(8, 0)).isNil());
        // a sample is needed
        assertThrows(IllegalArgumentException.class, () -> reader.lookupInstance((Keyed) null));
    }
}
