// SPDX-License-Identifier: Apache-2.0
package org.omg.dds;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.omg.dds.core.PreconditionNotMetException;
import org.omg.dds.core.event.DataAvailableEvent;
import org.omg.dds.core.event.DataOnReadersEvent;
import org.omg.dds.core.policy.History;
import org.omg.dds.core.status.DataOnReadersStatus;
import org.omg.dds.domain.DomainParticipant;
import org.omg.dds.pub.DataWriter;
import org.omg.dds.pub.Publisher;
import org.omg.dds.sub.DataReader;
import org.omg.dds.sub.DataReaderAdapter;
import org.omg.dds.sub.DataReaderQos;
import org.omg.dds.sub.Sample;
import org.omg.dds.sub.Subscriber;
import org.omg.dds.sub.SubscriberAdapter;
import org.omg.dds.topic.ContentFilteredTopic;
import org.omg.dds.topic.Topic;
import org.omg.dds.topic.TopicQos;
import org.zerodds.core.Values;
import org.zerodds.internal.InProcessBus;
import org.zerodds.topic.BuiltinTopics;
import org.zerodds.topic.SubscriptionBuiltinTopicDataImpl;

import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.concurrent.atomic.AtomicReference;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;
import static org.omg.dds.ReaderRig.Keyed;
import static org.omg.dds.ReaderRig.sample;

/**
 * DDS 1.4 §2.2.2.5.2 (Subscriber) and §2.2.2.5.3.27, §2.2.2.5.3.28 (DataReader): parent, lookup,
 * default reader QoS, copy_from_topic_qos, delete_contained_entities, notify_datareaders with
 * DATA_ON_READERS, begin_access and end_access, and the order of the samples that
 * PRESENTATION ordered_access and DESTINATION_ORDER give.
 */
class ReaderSubscriberTest {

    private static int dom = 1080;

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
    void theParentOfASubscriberIsItsParticipantAndTheParentOfAReaderItsSubscriber() {
        ReaderRig rig = new ReaderRig(nextDom());
        DataReader<Keyed> reader = rig.keepAllReader();
        assertSame(rig.participant, rig.subscriber.getParent());
        assertSame(rig.subscriber, reader.getParent());
    }

    @Test
    void theDescriptionOfAReaderIsTheTopicOrTheContentFilteredTopicItWasCreatedWith() {
        ReaderRig rig = new ReaderRig(nextDom());
        DataReader<Keyed> plain = rig.keepAllReader();
        assertSame(rig.topic, plain.getTopicDescription());
        ContentFilteredTopic<Keyed> filtered = rig.participant.createContentFilteredTopic(
                "ReaderFiltered", rig.topic, "seq >= 0", List.of());
        DataReader<Keyed> viaFilter = Qos.reader(rig.subscriber, filtered, ReaderRig.SUPPORT,
                Qos.readerKeepAll());
        assertSame(filtered, viaFilter.getTopicDescription());
    }

    @Test
    void lookupDataReaderFindsTheReaderByTopicNameAndByTopicDescription() {
        ReaderRig rig = new ReaderRig(nextDom());
        DataReader<Keyed> reader = rig.keepAllReader();
        DataReader<Keyed> byName = rig.subscriber.lookupDataReader(rig.topic.getName());
        assertSame(reader, byName);
        DataReader<Keyed> byDescription = rig.subscriber.lookupDataReader(rig.topic);
        assertSame(reader, byDescription);
        Topic<byte[]> other = rig.participant.createTopic("ReaderOther", byte[].class);
        assertNull(rig.subscriber.lookupDataReader("ReaderUnknown"));
        assertNull(rig.subscriber.lookupDataReader(other));
        reader.close();
        assertNull(rig.subscriber.lookupDataReader(rig.topic.getName()));
    }

    @Test
    void theDefaultReaderQosIsWhatCreateDataReaderUses() {
        DomainParticipant participant = Qos.factory().createParticipant(nextDom());
        Subscriber subscriber = participant.createSubscriber();
        Topic<byte[]> topic = participant.createTopic("ReaderDefaultQos", byte[].class);
        // DDS 1.4 §2.2.3.18: the default HISTORY is KEEP_LAST with depth 1
        History initial = subscriber.getDefaultDataReaderQos().getHistory();
        assertEquals(History.Kind.KEEP_LAST, initial.getKind());
        assertEquals(1, initial.getDepth());
        assertEquals(History.Kind.KEEP_LAST,
                subscriber.createDataReader(topic).getQos().getHistory().getKind());

        DataReaderQos keepAll = Qos.reader().withPolicy(Qos.pf().History().withKeepAll());
        subscriber.setDefaultDataReaderQos(keepAll);
        assertEquals(History.Kind.KEEP_ALL,
                subscriber.getDefaultDataReaderQos().getHistory().getKind());
        DataReader<byte[]> reader = subscriber.createDataReader(topic);
        assertEquals(History.Kind.KEEP_ALL, reader.getQos().getHistory().getKind());
    }

    @Test
    void copyFromTopicQosReplacesThePoliciesTheTopicAndTheReaderShare() {
        ReaderRig rig = new ReaderRig(nextDom());
        // the reader QoS has values other than the defaults for HISTORY and DURABILITY
        DataReaderQos reader = Qos.reader()
                .withPolicy(Qos.pf().History().withKeepLast(5))
                .withPolicy(Qos.pf().Durability().withTransientLocal())
                .withPolicy(Qos.pf().UserData().withValue(new byte[] {1, 2}, 0, 2));
        TopicQos topic = Qos.topic()
                .withPolicy(Qos.pf().History().withKeepAll())
                .withPolicy(Qos.pf().Durability().withVolatile());
        DataReaderQos merged = rig.subscriber.copyFromTopicQos(reader, topic);
        // the topic values replace those of the reader, whatever they were
        assertEquals(History.Kind.KEEP_ALL, merged.getHistory().getKind());
        assertEquals(org.omg.dds.core.policy.Durability.Kind.VOLATILE,
                merged.getDurability().getKind());
        // a policy only the reader has stays
        assertEquals(2, merged.getUserData().getValue().length);
        // the QoS passed in is not changed
        assertEquals(History.Kind.KEEP_LAST, reader.getHistory().getKind());
        assertEquals(5, reader.getHistory().getDepth());
    }

    @Test
    void closeContainedEntitiesClosesTheReadersAndWithdrawsThemFromTheBuiltinTopic()
            throws Exception {
        int domain = nextDom();
        DomainParticipant observer = Qos.factory().createParticipant(domain);
        DataReader<SubscriptionBuiltinTopicDataImpl> subscriptions =
                observer.getBuiltinSubscriber().createDataReader(
                        ReaderSubscriberTest.<SubscriptionBuiltinTopicDataImpl>builtin(
                                observer, BuiltinTopics.SUBSCRIPTION));

        ReaderRig rig = new ReaderRig(domain);
        DataReader<Keyed> first = rig.keepAllReader();
        DataReader<Keyed> second = rig.keepAllReader();
        assertEquals(2, rig.subscriber.getDataReaders(new ArrayList<>()).size());
        // what the observer announced so far is of no interest
        Qos.take(subscriptions);

        rig.subscriber.closeContainedEntities();
        assertTrue(Qos.isClosed(first));
        assertTrue(Qos.isClosed(second));
        assertTrue(rig.subscriber.getDataReaders(new ArrayList<>()).isEmpty());
        assertNull(rig.subscriber.lookupDataReader(rig.topic.getName()));
        // the subscriber itself is still open, and can create readers again
        assertFalse(Qos.isClosed(rig.subscriber));
        DataReader<Keyed> third = rig.keepAllReader();
        assertEquals(1, rig.subscriber.getDataReaders(new ArrayList<>()).size());
        assertFalse(Qos.isClosed(third));
        // DCPSSubscription: each reader that was closed is unregistered (no data, no writers)
        int withdrawn = 0;
        for (Sample<SubscriptionBuiltinTopicDataImpl> s : Qos.take(subscriptions)) {
            if (s.getData() == null && s.getInstanceState()
                    == org.omg.dds.sub.InstanceState.NOT_ALIVE_NO_WRITERS) {
                withdrawn++;
            }
        }
        assertEquals(2, withdrawn);
    }

    @Test
    void dataOnReadersGoesToTheSubscriberListenerAndNotifyDataReadersPassesItToTheReadersWithNewData()
            throws Exception {
        ReaderRig rig = new ReaderRig(nextDom());
        DataReader<Keyed> a = rig.keepAllReader();
        DataReader<Keyed> b = rig.keepAllReader();
        AtomicInteger availableA = new AtomicInteger();
        AtomicInteger availableB = new AtomicInteger();
        AtomicInteger onReaders = new AtomicInteger();
        AtomicReference<Object> source = new AtomicReference<>();
        a.setListener(new DataReaderAdapter<Keyed>() {
            @Override
            public void onDataAvailable(DataAvailableEvent<Keyed> event) {
                availableA.incrementAndGet();
            }
        });
        b.setListener(new DataReaderAdapter<Keyed>() {
            @Override
            public void onDataAvailable(DataAvailableEvent<Keyed> event) {
                availableB.incrementAndGet();
            }
        });
        rig.subscriber.setListener(new SubscriberAdapter() {
            @Override
            public void onDataOnReaders(DataOnReadersEvent event) {
                onReaders.incrementAndGet();
                source.set(event.getSource());
            }
        });

        rig.writer().write(sample(1, 0));
        // DATA_ON_READERS takes the notification: the readers' own listeners are not called
        assertTrue(onReaders.get() >= 1);
        assertSame(rig.subscriber, source.get());
        assertEquals(0, availableA.get());
        assertEquals(0, availableB.get());
        // the listener was told, so the status is no longer changed
        assertFalse(rig.subscriber.getStatusChanges().contains(DataOnReadersStatus.class));

        // b takes its sample: only a still has a changed DATA_AVAILABLE status
        assertEquals(1, Qos.take(b).size());
        rig.subscriber.notifyDataReaders();
        assertEquals(1, availableA.get());
        assertEquals(0, availableB.get());
    }

    @Test
    void withoutASubscriberListenerTheReadersListenersAreCalledAndDataOnReadersStaysChanged()
            throws Exception {
        ReaderRig rig = new ReaderRig(nextDom());
        DataReader<Keyed> reader = rig.keepAllReader();
        AtomicInteger available = new AtomicInteger();
        reader.setListener(new DataReaderAdapter<Keyed>() {
            @Override
            public void onDataAvailable(DataAvailableEvent<Keyed> event) {
                available.incrementAndGet();
            }
        });
        assertFalse(rig.subscriber.getStatusChanges().contains(DataOnReadersStatus.class));
        rig.writer().write(sample(1, 0));
        assertEquals(1, available.get());
        assertTrue(rig.subscriber.getStatusChanges().contains(DataOnReadersStatus.class));
        // reading data of a reader of the subscriber ends it
        Qos.read(reader);
        assertFalse(rig.subscriber.getStatusChanges().contains(DataOnReadersStatus.class));
        // notify_datareaders reaches no reader whose DATA_AVAILABLE is not changed
        rig.subscriber.notifyDataReaders();
        assertEquals(1, available.get());
    }

    @Test
    void withAccessScopeGroupTheSamplesAreAccessedBetweenBeginAccessAndEndAccess()
            throws Exception {
        ReaderRig rig = new ReaderRig(nextDom());
        Subscriber group = rig.participant.createSubscriber(
                Qos.subscriber().withPolicy(Qos.policies().Presentation().withGroup()));
        DataReader<Keyed> reader = Qos.reader(group, rig.topic, ReaderRig.SUPPORT,
                Qos.readerKeepAll());
        // PRESENTATION is requested by the subscriber and has to be offered by the publisher
        Publisher groupPublisher = rig.participant.createPublisher(
                Qos.publisher().withPolicy(Qos.policies().Presentation().withGroup()));
        Qos.writer(groupPublisher, rig.topic, ReaderRig.SUPPORT, Qos.writer())
                .write(sample(1, 0));

        assertThrows(PreconditionNotMetException.class, () -> Qos.read(reader));
        assertThrows(PreconditionNotMetException.class, () -> Qos.take(reader));
        assertThrows(PreconditionNotMetException.class,
                () -> group.getDataReaders(new ArrayList<>()));
        group.beginAccess();
        group.beginAccess(); // may be nested
        assertEquals(1, Qos.read(reader).size());
        assertEquals(1, group.getDataReaders(new ArrayList<>()).size());
        group.endAccess();
        assertEquals(1, Qos.take(reader).size()); // the outer block is still open
        group.endAccess();
        assertThrows(PreconditionNotMetException.class, () -> Qos.read(reader));
        // end_access without a begin_access is PRECONDITION_NOT_MET
        assertThrows(PreconditionNotMetException.class, group::endAccess);
    }

    @Test
    void withAnotherAccessScopeBeginAccessIsNotRequired() throws Exception {
        ReaderRig rig = new ReaderRig(nextDom());
        DataReader<Keyed> reader = rig.keepAllReader();
        rig.writer().write(sample(1, 0));
        assertEquals(1, Qos.read(reader).size());
        rig.subscriber.beginAccess();
        assertEquals(1, Qos.read(reader).size());
        rig.subscriber.endAccess();
        assertThrows(PreconditionNotMetException.class, rig.subscriber::endAccess);
    }

    @Test
    void destinationOrderBySourceTimestampKeepsTheSamplesOfAnInstanceInTimestampOrder()
            throws Exception {
        ReaderRig rig = new ReaderRig(nextDom());
        DataReader<Keyed> reader = rig.reader(Qos.readerKeepAll()
                .withPolicy(Qos.pf().DestinationOrder().withSourceTimestamp()));
        DataWriter<Keyed> writer = Qos.writer(rig.publisher, rig.topic, ReaderRig.SUPPORT,
                Qos.writer().withPolicy(Qos.pf().DestinationOrder().withSourceTimestamp()));
        writer.write(sample(1, 0), Values.ofMillis(3000L));
        writer.write(sample(2, 0), Values.ofMillis(1000L));
        writer.write(sample(1, 1), Values.ofMillis(2000L));
        writer.write(sample(2, 1), Values.ofMillis(4000L));
        // instance by instance, each instance in the order of the source timestamps
        assertEquals(List.of("1/1", "1/0", "2/0", "2/1"), order(Qos.read(reader)));
    }

    @Test
    void orderedAccessWithScopeTopicReturnsTheSamplesOfAllInstancesInOneOrder() throws Exception {
        ReaderRig rig = new ReaderRig(nextDom());
        Subscriber ordered = rig.participant.createSubscriber(Qos.subscriber().withPolicy(
                Qos.policies().Presentation().withTopic().withOrderedAccess(true)));
        DataReader<Keyed> reader = Qos.reader(ordered, rig.topic, ReaderRig.SUPPORT,
                Qos.readerKeepAll().withPolicy(Qos.pf().DestinationOrder().withSourceTimestamp()));
        Publisher orderedPublisher = rig.participant.createPublisher(Qos.publisher().withPolicy(
                Qos.policies().Presentation().withTopic().withOrderedAccess(true)));
        DataWriter<Keyed> writer = Qos.writer(orderedPublisher, rig.topic, ReaderRig.SUPPORT,
                Qos.writer().withPolicy(Qos.pf().DestinationOrder().withSourceTimestamp()));
        writer.write(sample(1, 0), Values.ofMillis(3000L));
        writer.write(sample(2, 0), Values.ofMillis(1000L));
        writer.write(sample(1, 1), Values.ofMillis(2000L));
        writer.write(sample(2, 1), Values.ofMillis(4000L));
        List<Sample<Keyed>> got = Qos.read(reader);
        assertEquals(List.of("2/0", "1/1", "1/0", "2/1"), order(got));
        // sample_rank still counts the samples of the same instance that follow in the collection
        assertEquals(1, got.get(0).getSampleRank());
        assertEquals(1, got.get(1).getSampleRank());
        assertEquals(0, got.get(2).getSampleRank());
        assertEquals(0, got.get(3).getSampleRank());
        // maximum samples cuts the one order
        List<Sample<Keyed>> two = reader.select().maxSamples(2).read(new ArrayList<>());
        assertEquals(List.of("2/0", "1/1"), order(two));
    }

    @SuppressWarnings("unchecked")
    private static <T> Topic<T> builtin(DomainParticipant participant, String name) {
        return (Topic<T>) participant.lookupTopicDescription(name);
    }

    private static List<String> order(List<Sample<Keyed>> samples) {
        List<String> out = new ArrayList<>();
        for (Sample<Keyed> s : samples) {
            out.add(s.getData().id() + "/" + s.getData().seq());
        }
        return out;
    }
}
