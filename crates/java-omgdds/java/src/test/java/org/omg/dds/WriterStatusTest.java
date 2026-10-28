// SPDX-License-Identifier: Apache-2.0
package org.omg.dds;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.omg.dds.core.InstanceHandle;
import org.omg.dds.core.StatusCondition;
import org.omg.dds.core.event.LivelinessLostEvent;
import org.omg.dds.core.event.OfferedIncompatibleQosEvent;
import org.omg.dds.core.policy.QosPolicyCount;
import org.omg.dds.core.policy.Reliability;
import org.omg.dds.core.status.LivelinessLostStatus;
import org.omg.dds.core.status.OfferedDeadlineMissedStatus;
import org.omg.dds.core.status.OfferedIncompatibleQosStatus;
import org.omg.dds.core.status.PublicationMatchedStatus;
import org.omg.dds.domain.DomainParticipant;
import org.omg.dds.pub.DataWriter;
import org.omg.dds.pub.DataWriterAdapter;
import org.omg.dds.sub.DataReader;
import org.omg.dds.topic.Topic;
import org.zerodds.internal.InProcessBus;

import java.util.Set;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicReference;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * The communication statuses of the DataWriter (DDS 1.4 §2.2.4.1): PUBLICATION_MATCHED with the
 * handle of the last subscription, OFFERED_INCOMPATIBLE_QOS, OFFERED_DEADLINE_MISSED,
 * LIVELINESS_LOST, the status changes, and assert_liveliness of the writer and of the
 * participant. Domain ids 1350 to 1379.
 */
class WriterStatusTest {

    /** The longest the tests wait for a condition; it is the bound, not the usual time. */
    private static final long WAIT_MILLIS = 20_000;

    @BeforeEach
    void setUp() {
        InProcessBus.instance().reset();
    }

    @AfterEach
    void tearDown() {
        InProcessBus.instance().reset();
    }

    private interface Condition {
        boolean holds();
    }

    private static boolean await(Condition condition) throws InterruptedException {
        long end = System.nanoTime() + WAIT_MILLIS * 1_000_000L;
        while (System.nanoTime() < end) {
            if (condition.holds()) {
                return true;
            }
            Thread.sleep(10);
        }
        return condition.holds();
    }

    // --- PUBLICATION_MATCHED (§2.2.4.1) ---------------------------------------------------

    @Test
    void thePublicationMatchedStatusCountsAndNamesTheLastSubscription() {
        DomainParticipant participant = Qos.factory().createParticipant(1350);
        Topic<byte[]> topic = participant.createTopic("Matched", byte[].class);
        DataWriter<byte[]> writer = participant.createPublisher().createDataWriter(topic);

        PublicationMatchedStatus none = writer.getPublicationMatchedStatus();
        assertEquals(0, none.getTotalCount());
        assertEquals(0, none.getCurrentCount());

        DataReader<byte[]> first = participant.createSubscriber().createDataReader(topic);
        PublicationMatchedStatus one = writer.getPublicationMatchedStatus();
        assertEquals(1, one.getTotalCount());
        assertEquals(1, one.getTotalCountChange());
        assertEquals(1, one.getCurrentCount());
        assertEquals(1, one.getCurrentCountChange());
        assertEquals(first.getInstanceHandle(), one.getLastSubscriptionHandle());
        Set<InstanceHandle> matched = writer.getMatchedSubscriptions();
        assertEquals(1, matched.size());
        assertTrue(matched.contains(one.getLastSubscriptionHandle()));

        // reading the status resets the change counts
        PublicationMatchedStatus again = writer.getPublicationMatchedStatus();
        assertEquals(0, again.getTotalCountChange());
        assertEquals(0, again.getCurrentCountChange());
        assertEquals(first.getInstanceHandle(), again.getLastSubscriptionHandle());

        DataReader<byte[]> second = participant.createSubscriber().createDataReader(topic);
        second.close();
        // two changes since the last read: the reader matched and left
        PublicationMatchedStatus left = writer.getPublicationMatchedStatus();
        assertEquals(2, left.getTotalCount());
        assertEquals(1, left.getTotalCountChange());
        assertEquals(1, left.getCurrentCount());
        assertEquals(0, left.getCurrentCountChange());
        assertEquals(second.getInstanceHandle(), left.getLastSubscriptionHandle());

        first.close();
        PublicationMatchedStatus gone = writer.getPublicationMatchedStatus();
        assertEquals(0, gone.getCurrentCount());
        assertEquals(-1, gone.getCurrentCountChange());
        assertEquals(0, gone.getTotalCountChange());
        assertEquals(first.getInstanceHandle(), gone.getLastSubscriptionHandle());
    }

    @Test
    void theStatusChangesOfAWriterListThePendingStatusesUntilTheyAreRead() {
        DomainParticipant participant = Qos.factory().createParticipant(1351);
        Topic<byte[]> topic = participant.createTopic("Changes", byte[].class);
        DataWriter<byte[]> writer = participant.createPublisher().createDataWriter(topic);
        StatusCondition<DataWriter<byte[]>> condition = writer.getStatusCondition();
        assertTrue(writer.getStatusChanges().isEmpty());
        assertFalse(condition.getTriggerValue());

        participant.createSubscriber().createDataReader(topic);
        assertTrue(writer.getStatusChanges().contains(PublicationMatchedStatus.class));
        // asking does not reset
        assertTrue(writer.getStatusChanges().contains(PublicationMatchedStatus.class));
        assertTrue(condition.getTriggerValue());

        writer.getPublicationMatchedStatus();
        assertFalse(writer.getStatusChanges().contains(PublicationMatchedStatus.class));
        assertFalse(condition.getTriggerValue());
    }

    // --- OFFERED_INCOMPATIBLE_QOS ----------------------------------------------------------

    @Test
    void anIncompatibleReaderIsCountedWithTheRefusedPolicy() {
        DomainParticipant participant = Qos.factory().createParticipant(1352);
        Topic<byte[]> topic = participant.createTopic("Incompatible", byte[].class);
        AtomicReference<OfferedIncompatibleQosStatus> listened = new AtomicReference<>();
        DataWriter<byte[]> writer = participant.createPublisher().createDataWriter(topic,
                Qos.writer().withPolicy(Qos.pf().Reliability().withBestEffort()),
                new DataWriterAdapter<byte[]>() {
                    @Override
                    public void onOfferedIncompatibleQos(OfferedIncompatibleQosEvent<byte[]> status) {
                        listened.set(status.getStatus());
                    }
                }, null);
        participant.createSubscriber().createDataReader(topic,
                Qos.reader().withPolicy(Qos.pf().Reliability().withReliable()));

        // the listener is told with the status as it was when the reader was refused
        assertNotNull(listened.get());
        assertEquals(1, listened.get().getTotalCount());
        assertEquals(Reliability.class, listened.get().getLastPolicyClass());

        OfferedIncompatibleQosStatus status = writer.getOfferedIncompatibleQosStatus();
        assertEquals(1, status.getTotalCount());
        assertEquals(Reliability.class, status.getLastPolicyClass());
        assertEquals(1, status.getPolicies().size());
        QosPolicyCount count = status.getPolicies().iterator().next();
        assertEquals(Reliability.class, count.getPolicyClass());
        assertEquals(1, count.getCount());
        assertEquals(0, writer.getPublicationMatchedStatus().getCurrentCount());
    }

    // --- OFFERED_DEADLINE_MISSED -----------------------------------------------------------------

    @Test
    void aMissedOfferedDeadlineIsCountedWithTheInstanceThatMissedIt() throws Exception {
        DomainParticipant participant = Qos.factory().createParticipant(1353);
        Topic<byte[]> topic = participant.createTopic("DeadlineMissed", byte[].class);
        DataWriter<byte[]> writer = participant.createPublisher().createDataWriter(topic,
                Qos.writer().withPolicy(Qos.pf().Deadline().withPeriod(30, TimeUnit.MILLISECONDS)));
        writer.write(new byte[] {1});
        InstanceHandle instance = writer.lookupInstance(new byte[] {1});

        assertTrue(await(() -> writer.getStatusChanges().contains(OfferedDeadlineMissedStatus.class)));
        OfferedDeadlineMissedStatus first = writer.getOfferedDeadlineMissedStatus();
        assertTrue(first.getTotalCount() >= 1);
        assertEquals(first.getTotalCount(), first.getTotalCountChange());
        assertEquals(instance, first.getLastInstanceHandle());

        // the change counts what happened since the last read
        OfferedDeadlineMissedStatus second = writer.getOfferedDeadlineMissedStatus();
        assertTrue(second.getTotalCount() >= first.getTotalCount());
        assertEquals(second.getTotalCount() - first.getTotalCount(), second.getTotalCountChange());
    }

    // --- LIVELINESS_LOST and assert_liveliness -----------------------------------------------------

    @Test
    void aWriterThatDoesNotAssertItsLivelinessLosesItOnceAndAssertingRestoresIt() throws Exception {
        DomainParticipant participant = Qos.factory().createParticipant(1354);
        Topic<byte[]> topic = participant.createTopic("LivelinessLost", byte[].class);
        CountDownLatch lost = new CountDownLatch(1);
        AtomicReference<LivelinessLostStatus> listened = new AtomicReference<>();
        DataWriter<byte[]> writer = participant.createPublisher().createDataWriter(topic,
                Qos.writer().withPolicy(Qos.pf().Liveliness().withManualByTopic()
                        .withLeaseDuration(100, TimeUnit.MILLISECONDS)),
                new DataWriterAdapter<byte[]>() {
                    @Override
                    public void onLivelinessLost(LivelinessLostEvent<byte[]> status) {
                        listened.set(status.getStatus());
                        lost.countDown();
                    }
                }, null);
        assertTrue(lost.await(WAIT_MILLIS, TimeUnit.MILLISECONDS));
        assertTrue(listened.get().getTotalCount() >= 1);

        // the change counts were reset when the listener was told (DDS 1.4 §2.2.4.1)
        LivelinessLostStatus first = writer.getLivelinessLostStatus();
        assertTrue(first.getTotalCount() >= 1);
        assertEquals(0, first.getTotalCountChange());
        // no lease runs while the writer is not alive: nothing more is counted
        LivelinessLostStatus second = writer.getLivelinessLostStatus();
        assertEquals(first.getTotalCount(), second.getTotalCount());
        assertEquals(0, second.getTotalCountChange());

        // assert_liveliness makes it alive again; without more assertions it is lost again
        writer.assertLiveliness();
        assertTrue(await(() -> writer.getLivelinessLostStatus().getTotalCount()
                > first.getTotalCount()));
    }

    @Test
    void assertLivelinessOfTheWriterKeepsALeaseFromRunningOut() throws Exception {
        DomainParticipant participant = Qos.factory().createParticipant(1355);
        Topic<byte[]> topic = participant.createTopic("LivelinessKept", byte[].class);
        DataWriter<byte[]> writer = participant.createPublisher().createDataWriter(topic,
                Qos.writer().withPolicy(Qos.pf().Liveliness().withManualByTopic()
                        .withLeaseDuration(1500, TimeUnit.MILLISECONDS)));
        long end = System.nanoTime() + 3_000_000_000L;
        while (System.nanoTime() < end) {
            writer.assertLiveliness();
            Thread.sleep(20);
        }
        assertEquals(0, writer.getLivelinessLostStatus().getTotalCount());
        // once the assertions stop, the lease runs out
        assertTrue(await(() -> writer.getLivelinessLostStatus().getTotalCount() >= 1));
    }

    @Test
    void assertLivelinessOfTheParticipantRenewsTheWritersWithManualByParticipant() throws Exception {
        DomainParticipant participant = Qos.factory().createParticipant(1356);
        Topic<byte[]> topic = participant.createTopic("ParticipantLiveliness", byte[].class);
        Topic<byte[]> other = participant.createTopic("ParticipantLivelinessByTopic", byte[].class);
        org.omg.dds.pub.Publisher publisher = participant.createPublisher();
        DataWriter<byte[]> byParticipant = publisher.createDataWriter(topic,
                Qos.writer().withPolicy(Qos.pf().Liveliness().withManualByParticipant()
                        .withLeaseDuration(1500, TimeUnit.MILLISECONDS)));
        DataWriter<byte[]> byTopic = publisher.createDataWriter(other,
                Qos.writer().withPolicy(Qos.pf().Liveliness().withManualByTopic()
                        .withLeaseDuration(1500, TimeUnit.MILLISECONDS)));
        long end = System.nanoTime() + 3_000_000_000L;
        while (System.nanoTime() < end) {
            participant.assertLiveliness();
            Thread.sleep(20);
        }
        // DDS 1.4 §2.2.2.2.1.19: it affects only the MANUAL_BY_PARTICIPANT writers
        assertEquals(0, byParticipant.getLivelinessLostStatus().getTotalCount());
        assertTrue(byTopic.getLivelinessLostStatus().getTotalCount() >= 1);
    }

    @Test
    void aWriteOfOneWriterAssertsTheLivelinessOfTheParticipantsOtherManualWriters() throws Exception {
        DomainParticipant participant = Qos.factory().createParticipant(1357);
        Topic<byte[]> idle = participant.createTopic("ParticipantIdle", byte[].class);
        Topic<byte[]> busy = participant.createTopic("ParticipantBusy", byte[].class);
        org.omg.dds.pub.Publisher publisher = participant.createPublisher();
        DataWriter<byte[]> idleWriter = publisher.createDataWriter(idle,
                Qos.writer().withPolicy(Qos.pf().Liveliness().withManualByParticipant()
                        .withLeaseDuration(1500, TimeUnit.MILLISECONDS)));
        DataWriter<byte[]> busyWriter = publisher.createDataWriter(busy,
                Qos.writer().withPolicy(Qos.pf().Liveliness().withManualByParticipant()
                        .withLeaseDuration(1500, TimeUnit.MILLISECONDS)));
        long end = System.nanoTime() + 3_000_000_000L;
        while (System.nanoTime() < end) {
            busyWriter.write(new byte[] {1});
            Thread.sleep(20);
        }
        // DDS 1.4 §2.2.2.2.1.19, NOTE: a write asserts the liveliness of the participant
        assertEquals(0, idleWriter.getLivelinessLostStatus().getTotalCount());
        assertSame(publisher, idleWriter.getParent());
    }
}
