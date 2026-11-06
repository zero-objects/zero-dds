// SPDX-License-Identifier: Apache-2.0
package org.omg.dds;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.omg.dds.core.Time;
import org.omg.dds.core.policy.Durability;
import org.omg.dds.domain.DomainParticipant;
import org.omg.dds.pub.DataWriter;
import org.omg.dds.pub.DataWriterQos;
import org.omg.dds.sub.DataReader;
import org.omg.dds.sub.Sample;
import org.omg.dds.topic.Topic;
import org.zerodds.internal.DurabilityService;
import org.zerodds.internal.InProcessBus;

import java.util.List;
import java.util.concurrent.TimeUnit;

import static org.junit.jupiter.api.Assertions.assertEquals;

/**
 * LIFESPAN of the DataWriter (DDS 1.4 §2.2.3.16): a sample is valid from its source timestamp
 * for the lifespan, and is not delivered once it has expired, to a reader that is there and to
 * one that joins later. Domain ids are 1360 to 1369.
 */
class WriterLifespanTest {

    @BeforeEach
    void setUp() {
        InProcessBus.instance().reset();
        DurabilityService.instance().reset();
    }

    @AfterEach
    void tearDown() {
        InProcessBus.instance().reset();
        DurabilityService.instance().reset();
    }

    private static Time time(long millis) {
        return Qos.env().getSPI().newTime(millis, TimeUnit.MILLISECONDS);
    }

    private static DataWriterQos lifespan(long millis, Durability durability) {
        return Qos.writer()
                .withPolicy(Qos.pf().Lifespan().withDuration(millis, TimeUnit.MILLISECONDS))
                .withPolicy(durability)
                // the tests close writers whose data the Durability service keeps; closing
                // unregisters, and an unregister disposes with autodispose_unregistered_instances
                // true (DDS 1.4 2.2.3.21)
                .withPolicy(Qos.pf().WriterDataLifecycle().withAutDisposeUnregisteredInstances(false))
                .withPolicy(Qos.pf().History().withKeepAll());
    }

    /** Waits until the wall clock is past {@code millis}. */
    private static void waitUntilPast(long millis) throws InterruptedException {
        long end = System.nanoTime() + 20_000_000_000L;
        while (System.currentTimeMillis() <= millis && System.nanoTime() < end) {
            Thread.sleep(5);
        }
    }

    @Test
    void javapsm_7_3_5_1_a_replayed_transient_local_sample_keeps_the_source_timestamp_it_was_written_with() throws Exception {
        DomainParticipant participant = Qos.factory().createParticipant(1363);
        Topic<byte[]> topic = participant.createTopic("ReplayStamp", byte[].class);
        DataWriter<byte[]> writer = participant.createPublisher().createDataWriter(topic,
                lifespan(60_000, Qos.pf().Durability().withTransientLocal()));
        long stamp = System.currentTimeMillis() - 5_000;
        writer.write(new byte[] {7}, time(stamp));

        DataReader<byte[]> late = participant.createSubscriber().createDataReader(topic,
                Qos.readerKeepAll().withPolicy(Qos.pf().Durability().withTransientLocal()));
        List<Sample<byte[]>> samples = Qos.take(late);
        assertEquals(1, samples.size());
        assertEquals(stamp, samples.get(0).getSourceTimestamp().getTime(TimeUnit.MILLISECONDS));
    }

}
