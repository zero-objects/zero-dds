// SPDX-License-Identifier: Apache-2.0
package org.omg.dds;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.omg.dds.pub.DataWriter;
import org.omg.dds.pub.DataWriterQos;
import org.omg.dds.sub.DataReader;
import org.omg.dds.sub.InstanceState;
import org.omg.dds.sub.Sample;
import org.zerodds.internal.InProcessBus;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * WRITER_DATA_LIFECYCLE (DDS 1.4 §2.2.3.21): with {@code autodispose_unregistered_instances}
 * true the writer disposes an instance each time it unregisters it, also when it unregisters
 * the instances it manages because it is deleted. Domain ids 1400 to 1499.
 */
class QosWriterDataLifecycleTest {

    @BeforeEach
    void setUp() {
        InProcessBus.instance().reset();
    }

    @AfterEach
    void tearDown() {
        InProcessBus.instance().reset();
    }

    private static DataWriterQos autodispose(boolean on) {
        return Qos.writer().withPolicy(
                Qos.pf().WriterDataLifecycle().withAutDisposeUnregisteredInstances(on));
    }

    private static void assertAllIn(InstanceState expected, List<Sample<QosFixtures.Item>> samples) {
        assertTrue(!samples.isEmpty(), "samples expected");
        for (Sample<QosFixtures.Item> sample : samples) {
            assertEquals(expected, sample.getInstanceState());
        }
    }

    @Test
    void theDefaultIsToDisposeWhatIsUnregistered() {
        assertTrue(Qos.writer().getWriterDataLifecycle().isAutDisposeUnregisteredInstances());
        assertTrue(Qos.pf().WriterDataLifecycle().isAutDisposeUnregisteredInstances());
    }

    @Test
    void unregisterDisposesTheInstanceWhenTheFlagIsTrue() throws Exception {
        try (QosFixtures.Rig rig = new QosFixtures.Rig()) {
            DataReader<QosFixtures.Item> reader = rig.reader(Qos.readerKeepAll());
            DataWriter<QosFixtures.Item> writer = rig.writer(Qos.writer());
            writer.write(new QosFixtures.Item(1, 10));
            Qos.unregister(writer, new QosFixtures.Item(1, 0));

            // The data sample, then the notification of the state change; both are seen
            // in the instance_state of the moment of the take: NOT_ALIVE_DISPOSED, and the
            // unregister that follows the dispose does not move it to NO_WRITERS.
            List<Sample<QosFixtures.Item>> got = Qos.take(reader);
            assertEquals(2, got.size());
            assertNotNull(got.get(0).getData());
            assertEquals(10, got.get(0).getData().seq());
            assertNull(got.get(1).getData());
            assertAllIn(InstanceState.NOT_ALIVE_DISPOSED, got);
        }
    }

    @Test
    void unregisterOnlyLeavesNoWritersWhenTheFlagIsFalse() throws Exception {
        try (QosFixtures.Rig rig = new QosFixtures.Rig()) {
            DataReader<QosFixtures.Item> reader = rig.reader(Qos.readerKeepAll());
            DataWriter<QosFixtures.Item> writer = rig.writer(autodispose(false));
            writer.write(new QosFixtures.Item(1, 10));
            Qos.unregister(writer, new QosFixtures.Item(1, 0));

            List<Sample<QosFixtures.Item>> got = Qos.take(reader);
            assertEquals(2, got.size());
            assertAllIn(InstanceState.NOT_ALIVE_NO_WRITERS, got);
        }
    }

    @Test
    void anInstanceDisposedByTheApplicationStaysDisposedWhenItIsUnregistered() throws Exception {
        try (QosFixtures.Rig rig = new QosFixtures.Rig()) {
            DataReader<QosFixtures.Item> reader = rig.reader(Qos.readerKeepAll());
            DataWriter<QosFixtures.Item> writer = rig.writer(autodispose(false));
            writer.write(new QosFixtures.Item(1, 10));
            Qos.dispose(writer, new QosFixtures.Item(1, 0));
            Qos.unregister(writer, new QosFixtures.Item(1, 0));

            List<Sample<QosFixtures.Item>> got = Qos.take(reader);
            assertEquals(2, got.size());
            assertAllIn(InstanceState.NOT_ALIVE_DISPOSED, got);
        }
    }

    @Test
    void deletingTheWriterDisposesItsInstancesWhenTheFlagIsTrue() throws Exception {
        try (QosFixtures.Rig rig = new QosFixtures.Rig()) {
            DataReader<QosFixtures.Item> reader = rig.reader(Qos.readerKeepAll());
            DataWriter<QosFixtures.Item> writer = rig.writer(Qos.writer());
            writer.write(new QosFixtures.Item(1, 10));
            writer.write(new QosFixtures.Item(2, 20));
            writer.close();

            List<Sample<QosFixtures.Item>> got = Qos.take(reader);
            // two data samples and two notifications
            assertEquals(4, got.size());
            assertAllIn(InstanceState.NOT_ALIVE_DISPOSED, got);
        }
    }

    @Test
    void deletingTheWriterOnlyLeavesNoWritersWhenTheFlagIsFalse() throws Exception {
        try (QosFixtures.Rig rig = new QosFixtures.Rig()) {
            DataReader<QosFixtures.Item> reader = rig.reader(Qos.readerKeepAll());
            DataWriter<QosFixtures.Item> writer = rig.writer(autodispose(false));
            writer.write(new QosFixtures.Item(1, 10));
            writer.write(new QosFixtures.Item(2, 20));
            writer.close();

            List<Sample<QosFixtures.Item>> got = Qos.take(reader);
            assertEquals(4, got.size());
            assertAllIn(InstanceState.NOT_ALIVE_NO_WRITERS, got);
        }
    }

    @Test
    void aWriteAfterTheDisposeBringsTheInstanceBackToAlive() throws Exception {
        try (QosFixtures.Rig rig = new QosFixtures.Rig()) {
            DataReader<QosFixtures.Item> reader = rig.reader(Qos.readerKeepAll());
            DataWriter<QosFixtures.Item> writer = rig.writer(Qos.writer());
            writer.write(new QosFixtures.Item(1, 10));
            Qos.unregister(writer, new QosFixtures.Item(1, 0));
            assertEquals(2, Qos.take(reader).size());

            writer.write(new QosFixtures.Item(1, 11));
            List<Sample<QosFixtures.Item>> got = Qos.take(reader);
            assertEquals(1, got.size());
            assertEquals(11, got.get(0).getData().seq());
            assertEquals(InstanceState.ALIVE, got.get(0).getInstanceState());
        }
    }
}
