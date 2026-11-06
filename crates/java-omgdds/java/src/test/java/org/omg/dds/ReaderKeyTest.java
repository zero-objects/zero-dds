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
    void javapsm_2_get_key_value_returns_a_sample_of_the_instance_that_has_the_key() throws Exception {
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
    void javapsm_2_lookup_instance_is_nil_before_the_instance_is_known_and_the_handle_of_the_sample_afterwards()
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
    void javapsm_2_lookup_instance_with_a_handle_fills_it_and_without_one_returns_a_new_object() throws Exception {
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
