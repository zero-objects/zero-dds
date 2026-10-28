// SPDX-License-Identifier: Apache-2.0
package org.omg.dds;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.omg.dds.pub.DataWriter;
import org.omg.dds.pub.DataWriterQos;
import org.omg.dds.sub.DataReader;
import org.omg.dds.sub.DataReaderQos;
import org.omg.dds.sub.Sample;
import org.zerodds.internal.InProcessBus;

import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import java.util.concurrent.TimeUnit;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.fail;

/**
 * The effect of LIFESPAN (§2.2.3.16), TIME_BASED_FILTER (§2.2.3.12), DESTINATION_ORDER
 * (§2.2.3.17) and READER_DATA_LIFECYCLE (§2.2.3.22) of DDS 1.4 on what a reader holds.
 * Domain ids 1400 to 1499.
 */
class QosReaderEffectsTest {

    @BeforeEach
    void setUp() {
        InProcessBus.instance().reset();
    }

    @AfterEach
    void tearDown() {
        InProcessBus.instance().reset();
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

    private static DataWriterQos autodispose(boolean on) {
        return Qos.writer().withPolicy(
                Qos.pf().WriterDataLifecycle().withAutDisposeUnregisteredInstances(on));
    }

    /** Calls {@code check} every 20 ms until it holds; fails after 20 s. */
    private static void awaitTrue(String what, java.util.function.BooleanSupplier check) throws Exception {
        long limit = System.nanoTime() + TimeUnit.SECONDS.toNanos(20);
        while (!check.getAsBoolean()) {
            if (System.nanoTime() > limit) {
                fail("not reached within 20 s: " + what);
            }
            Thread.sleep(20);
        }
    }

    // --- LIFESPAN ----------------------------------------------------------------------

    @Test
    void aSampleWhoseLifespanHasPassedWhenItArrivesIsNotDelivered() throws Exception {
        try (QosFixtures.Rig rig = new QosFixtures.Rig()) {
            DataReader<QosFixtures.Item> reader = rig.reader(Qos.readerKeepAll());
            DataWriter<QosFixtures.Item> writer = rig.writer(Qos.writer()
                    .withPolicy(Qos.pf().Lifespan().withDuration(60, TimeUnit.SECONDS)));
            long now = System.currentTimeMillis();
            // source timestamp plus 60 s is two minutes ago for the first, a minute ahead for the second
            writer.write(new QosFixtures.Item(1, 1), now - 120_000L, TimeUnit.MILLISECONDS);
            writer.write(new QosFixtures.Item(2, 2), now, TimeUnit.MILLISECONDS);
            assertEquals(Arrays.asList(2), seqs(Qos.take(reader)));
        }
    }

    @Test
    void aStoredSampleDisappearsWhenItsLifespanPasses() throws Exception {
        try (QosFixtures.Rig rig = new QosFixtures.Rig()) {
            DataReader<QosFixtures.Item> reader = rig.reader(Qos.readerKeepAll());
            DataWriter<QosFixtures.Item> writer = rig.writer(Qos.writer()
                    .withPolicy(Qos.pf().Lifespan().withDuration(3, TimeUnit.SECONDS)));
            writer.write(new QosFixtures.Item(1, 1));
            assertEquals(Arrays.asList(1), seqs(Qos.read(reader)), "valid for 3 s");
            awaitTrue("the sample expires", () -> Qos.read(reader).isEmpty());
        }
    }

    @Test
    void anInfiniteLifespanNeverExpires() throws Exception {
        try (QosFixtures.Rig rig = new QosFixtures.Rig()) {
            DataReader<QosFixtures.Item> reader = rig.reader(Qos.readerKeepAll());
            DataWriter<QosFixtures.Item> writer = rig.writer(Qos.writer());
            writer.write(new QosFixtures.Item(1, 1), 1L, TimeUnit.MILLISECONDS); // 1 ms after the epoch
            assertEquals(Arrays.asList(1), seqs(Qos.take(reader)));
        }
    }

    @Test
    void aLateJoinerIsNotReplayedAnExpiredSampleOfATransientLocalWriter() throws Exception {
        try (QosFixtures.Rig rig = new QosFixtures.Rig()) {
            DataWriter<QosFixtures.Item> writer = rig.writer(Qos.writer()
                    .withPolicy(Qos.pf().Durability().withTransientLocal())
                    .withPolicy(Qos.pf().Lifespan().withDuration(60, TimeUnit.SECONDS)));
            long now = System.currentTimeMillis();
            writer.write(new QosFixtures.Item(1, 1), now - 120_000L, TimeUnit.MILLISECONDS);
            writer.write(new QosFixtures.Item(2, 2), now, TimeUnit.MILLISECONDS);

            DataReader<QosFixtures.Item> late = rig.reader(Qos.readerKeepAll()
                    .withPolicy(Qos.pf().Durability().withTransientLocal()));
            assertEquals(Arrays.asList(2), seqs(Qos.take(late)));
        }
    }

    // --- TIME_BASED_FILTER -------------------------------------------------------------

    private static DataReaderQos separated(long millis) {
        return Qos.readerKeepAll().withPolicy(
                Qos.pf().TimeBasedFilter().withMinimumSeparation(millis, TimeUnit.MILLISECONDS));
    }

    @Test
    void aReaderWithMinimumSeparationTakesOneSamplePerInstanceInThatTime() throws Exception {
        try (QosFixtures.Rig rig = new QosFixtures.Rig()) {
            DataReader<QosFixtures.Item> reader = rig.reader(separated(3_600_000L));
            DataWriter<QosFixtures.Item> writer = rig.writer(Qos.writer());
            writer.write(new QosFixtures.Item(1, 0));
            writer.write(new QosFixtures.Item(1, 1));
            writer.write(new QosFixtures.Item(1, 2));
            writer.write(new QosFixtures.Item(2, 0)); // the filter is per instance
            writer.write(new QosFixtures.Item(2, 1));

            List<Sample<QosFixtures.Item>> got = Qos.take(reader);
            List<String> held = new ArrayList<>();
            for (Sample<QosFixtures.Item> sample : got) {
                held.add(sample.getData().id() + ":" + sample.getData().seq());
            }
            assertEquals(Arrays.asList("1:0", "2:0"), held);
        }
    }

    @Test
    void theNewestSampleHeldBackIsDeliveredWhenTheSeparationHasPassed() throws Exception {
        try (QosFixtures.Rig rig = new QosFixtures.Rig()) {
            DataReader<QosFixtures.Item> reader = rig.reader(separated(500L));
            DataWriter<QosFixtures.Item> writer = rig.writer(Qos.writer());
            writer.write(new QosFixtures.Item(1, 0));
            writer.write(new QosFixtures.Item(1, 1));
            writer.write(new QosFixtures.Item(1, 2));

            // 0 is accepted; 1 and 2 arrive within 500 ms of it, 1 is overtaken by 2, and 2,
            // the last one, reaches the reader when the 500 ms have passed (§2.2.3.12).
            List<Integer> seen = new ArrayList<>();
            awaitTrue("the last sample arrives", () -> {
                seen.addAll(seqs(Qos.take(reader)));
                return seen.contains(2);
            });
            assertEquals(Arrays.asList(0, 2), seen);
        }
    }

    @Test
    void withoutMinimumSeparationEverySampleIsDelivered() throws Exception {
        try (QosFixtures.Rig rig = new QosFixtures.Rig()) {
            DataReader<QosFixtures.Item> reader = rig.reader(Qos.readerKeepAll());
            DataWriter<QosFixtures.Item> writer = rig.writer(Qos.writer());
            for (int seq = 0; seq < 5; seq++) {
                writer.write(new QosFixtures.Item(1, seq));
            }
            assertEquals(Arrays.asList(0, 1, 2, 3, 4), seqs(Qos.take(reader)));
        }
    }

    // --- DESTINATION_ORDER -------------------------------------------------------------

    @Test
    void bySourceTimestampASampleOlderThanTheNewestOfTheInstanceIsOrderedBeforeIt() throws Exception {
        try (QosFixtures.Rig rig = new QosFixtures.Rig()) {
            DataReader<QosFixtures.Item> reader = rig.reader(Qos.readerKeepAll()
                    .withPolicy(Qos.pf().DestinationOrder().withSourceTimestamp()));
            DataWriter<QosFixtures.Item> writer = rig.writer(Qos.writer()
                    .withPolicy(Qos.pf().DestinationOrder().withSourceTimestamp()));
            long base = System.currentTimeMillis();
            writer.write(new QosFixtures.Item(1, 1), base + 2000, TimeUnit.MILLISECONDS);
            writer.write(new QosFixtures.Item(1, 2), base + 1000, TimeUnit.MILLISECONDS); // older: ordered before the first
            writer.write(new QosFixtures.Item(1, 3), base + 3000, TimeUnit.MILLISECONDS);
            writer.write(new QosFixtures.Item(1, 4), base + 3000, TimeUnit.MILLISECONDS); // equal: kept
            writer.write(new QosFixtures.Item(2, 5), base + 500, TimeUnit.MILLISECONDS);  // another instance
            // DDS 1.4 2.2.2.5.3.8: the samples of an instance appear in the order of their source
            // timestamps (smaller ahead of larger, equal ones in arrival order); none is lost
            assertEquals(Arrays.asList(2, 1, 3, 4, 5), seqs(Qos.take(reader)));
        }
    }

    @Test
    void byReceptionTimestampTheSourceTimestampDoesNotDecide() throws Exception {
        try (QosFixtures.Rig rig = new QosFixtures.Rig()) {
            DataReader<QosFixtures.Item> reader = rig.reader(Qos.readerKeepAll());
            DataWriter<QosFixtures.Item> writer = rig.writer(Qos.writer());
            long base = System.currentTimeMillis();
            writer.write(new QosFixtures.Item(1, 1), base + 2000, TimeUnit.MILLISECONDS);
            writer.write(new QosFixtures.Item(1, 2), base + 1000, TimeUnit.MILLISECONDS);
            assertEquals(Arrays.asList(1, 2), seqs(Qos.take(reader)));
        }
    }

    // --- READER_DATA_LIFECYCLE -----------------------------------------------------------

    private static DataReaderQos purging(long noWritersSeconds, long disposedSeconds) {
        return Qos.readerKeepAll().withPolicy(Qos.pf().ReaderDataLifecycle()
                .withAutoPurgeNoWriterSamplesDelay(noWritersSeconds, TimeUnit.SECONDS)
                .withAutoPurgeDisposedSamplesDelay(disposedSeconds, TimeUnit.SECONDS));
    }

    @Test
    void byDefaultAnInstanceWithoutWritersKeepsItsSamplesUntilTheyAreTaken() throws Exception {
        try (QosFixtures.Rig rig = new QosFixtures.Rig()) {
            DataReader<QosFixtures.Item> reader = rig.reader(Qos.readerKeepAll());
            DataWriter<QosFixtures.Item> writer = rig.writer(autodispose(false));
            writer.write(new QosFixtures.Item(1, 1));
            Qos.unregister(writer, new QosFixtures.Item(1, 0));
            Thread.sleep(100);
            // the sample and the notice of the state change
            assertEquals(2, Qos.take(reader).size());
        }
    }

    @Test
    void aZeroDelayPurgesTheInstanceWithoutWritersWithItsUntakenSamples() throws Exception {
        try (QosFixtures.Rig rig = new QosFixtures.Rig()) {
            DataReader<QosFixtures.Item> reader = rig.reader(purging(0, 100_000));
            DataWriter<QosFixtures.Item> writer = rig.writer(autodispose(false));
            writer.write(new QosFixtures.Item(1, 1));
            Qos.unregister(writer, new QosFixtures.Item(1, 0));
            assertEquals(0, Qos.take(reader).size());
        }
    }

    @Test
    void aZeroDelayPurgesTheSamplesOfADisposedInstance() throws Exception {
        try (QosFixtures.Rig rig = new QosFixtures.Rig()) {
            DataReader<QosFixtures.Item> reader = rig.reader(purging(100_000, 0));
            DataWriter<QosFixtures.Item> writer = rig.writer(Qos.writer());
            writer.write(new QosFixtures.Item(1, 1));
            Qos.dispose(writer, new QosFixtures.Item(1, 0));
            assertEquals(0, Qos.take(reader).size());
        }
    }

    @Test
    void theDelayOfTheOtherStateDoesNotPurge() throws Exception {
        try (QosFixtures.Rig rig = new QosFixtures.Rig()) {
            // only the no-writers delay is short: a disposed instance keeps its samples
            DataReader<QosFixtures.Item> reader = rig.reader(purging(0, 100_000));
            DataWriter<QosFixtures.Item> writer = rig.writer(autodispose(true));
            writer.write(new QosFixtures.Item(1, 1));
            Qos.unregister(writer, new QosFixtures.Item(1, 0)); // disposes, then unregisters
            assertEquals(2, Qos.take(reader).size());
        }
    }

    @Test
    void aFiniteDelayPurgesWhenItHasPassedWithoutAnyAccess() throws Exception {
        try (QosFixtures.Rig rig = new QosFixtures.Rig()) {
            DataReader<QosFixtures.Item> reader = rig.reader(purging(2, 100_000));
            DataWriter<QosFixtures.Item> writer = rig.writer(autodispose(false));
            writer.write(new QosFixtures.Item(1, 1));
            Qos.unregister(writer, new QosFixtures.Item(1, 0));
            assertEquals(2, Qos.read(reader).size(), "within the 2 s the samples are there");
            awaitTrue("the 2 s pass and the instance is purged", () -> Qos.read(reader).isEmpty());
        }
    }
}
