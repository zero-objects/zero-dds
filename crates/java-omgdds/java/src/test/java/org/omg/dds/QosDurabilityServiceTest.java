// SPDX-License-Identifier: Apache-2.0
package org.omg.dds;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.omg.dds.core.OutOfResourcesException;
import org.omg.dds.core.policy.Durability;
import org.omg.dds.core.policy.DurabilityService;
import org.omg.dds.pub.DataWriter;
import org.omg.dds.pub.DataWriterQos;
import org.omg.dds.sub.DataReader;
import org.omg.dds.sub.DataReaderQos;
import org.omg.dds.sub.InstanceState;
import org.omg.dds.sub.Sample;
import org.zerodds.internal.InProcessBus;
import org.zerodds.internal.InstanceKey;

import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicLong;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * DURABILITY_SERVICE (DDS 1.4 §2.2.3.5) and {@code service_cleanup_delay} (§2.2.3.4): the
 * history and the limits of the store behind TRANSIENT and PERSISTENT, and the time the store
 * keeps an instance that was disposed. Domain ids 1400 to 1499.
 */
class QosDurabilityServiceTest {

    private static final org.zerodds.internal.DurabilityService STORE =
            org.zerodds.internal.DurabilityService.instance();

    @BeforeEach
    void setUp() {
        InProcessBus.instance().reset();
        STORE.reset();
    }

    @AfterEach
    void tearDown() {
        InProcessBus.instance().reset();
        STORE.reset();
    }

    /** A TRANSIENT writer that does not dispose what it unregisters, with the given store policy. */
    private static DataWriterQos transientWriter(DurabilityService service) {
        return Qos.writer()
                .withPolicy(Qos.pf().Durability().withTransient())
                .withPolicy(Qos.pf().WriterDataLifecycle().withAutDisposeUnregisteredInstances(false))
                .withPolicy(service);
    }

    private static DataReaderQos transientReader() {
        return Qos.readerKeepAll().withPolicy(Qos.pf().Durability().withTransient());
    }

    private static List<Integer> seqs(List<Sample<QosFixtures.Item>> samples) {
        List<Integer> seqs = new ArrayList<>();
        for (Sample<QosFixtures.Item> sample : samples) {
            if (sample.getData() != null) {
                seqs.add(sample.getData().seq());
            }
        }
        return seqs;
    }

    // --- the history of the store -------------------------------------------------------

    @Test
    void theHistoryDepthOfThePolicyDecidesWhatALateJoinerGets() throws Exception {
        try (QosFixtures.Rig rig = new QosFixtures.Rig()) {
            // the HISTORY of the writer is 1; the policy of the store says 3
            DataWriter<QosFixtures.Item> writer = rig.writer(transientWriter(
                    Qos.pf().DurabilityService().withHistoryDepth(3)));
            for (int seq = 0; seq < 5; seq++) {
                writer.write(new QosFixtures.Item(1, seq));
            }
            writer.close();

            DataReader<QosFixtures.Item> late = rig.reader(transientReader());
            assertEquals(java.util.Arrays.asList(2, 3, 4), seqs(Qos.take(late)));
        }
    }

    @Test
    void keepAllInTheStoreKeepsEverySampleUpToTheLimit() throws Exception {
        try (QosFixtures.Rig rig = new QosFixtures.Rig()) {
            DataWriter<QosFixtures.Item> writer = rig.writer(transientWriter(
                    Qos.pf().DurabilityService().withHistoryKind(
                            org.omg.dds.core.policy.History.Kind.KEEP_ALL)));
            for (int seq = 0; seq < 6; seq++) {
                writer.write(new QosFixtures.Item(1, seq));
            }
            writer.close();
            DataReader<QosFixtures.Item> late = rig.reader(transientReader());
            assertEquals(java.util.Arrays.asList(0, 1, 2, 3, 4, 5), seqs(Qos.take(late)));
        }
    }

    // --- the limits of the store --------------------------------------------------------

    @Test
    void aSampleBeyondMaxSamplesPerInstanceIsRefusedAndNotDelivered() throws Exception {
        try (QosFixtures.Rig rig = new QosFixtures.Rig()) {
            DataReader<QosFixtures.Item> live = rig.reader(Qos.readerKeepAll());
            DataWriter<QosFixtures.Item> writer = rig.writer(transientWriter(
                    Qos.pf().DurabilityService()
                            .withHistoryKind(org.omg.dds.core.policy.History.Kind.KEEP_ALL)
                            .withMaxSamplesPerInstance(2)));
            writer.write(new QosFixtures.Item(1, 0));
            writer.write(new QosFixtures.Item(1, 1));
            assertThrows(OutOfResourcesException.class, () -> writer.write(new QosFixtures.Item(1, 2)));
            // another instance has its own two
            writer.write(new QosFixtures.Item(2, 0));

            // the refused sample reached nobody; the instances reach a reader in no set order
            assertEquals(java.util.Arrays.asList(0, 0, 1), sorted(seqs(Qos.take(live))));
            writer.close();
            DataReader<QosFixtures.Item> late = rig.reader(transientReader());
            assertEquals(java.util.Arrays.asList(0, 0, 1), sorted(seqs(Qos.take(late))));
        }
    }

    @Test
    void aNewInstanceBeyondMaxInstancesIsRefused() throws Exception {
        try (QosFixtures.Rig rig = new QosFixtures.Rig()) {
            DataWriter<QosFixtures.Item> writer = rig.writer(transientWriter(
                    Qos.pf().DurabilityService().withMaxInstances(1)));
            writer.write(new QosFixtures.Item(1, 0));
            writer.write(new QosFixtures.Item(1, 1)); // the same instance
            assertThrows(OutOfResourcesException.class, () -> writer.write(new QosFixtures.Item(2, 0)));
        }
    }

    @Test
    void maxSamplesBoundsTheSamplesOfAllInstancesTogether() throws Exception {
        try (QosFixtures.Rig rig = new QosFixtures.Rig()) {
            DataWriter<QosFixtures.Item> writer = rig.writer(transientWriter(
                    Qos.pf().DurabilityService().withMaxSamples(2)));
            writer.write(new QosFixtures.Item(1, 0));
            writer.write(new QosFixtures.Item(2, 0));
            // history depth 1: the new sample of instance 1 replaces the old one, still 2 samples
            writer.write(new QosFixtures.Item(1, 1));
            // a third instance would be a third sample
            assertThrows(OutOfResourcesException.class, () -> writer.write(new QosFixtures.Item(3, 0)));
        }
    }

    // --- service_cleanup_delay -----------------------------------------------------------

    private static DataWriterQos disposingTransientWriter(long cleanupDelaySeconds) {
        return Qos.writer()
                .withPolicy(Qos.pf().Durability().withTransient())
                .withPolicy(Qos.pf().DurabilityService()
                        .withServiceCleanupDelay(cleanupDelaySeconds, TimeUnit.SECONDS));
    }

    @Test
    void aDisposedInstanceStaysInTheStoreForTheCleanupDelayAndIsToldDisposed() throws Exception {
        AtomicLong now = new AtomicLong(1_000_000L);
        STORE.useClock(now::get);
        try (QosFixtures.Rig rig = new QosFixtures.Rig()) {
            DataWriter<QosFixtures.Item> writer = rig.writer(disposingTransientWriter(10));
            writer.write(new QosFixtures.Item(1, 7));
            writer.close(); // autodispose: disposed, and no writer is left
            InstanceKey key = new InstanceKey(java.nio.ByteBuffer.allocate(4).putInt(1).array());
            assertTrue(STORE.holds(domainOf(rig), "Items", "QosItem", key));

            now.set(1_000_000L + 5_000L); // 5 s: inside the 10 s delay
            DataReader<QosFixtures.Item> within = rig.reader(transientReader());
            List<Sample<QosFixtures.Item>> got = Qos.take(within);
            assertEquals(2, got.size(), "the sample and the dispose");
            assertEquals(7, got.get(0).getData().seq());
            for (Sample<QosFixtures.Item> sample : got) {
                assertEquals(InstanceState.NOT_ALIVE_DISPOSED, sample.getInstanceState());
            }

            now.set(1_000_000L + 11_000L); // 11 s: the delay has passed
            assertFalse(STORE.holds(domainOf(rig), "Items", "QosItem", key));
            DataReader<QosFixtures.Item> after = rig.reader(transientReader());
            assertEquals(0, Qos.take(after).size());
        }
    }

    @Test
    void withNoCleanupDelayADisposedInstanceWithoutWritersIsForgottenAtOnce() throws Exception {
        try (QosFixtures.Rig rig = new QosFixtures.Rig()) {
            DataWriter<QosFixtures.Item> writer = rig.writer(disposingTransientWriter(0));
            writer.write(new QosFixtures.Item(1, 7));
            writer.close();
            DataReader<QosFixtures.Item> late = rig.reader(transientReader());
            assertEquals(0, Qos.take(late).size());
        }
    }

    @Test
    void anInstanceDisposedWhileAWriterStillWritesItIsKeptDisposed() throws Exception {
        try (QosFixtures.Rig rig = new QosFixtures.Rig()) {
            DataWriter<QosFixtures.Item> writer = rig.writer(disposingTransientWriter(0));
            writer.write(new QosFixtures.Item(1, 7));
            Qos.dispose(writer, new QosFixtures.Item(1, 0));
            // the writer has not unregistered: the instance is disposed, not forgotten
            DataReader<QosFixtures.Item> late = rig.reader(transientReader());
            List<Sample<QosFixtures.Item>> got = Qos.take(late);
            assertEquals(2, got.size());
            for (Sample<QosFixtures.Item> sample : got) {
                assertEquals(InstanceState.NOT_ALIVE_DISPOSED, sample.getInstanceState());
            }
        }
    }

    @Test
    void aWriteAfterTheDisposeMakesTheInstanceAliveInTheStore() throws Exception {
        try (QosFixtures.Rig rig = new QosFixtures.Rig()) {
            DataWriter<QosFixtures.Item> writer = rig.writer(disposingTransientWriter(100));
            writer.write(new QosFixtures.Item(1, 7));
            Qos.dispose(writer, new QosFixtures.Item(1, 0));
            writer.write(new QosFixtures.Item(1, 8));
            DataReader<QosFixtures.Item> late = rig.reader(transientReader());
            List<Sample<QosFixtures.Item>> got = Qos.take(late);
            assertEquals(java.util.Arrays.asList(8), seqs(got));
            assertEquals(InstanceState.ALIVE, got.get(got.size() - 1).getInstanceState());
        }
    }

    @Test
    void durableSamplesKeepTheirSourceTimestampAndReachTheReaderWithItsLifespan() throws Exception {
        try (QosFixtures.Rig rig = new QosFixtures.Rig()) {
            DataWriter<QosFixtures.Item> writer = rig.writer(transientWriter(Qos.pf().DurabilityService())
                    .withPolicy(Qos.pf().Lifespan().withDuration(60, TimeUnit.SECONDS)));
            long now = System.currentTimeMillis();
            writer.write(new QosFixtures.Item(1, 0), now - 120_000L, TimeUnit.MILLISECONDS); // expired
            writer.write(new QosFixtures.Item(2, 1), now, TimeUnit.MILLISECONDS);            // valid
            writer.close();
            DataReader<QosFixtures.Item> late = rig.reader(transientReader());
            assertEquals(java.util.Arrays.asList(1), seqs(Qos.take(late)));
        }
    }

    private static List<Integer> sorted(List<Integer> values) {
        List<Integer> copy = new ArrayList<>(values);
        java.util.Collections.sort(copy);
        return copy;
    }

    private static int domainOf(QosFixtures.Rig rig) {
        return rig.participant.getDomainId();
    }
}
