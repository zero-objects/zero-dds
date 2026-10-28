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
    private static final long PASS_MS = 600;

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

    @Test
    void theParticipantAssertsTheLivelinessOfItsManualByParticipantWriters() throws Exception {
        try (QosFixtures.Rig rig = new QosFixtures.Rig()) {
            DataWriter<QosFixtures.Item> writer = rig.writer(manualByParticipant());
            assertTrue(alive(writer));
            Thread.sleep(PASS_MS);
            assertFalse(alive(writer), "the lease of 300 ms has run out");
            rig.participant.assertLiveliness();
            assertTrue(alive(writer));
        }
    }

    @Test
    void theParticipantDoesNotAssertTheLivelinessOfAManualByTopicWriter() throws Exception {
        try (QosFixtures.Rig rig = new QosFixtures.Rig()) {
            DataWriter<QosFixtures.Item> writer = rig.writer(
                    writerOf(Qos.pf().Liveliness().withManualByTopic()));
            Thread.sleep(PASS_MS);
            rig.participant.assertLiveliness();
            assertFalse(alive(writer));
            writer.assertLiveliness();
            assertTrue(alive(writer));
        }
    }

    @Test
    void anAssertOnAnyWriterOfTheParticipantAssertsTheManualByParticipantWriters() throws Exception {
        try (QosFixtures.Rig rig = new QosFixtures.Rig()) {
            DataWriter<QosFixtures.Item> silent = rig.writer(manualByParticipant());
            DataWriter<QosFixtures.Item> other = rig.writer(Qos.writer());
            Thread.sleep(PASS_MS);
            assertFalse(alive(silent));
            other.assertLiveliness();
            assertTrue(alive(silent));
        }
    }

    @Test
    void aWriteByAnyWriterOfTheParticipantAssertsTheManualByParticipantWriters() throws Exception {
        try (QosFixtures.Rig rig = new QosFixtures.Rig()) {
            DataWriter<QosFixtures.Item> silent = rig.writer(manualByParticipant());
            DataWriter<QosFixtures.Item> other = rig.writer(Qos.writer());
            Thread.sleep(PASS_MS);
            assertFalse(alive(silent));
            other.write(new QosFixtures.Item(1, 1));
            assertTrue(alive(silent));
        }
    }

    @Test
    void theWritersOfAnotherParticipantAreNotAsserted() throws Exception {
        try (QosFixtures.Rig rig = new QosFixtures.Rig()) {
            DomainParticipant elsewhere = Qos.factory().createParticipant(rig.participant.getDomainId());
            try {
                Topic<QosFixtures.Item> topic = elsewhere.createTopic("Items", QosFixtures.Item.class);
                Publisher publisher = elsewhere.createPublisher();
                DataWriter<QosFixtures.Item> foreign = Qos.writer(publisher, topic, QosFixtures.SUPPORT,
                        manualByParticipant());
                DataWriter<QosFixtures.Item> mine = rig.writer(manualByParticipant());
                Thread.sleep(PASS_MS);
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

    @Test
    void theDefaultLivelinessIsAutomaticWithAnInfiniteLease() {
        assertEquals(Liveliness.Kind.AUTOMATIC, Qos.writer().getLiveliness().getKind());
        assertTrue(Qos.writer().getLiveliness().getLeaseDuration().isInfinite());
    }
}
