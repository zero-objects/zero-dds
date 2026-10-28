// SPDX-License-Identifier: Apache-2.0
package org.omg.dds;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.omg.dds.core.InstanceHandle;
import org.omg.dds.core.policy.DestinationOrder;
import org.omg.dds.core.policy.LatencyBudget;
import org.omg.dds.core.policy.Presentation;
import org.omg.dds.core.policy.QosPolicy;
import org.omg.dds.core.policy.QosPolicyCount;
import org.omg.dds.core.policy.Reliability;
import org.omg.dds.core.status.LivelinessChangedStatus;
import org.omg.dds.core.status.RequestedDeadlineMissedStatus;
import org.omg.dds.core.status.RequestedIncompatibleQosStatus;
import org.omg.dds.core.status.SubscriptionMatchedStatus;
import org.omg.dds.pub.DataWriter;
import org.omg.dds.pub.Publisher;
import org.omg.dds.sub.DataReader;
import org.omg.dds.sub.Subscriber;
import org.zerodds.internal.InProcessBus;

import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.concurrent.TimeUnit;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;
import static org.omg.dds.ReaderRig.Keyed;
import static org.omg.dds.ReaderRig.sample;

/**
 * DDS 1.4 §2.2.4.1 and §2.2.2.5.3.22 to §2.2.2.5.3.26, §2.2.2.5.3.34: the fields of the
 * communication statuses of a reader that name a writer or an instance (last_publication_handle,
 * last_instance_handle), the policies of REQUESTED_INCOMPATIBLE_QOS, and the writers a reader is
 * matched with.
 */
class ReaderStatusDetailTest {

    private static int dom = 1110;

    private static int nextDom() {
        return dom++;
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
    void subscriptionMatchedNamesTheWriterThatConnectedOrLeftLast() throws Exception {
        ReaderRig rig = new ReaderRig(nextDom());
        DataReader<Keyed> reader = rig.keepAllReader();
        DataWriter<Keyed> first = rig.writer();
        SubscriptionMatchedStatus one = reader.getSubscriptionMatchedStatus();
        assertEquals(1, one.getTotalCount());
        assertEquals(1, one.getTotalCountChange());
        assertEquals(1, one.getCurrentCount());
        assertEquals(1, one.getCurrentCountChange());
        assertEquals(first.getInstanceHandle(), one.getLastPublicationHandle());
        assertFalse(one.getLastPublicationHandle().isNil());

        DataWriter<Keyed> second = rig.writer();
        SubscriptionMatchedStatus two = reader.getSubscriptionMatchedStatus();
        assertEquals(2, two.getTotalCount());
        assertEquals(1, two.getTotalCountChange());
        assertEquals(second.getInstanceHandle(), two.getLastPublicationHandle());
        // the two writers are the publications the reader is matched with
        assertEquals(2, reader.getMatchedPublications().size());
        assertTrue(reader.getMatchedPublications().contains(first.getInstanceHandle()));
        assertTrue(reader.getMatchedPublications().contains(second.getInstanceHandle()));

        first.close();
        SubscriptionMatchedStatus left = reader.getSubscriptionMatchedStatus();
        assertEquals(2, left.getTotalCount());
        assertEquals(0, left.getTotalCountChange());
        assertEquals(1, left.getCurrentCount());
        assertEquals(-1, left.getCurrentCountChange());
        assertEquals(first.getInstanceHandle(), left.getLastPublicationHandle());
        // and the writer that left is no longer matched
        assertEquals(1, reader.getMatchedPublications().size());
        assertFalse(reader.getMatchedPublications().contains(first.getInstanceHandle()));
    }

    @Test
    void aWriterInAnotherPartitionOrWithLessQosThanRequestedIsNotMatched() throws Exception {
        ReaderRig rig = new ReaderRig(nextDom());
        DataReader<Keyed> reader = rig.reader(Qos.readerKeepAll()
                .withPolicy(Qos.pf().Reliability().withReliable()));
        DataWriter<Keyed> good = rig.writer();
        Publisher inA = rig.participant.createPublisher(Qos.publisher()
                .withPolicy(Qos.policies().Partition().withName(List.of("A"))));
        Qos.writer(inA, rig.topic, ReaderRig.SUPPORT, Qos.writer());
        Qos.writer(rig.publisher, rig.topic, ReaderRig.SUPPORT, Qos.writer()
                .withPolicy(Qos.pf().Reliability().withBestEffort()));
        assertEquals(1, reader.getMatchedPublications().size());
        assertTrue(reader.getMatchedPublications().contains(good.getInstanceHandle()));
        assertEquals(1, reader.getSubscriptionMatchedStatus().getTotalCount());
    }

    @Test
    void livelinessChangedNamesTheWriterWhoseLivelinessChangedAndCountsTheChanges()
            throws Exception {
        ReaderRig rig = new ReaderRig(nextDom());
        DataReader<Keyed> reader = rig.reader(Qos.readerKeepAll().withPolicy(
                Qos.policies().Liveliness().withManualByTopic()
                        .withLeaseDuration(500, TimeUnit.MILLISECONDS)));
        DataWriter<Keyed> writer = Qos.writer(rig.publisher, rig.topic, ReaderRig.SUPPORT,
                Qos.writer().withPolicy(Qos.policies().Liveliness().withManualByTopic()
                        .withLeaseDuration(500, TimeUnit.MILLISECONDS)));
        LivelinessChangedStatus alive = reader.getLivelinessChangedStatus();
        assertEquals(1, alive.getAliveCount());
        assertEquals(0, alive.getNotAliveCount());
        assertEquals(1, alive.getAliveCountChange());
        assertEquals(0, alive.getNotAliveCountChange());
        assertEquals(writer.getInstanceHandle(), alive.getLastPublicationHandle());

        // the lease of the writer runs out without it asserting liveliness again
        LivelinessChangedStatus lost;
        long end = System.nanoTime() + TimeUnit.SECONDS.toNanos(20);
        do {
            Thread.sleep(25);
            lost = reader.getLivelinessChangedStatus();
        } while (lost.getNotAliveCount() == 0 && System.nanoTime() < end);
        assertEquals(0, lost.getAliveCount());
        assertEquals(1, lost.getNotAliveCount());
        // the change is measured from the status that was read before
        assertEquals(-1, lost.getAliveCountChange());
        assertEquals(1, lost.getNotAliveCountChange());
        assertEquals(writer.getInstanceHandle(), lost.getLastPublicationHandle());
    }

    @Test
    void anIncompatibleWriterIsNotCountedAsAliveOrNotAlive() throws Exception {
        ReaderRig rig = new ReaderRig(nextDom());
        DataReader<Keyed> reader = rig.reader(Qos.readerKeepAll()
                .withPolicy(Qos.pf().Reliability().withReliable()));
        Qos.writer(rig.publisher, rig.topic, ReaderRig.SUPPORT, Qos.writer()
                .withPolicy(Qos.pf().Reliability().withBestEffort()));
        LivelinessChangedStatus status = reader.getLivelinessChangedStatus();
        assertEquals(0, status.getAliveCount());
        assertEquals(0, status.getNotAliveCount());
    }

    @Test
    void requestedDeadlineMissedNamesTheInstanceThatMissedLast() throws Exception {
        ReaderRig rig = new ReaderRig(nextDom());
        DataReader<Keyed> reader = rig.reader(Qos.readerKeepAll().withPolicy(
                Qos.policies().Deadline().withPeriod(100, TimeUnit.MILLISECONDS)));
        DataWriter<Keyed> writer = Qos.writer(rig.publisher, rig.topic, ReaderRig.SUPPORT,
                Qos.writer().withPolicy(
                        Qos.policies().Deadline().withPeriod(100, TimeUnit.MILLISECONDS)));
        writer.write(sample(1, 0));
        writer.write(sample(2, 0));
        InstanceHandle second = reader.lookupInstance(sample(2, 0));
        InstanceHandle first = reader.lookupInstance(sample(1, 0));
        assertFalse(second.isNil());
        assertFalse(first.equals(second));

        // both instances miss their deadline again and again; the status names the one the
        // reader found last, and it does so in some evaluation within the time allowed
        RequestedDeadlineMissedStatus status;
        long end = System.nanoTime() + TimeUnit.SECONDS.toNanos(20);
        do {
            Thread.sleep(20);
            status = reader.getRequestedDeadlineMissedStatus();
        } while (!second.equals(status.getLastInstanceHandle()) && System.nanoTime() < end);
        assertEquals(second, status.getLastInstanceHandle());
        assertTrue(status.getTotalCount() >= 2);
        assertTrue(status.getTotalCountChange() >= 1);
    }

    @Test
    void theChangeOfRequestedDeadlineMissedStartsAgainWhenTheStatusIsRead() throws Exception {
        ReaderRig rig = new ReaderRig(nextDom());
        // a long period: the status read twice in a row does not see a second miss in between
        DataReader<Keyed> reader = rig.reader(Qos.readerKeepAll().withPolicy(
                Qos.policies().Deadline().withPeriod(1500, TimeUnit.MILLISECONDS)));
        DataWriter<Keyed> writer = Qos.writer(rig.publisher, rig.topic, ReaderRig.SUPPORT,
                Qos.writer().withPolicy(
                        Qos.policies().Deadline().withPeriod(1500, TimeUnit.MILLISECONDS)));
        writer.write(sample(1, 0));
        RequestedDeadlineMissedStatus first;
        long end = System.nanoTime() + TimeUnit.SECONDS.toNanos(30);
        do {
            Thread.sleep(50);
            first = reader.getRequestedDeadlineMissedStatus();
        } while (first.getTotalCount() == 0 && System.nanoTime() < end);
        assertTrue(first.getTotalCount() >= 1);
        assertTrue(first.getTotalCountChange() >= 1);
        RequestedDeadlineMissedStatus second = reader.getRequestedDeadlineMissedStatus();
        assertEquals(0, second.getTotalCountChange());
        assertEquals(first.getTotalCount(), second.getTotalCount());
        assertEquals(first.getLastInstanceHandle(), second.getLastInstanceHandle());
    }

    @Test
    void requestedIncompatibleQosCountsTheRefusingPolicyAndResetsItsChange() throws Exception {
        ReaderRig rig = new ReaderRig(nextDom());
        DataReader<Keyed> reader = rig.reader(Qos.readerKeepAll()
                .withPolicy(Qos.pf().Reliability().withReliable()));
        Qos.writer(rig.publisher, rig.topic, ReaderRig.SUPPORT, Qos.writer()
                .withPolicy(Qos.pf().Reliability().withBestEffort()));
        RequestedIncompatibleQosStatus status = reader.getRequestedIncompatibleQosStatus();
        assertEquals(1, status.getTotalCount());
        assertEquals(1, status.getTotalCountChange());
        assertEquals(Reliability.class, status.getLastPolicyClass());
        Map<Class<? extends QosPolicy>, Integer> counts = counts(status);
        assertEquals(1, counts.size());
        assertEquals(1, counts.get(Reliability.class));
        // read again: the change starts again, the totals stay
        RequestedIncompatibleQosStatus again = reader.getRequestedIncompatibleQosStatus();
        assertEquals(1, again.getTotalCount());
        assertEquals(0, again.getTotalCountChange());
    }

    @Test
    void everyPolicyThatRefusesTheConnectionIsCountedAndOneConnectionIsOneInTotal()
            throws Exception {
        ReaderRig rig = new ReaderRig(nextDom());
        DataReader<Keyed> reader = rig.reader(Qos.readerKeepAll()
                .withPolicy(Qos.pf().Reliability().withReliable())
                .withPolicy(Qos.pf().DestinationOrder().withSourceTimestamp())
                .withPolicy(Qos.pf().LatencyBudget().withDuration(10, TimeUnit.MILLISECONDS)));
        Qos.writer(rig.publisher, rig.topic, ReaderRig.SUPPORT, Qos.writer()
                .withPolicy(Qos.pf().Reliability().withBestEffort())
                .withPolicy(Qos.pf().DestinationOrder().withReceptionTimestamp())
                .withPolicy(Qos.pf().LatencyBudget().withDuration(1, TimeUnit.SECONDS)));
        RequestedIncompatibleQosStatus status = reader.getRequestedIncompatibleQosStatus();
        assertEquals(1, status.getTotalCount());
        Map<Class<? extends QosPolicy>, Integer> counts = counts(status);
        assertEquals(3, counts.size());
        assertEquals(1, counts.get(Reliability.class));
        assertEquals(1, counts.get(DestinationOrder.class));
        assertEquals(1, counts.get(LatencyBudget.class));
        assertTrue(counts.containsKey(status.getLastPolicyClass()));
        assertEquals(0, reader.getMatchedPublications().size());
    }

    @Test
    void thePresentationOfTheSubscriberMustBeOfferedByThePublisher() throws Exception {
        ReaderRig rig = new ReaderRig(nextDom());
        // the subscriber asks for access scope TOPIC with ordered access
        Subscriber asking = rig.participant.createSubscriber(Qos.subscriber().withPolicy(
                Qos.policies().Presentation().withTopic().withOrderedAccess(true)));
        DataReader<Keyed> reader = Qos.reader(asking, rig.topic, ReaderRig.SUPPORT,
                Qos.readerKeepAll());
        // a publisher with the default access scope INSTANCE offers less
        Qos.writer(rig.publisher, rig.topic, ReaderRig.SUPPORT, Qos.writer());
        // access scope TOPIC but no ordered access: not enough either
        Publisher unordered = rig.participant.createPublisher(Qos.publisher()
                .withPolicy(Qos.policies().Presentation().withTopic()));
        Qos.writer(unordered, rig.topic, ReaderRig.SUPPORT, Qos.writer());
        RequestedIncompatibleQosStatus refused = reader.getRequestedIncompatibleQosStatus();
        assertEquals(2, refused.getTotalCount());
        assertEquals(Presentation.class, refused.getLastPolicyClass());
        assertEquals(2, counts(refused).get(Presentation.class));
        assertEquals(0, reader.getMatchedPublications().size());
        // a publisher that offers at least that is matched
        Publisher offering = rig.participant.createPublisher(Qos.publisher().withPolicy(
                Qos.policies().Presentation().withGroup().withOrderedAccess(true)));
        DataWriter<Keyed> matched = Qos.writer(offering, rig.topic, ReaderRig.SUPPORT,
                Qos.writer());
        assertEquals(1, reader.getMatchedPublications().size());
        assertTrue(reader.getMatchedPublications().contains(matched.getInstanceHandle()));
        assertEquals(2, reader.getRequestedIncompatibleQosStatus().getTotalCount());
    }

    private static Map<Class<? extends QosPolicy>, Integer> counts(
            RequestedIncompatibleQosStatus status) {
        Map<Class<? extends QosPolicy>, Integer> out = new HashMap<>();
        for (QosPolicyCount count : status.getPolicies()) {
            out.put(count.getPolicyClass(), count.getCount());
        }
        return out;
    }
}
