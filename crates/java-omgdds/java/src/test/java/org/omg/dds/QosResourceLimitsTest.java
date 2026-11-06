// SPDX-License-Identifier: Apache-2.0
package org.omg.dds;

import org.zerodds.testing.Waits;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.omg.dds.core.OutOfResourcesException;
import org.omg.dds.core.policy.ResourceLimits;
import org.omg.dds.core.status.SampleRejectedStatus;
import org.omg.dds.pub.DataWriter;
import org.omg.dds.pub.DataWriterQos;
import org.omg.dds.sub.DataReader;
import org.omg.dds.sub.Sample;
import org.zerodds.internal.InProcessBus;

import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.TimeoutException;
import java.util.concurrent.atomic.AtomicReference;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertThrows;

/**
 * RESOURCE_LIMITS (DDS 1.4 §2.2.3.19) at the writer, where a RELIABLE write waits up to
 * {@code max_blocking_time} (§2.2.3.14) and then times out, and at the reader, which rejects
 * (SAMPLE_REJECTED, §2.2.4.1). Domain ids 1400 to 1499.
 */
class QosResourceLimitsTest {

    @BeforeEach
    void setUp() {
        InProcessBus.instance().reset();
    }

    @AfterEach
    void tearDown() {
        InProcessBus.instance().reset();
    }

    private static DataWriterQos limited(ResourceLimits limits, long blockingMillis) {
        return Qos.writer()
                .withPolicy(limits)
                .withPolicy(Qos.policies().DataWriterReliability()
                        .withMaxBlockingTime(blockingMillis, TimeUnit.MILLISECONDS));
    }

    // --- writer: max_instances -----------------------------------------------------------

    @Test
    void javapsm_7_3_5_1_a_write_to_a_new_instance_beyond_max_instances_times_out_until_an_instance_is_unregistered() throws Exception {
        try (QosFixtures.Rig rig = new QosFixtures.Rig()) {
            DataWriter<QosFixtures.Item> writer = rig.writer(
                    limited(Qos.pf().ResourceLimits().withMaxInstances(1), 50));
            writer.write(new QosFixtures.Item(1, 0));
            writer.write(new QosFixtures.Item(1, 1)); // the same instance needs no new one
            assertThrows(TimeoutException.class, () -> writer.write(new QosFixtures.Item(2, 0)));

            Qos.unregister(writer, new QosFixtures.Item(1, 0));
            writer.write(new QosFixtures.Item(2, 0)); // the freed instance is taken
        }
    }

    @Test
    void javapsm_7_3_5_1_a_blocked_write_continues_when_another_thread_frees_the_instance() throws Exception {
        try (QosFixtures.Rig rig = new QosFixtures.Rig()) {
            // a long max_blocking_time: only the unregister of the other thread can end the wait in time
            DataWriter<QosFixtures.Item> writer = rig.writer(
                    limited(Qos.pf().ResourceLimits().withMaxInstances(1), 20_000));
            writer.write(new QosFixtures.Item(1, 0));
            AtomicReference<Throwable> failure = new AtomicReference<>();
            Thread blocked = Thread.currentThread();
            Thread freeing = new Thread(() -> {
                try {
                    Waits.untilWaitingIn(blocked, "write");
                    Qos.unregister(writer, new QosFixtures.Item(1, 0));
                } catch (Throwable t) {
                    failure.set(t);
                }
            });
            freeing.start();
            writer.write(new QosFixtures.Item(2, 0)); // returns once the instance is free
            freeing.join(20_000);
            assertNull(failure.get());
        }
    }

    @Test
    void javapsm_7_3_5_1_a_best_effort_writer_does_not_wait_and_is_out_of_resources() throws Exception {
        try (QosFixtures.Rig rig = new QosFixtures.Rig()) {
            DataWriter<QosFixtures.Item> writer = rig.writer(Qos.writer()
                    .withPolicy(Qos.pf().ResourceLimits().withMaxInstances(1))
                    .withPolicy(Qos.policies().DataWriterReliability().withBestEffort()));
            writer.write(new QosFixtures.Item(1, 0));
            assertThrows(OutOfResourcesException.class, () -> writer.write(new QosFixtures.Item(2, 0)));
        }
    }

    // --- writer: the history kept for late joiners ------------------------------------------

    @Test
    void javapsm_7_3_5_1_keep_all_with_max_samples_per_instance_times_out_for_transient_local() throws Exception {
        try (QosFixtures.Rig rig = new QosFixtures.Rig()) {
            DataWriter<QosFixtures.Item> writer = rig.writer(limited(
                    Qos.pf().ResourceLimits().withMaxSamplesPerInstance(2), 50)
                    .withPolicy(Qos.pf().Durability().withTransientLocal())
                    .withPolicy(Qos.pf().History().withKeepAll()));
            writer.write(new QosFixtures.Item(1, 0));
            writer.write(new QosFixtures.Item(1, 1));
            assertThrows(TimeoutException.class, () -> writer.write(new QosFixtures.Item(1, 2)));
            writer.write(new QosFixtures.Item(2, 0)); // another instance has its own two

            // a disposed instance gives its samples back
            Qos.dispose(writer, new QosFixtures.Item(1, 0));
            writer.write(new QosFixtures.Item(1, 3));
        }
    }

    @Test
    void javapsm_7_3_5_1_max_samples_bounds_the_history_of_all_instances_together() throws Exception {
        try (QosFixtures.Rig rig = new QosFixtures.Rig()) {
            DataWriter<QosFixtures.Item> writer = rig.writer(limited(
                    Qos.pf().ResourceLimits().withMaxSamples(2), 50)
                    .withPolicy(Qos.pf().Durability().withTransientLocal())
                    .withPolicy(Qos.pf().History().withKeepAll()));
            writer.write(new QosFixtures.Item(1, 0));
            writer.write(new QosFixtures.Item(2, 0));
            assertThrows(TimeoutException.class, () -> writer.write(new QosFixtures.Item(3, 0)));
            assertThrows(TimeoutException.class, () -> writer.write(new QosFixtures.Item(1, 1)));
        }
    }

    @Test
    void javapsm_7_3_5_1_keep_last_replaces_the_oldest_sample_of_the_instance_and_needs_no_new_resource() throws Exception {
        try (QosFixtures.Rig rig = new QosFixtures.Rig()) {
            DataWriter<QosFixtures.Item> writer = rig.writer(limited(
                    Qos.pf().ResourceLimits().withMaxSamples(1), 50)
                    .withPolicy(Qos.pf().Durability().withTransientLocal())
                    .withPolicy(Qos.pf().History().withKeepLast(1)));
            writer.write(new QosFixtures.Item(1, 0));
            writer.write(new QosFixtures.Item(1, 1)); // replaces the sample, still one
            assertThrows(TimeoutException.class, () -> writer.write(new QosFixtures.Item(2, 0)));
        }
    }

    @Test
    void javapsm_7_3_5_1_with_no_limit_a_writer_never_waits() throws Exception {
        try (QosFixtures.Rig rig = new QosFixtures.Rig()) {
            DataWriter<QosFixtures.Item> writer = rig.writer(Qos.writer()
                    .withPolicy(Qos.pf().Durability().withTransientLocal())
                    .withPolicy(Qos.pf().History().withKeepAll()));
            for (int i = 0; i < 300; i++) {
                writer.write(new QosFixtures.Item(i % 7, i));
            }
        }
    }

    // --- reader ------------------------------------------------------------------------

    @Test
    void javapsm_7_3_5_1_the_default_limits_are_unlimited_and_reject_nothing() throws Exception {
        assertEquals(ResourceLimits.LENGTH_UNLIMITED, Qos.reader().getResourceLimits().getMaxSamples());
        assertEquals(ResourceLimits.LENGTH_UNLIMITED, Qos.reader().getResourceLimits().getMaxInstances());
        assertEquals(ResourceLimits.LENGTH_UNLIMITED,
                Qos.reader().getResourceLimits().getMaxSamplesPerInstance());
        try (QosFixtures.Rig rig = new QosFixtures.Rig()) {
            DataReader<QosFixtures.Item> reader = rig.reader(Qos.readerKeepAll());
            DataWriter<QosFixtures.Item> writer = rig.writer(Qos.writer());
            for (int i = 0; i < 50; i++) {
                writer.write(new QosFixtures.Item(i % 5, i));
            }
            assertEquals(50, Qos.take(reader).size());
            assertEquals(0, reader.getSampleRejectedStatus().getTotalCount());
        }
    }
}
