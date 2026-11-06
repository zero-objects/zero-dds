// SPDX-License-Identifier: Apache-2.0
package org.omg.dds;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.omg.dds.core.policy.Liveliness;
import org.omg.dds.domain.DomainParticipant;
import org.omg.dds.pub.DataWriter;
import org.omg.dds.pub.DataWriterQos;
import org.omg.dds.pub.Publisher;
import org.omg.dds.topic.Topic;
import org.zerodds.internal.InProcessBus;
import org.zerodds.pub.DataWriterImpl;

import java.util.concurrent.TimeUnit;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * LIVELINESS kinds that a participant asserts for its writers (DDS 1.4 §2.2.3.11): a writer of
 * kind MANUAL_BY_PARTICIPANT is alive as long as one entity of its DomainParticipant has asserted
 * its liveliness, by {@code assert_liveliness} of the participant or of a writer or by a write.
 * Each test lets a lease of 300 ms pass before it asserts; a lease only runs out, so "no longer
 * alive after 600 ms" does not depend on timing. Domain ids 1400 to 1499.
 */
class QosLivelinessTest {

    private static final long LEASE_MS = 300;

    @BeforeEach
    void setUp() {
        InProcessBus.instance().reset();
    }

    @AfterEach
    void tearDown() {
        InProcessBus.instance().reset();
    }

    private static DataWriterQos writerOf(Liveliness liveliness) {
        return Qos.writer().withPolicy(liveliness.withLeaseDuration(LEASE_MS, TimeUnit.MILLISECONDS));
    }

    private static DataWriterQos manualByParticipant() {
        return writerOf(Qos.pf().Liveliness().withManualByParticipant());
    }

    private static boolean alive(DataWriter<?> writer) {
        return ((DataWriterImpl<?>) writer).isAlive();
    }

    /** Waits until the lease of {@code writer} has run out, at most 10 s. */
    private static void awaitLost(DataWriter<?> writer) throws InterruptedException {
        long end = System.nanoTime() + 10_000_000_000L;
        while (alive(writer) && System.nanoTime() < end) {
            Thread.sleep(10);
        }
        assertFalse(alive(writer), "the lease of " + LEASE_MS + " ms has run out");
    }

    @Test
    void javapsm_7_3_5_1_the_participant_asserts_the_liveliness_of_its_manual_by_participant_writers() throws Exception {
        try (QosFixtures.Rig rig = new QosFixtures.Rig()) {
            DataWriter<QosFixtures.Item> writer = rig.writer(manualByParticipant());
            assertTrue(alive(writer));
            awaitLost(writer);
            rig.participant.assertLiveliness();
            assertTrue(alive(writer));
        }
    }

    @Test
    void javapsm_7_3_5_1_the_participant_does_not_assert_the_liveliness_of_a_manual_by_topic_writer() throws Exception {
        try (QosFixtures.Rig rig = new QosFixtures.Rig()) {
            DataWriter<QosFixtures.Item> writer = rig.writer(
                    writerOf(Qos.pf().Liveliness().withManualByTopic()));
            awaitLost(writer);
            rig.participant.assertLiveliness();
            assertFalse(alive(writer));
            writer.assertLiveliness();
            assertTrue(alive(writer));
        }
    }

    @Test
    void javapsm_7_3_5_1_an_assert_on_any_writer_of_the_participant_asserts_the_manual_by_participant_writers() throws Exception {
        try (QosFixtures.Rig rig = new QosFixtures.Rig()) {
            DataWriter<QosFixtures.Item> silent = rig.writer(manualByParticipant());
            DataWriter<QosFixtures.Item> other = rig.writer(Qos.writer());
            awaitLost(silent);
            other.assertLiveliness();
            assertTrue(alive(silent));
        }
    }

    @Test
    void javapsm_7_3_5_1_a_write_by_any_writer_of_the_participant_asserts_the_manual_by_participant_writers() throws Exception {
        try (QosFixtures.Rig rig = new QosFixtures.Rig()) {
            DataWriter<QosFixtures.Item> silent = rig.writer(manualByParticipant());
            DataWriter<QosFixtures.Item> other = rig.writer(Qos.writer());
            awaitLost(silent);
            other.write(new QosFixtures.Item(1, 1));
            assertTrue(alive(silent));
        }
    }

    @Test
    void javapsm_7_3_5_1_the_writers_of_another_participant_are_not_asserted() throws Exception {
        try (QosFixtures.Rig rig = new QosFixtures.Rig()) {
            DomainParticipant elsewhere = Qos.factory().createParticipant(rig.participant.getDomainId());
            try {
                Topic<QosFixtures.Item> topic = elsewhere.createTopic("Items", QosFixtures.Item.class);
                Publisher publisher = elsewhere.createPublisher();
                DataWriter<QosFixtures.Item> foreign = Qos.writer(publisher, topic, QosFixtures.SUPPORT,
                        manualByParticipant());
                DataWriter<QosFixtures.Item> mine = rig.writer(manualByParticipant());
                awaitLost(mine);
                awaitLost(foreign);
                rig.participant.assertLiveliness();
                assertTrue(alive(mine));
                assertFalse(alive(foreign));
                elsewhere.assertLiveliness();
                assertTrue(alive(foreign));
            } finally {
                elsewhere.close();
            }
        }
    }

}
