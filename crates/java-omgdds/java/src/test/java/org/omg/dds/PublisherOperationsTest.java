// SPDX-License-Identifier: Apache-2.0
package org.omg.dds;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.omg.dds.core.AlreadyClosedException;
import org.omg.dds.core.InconsistentPolicyException;
import org.omg.dds.core.PreconditionNotMetException;
import org.omg.dds.core.event.OfferedIncompatibleQosEvent;
import org.omg.dds.core.policy.Deadline;
import org.omg.dds.core.policy.OwnershipStrength;
import org.omg.dds.core.policy.Reliability;
import org.omg.dds.core.policy.UserData;
import org.omg.dds.domain.DomainParticipant;
import org.omg.dds.domain.DomainParticipantAdapter;
import org.omg.dds.pub.DataWriter;
import org.omg.dds.pub.DataWriterQos;
import org.omg.dds.pub.Publisher;
import org.omg.dds.pub.PublisherAdapter;
import org.omg.dds.pub.PublisherQos;
import org.omg.dds.sub.DataReader;
import org.omg.dds.sub.InstanceState;
import org.omg.dds.sub.Sample;
import org.omg.dds.topic.Topic;
import org.omg.dds.topic.TopicQos;
import org.zerodds.internal.InProcessBus;
import org.zerodds.pub.DataWriterImpl;
import org.zerodds.pub.PublisherImpl;
import org.zerodds.topic.BuiltinTopics;
import org.zerodds.topic.PublicationBuiltinTopicDataImpl;

import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.concurrent.atomic.AtomicReference;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * The Publisher operations of DDS 1.4 §2.2.2.4.1: lookup, suspend and resume, coherent changes,
 * wait_for_acknowledgments, delete_contained_entities, the default DataWriter QoS and
 * copy_from_topic_qos, and the listener of the Publisher. Domain ids 1320 to 1349.
 */
class PublisherOperationsTest {

    @BeforeEach
    void setUp() {
        InProcessBus.instance().reset();
    }

    @AfterEach
    void tearDown() {
        InProcessBus.instance().reset();
    }

    private static void write(DataWriter<byte[]> writer, int value) {
        try {
            writer.write(new byte[] {(byte) value});
        } catch (java.util.concurrent.TimeoutException e) {
            throw new AssertionError(e);
        }
    }

    // --- get_participant, lookup_datawriter ---------------------------------------------

    @Test
    void aPublisherKnowsItsParticipantAndItsWritersTheirPublisherAndTopic() {
        DomainParticipant participant = Qos.factory().createParticipant(1320);
        Publisher publisher = participant.createPublisher();
        Topic<byte[]> topic = participant.createTopic("Parents", byte[].class);
        DataWriter<byte[]> writer = publisher.createDataWriter(topic);
        assertSame(participant, publisher.getParent());
        assertSame(publisher, writer.getParent());
        assertSame(topic, writer.getTopic());
    }

    @Test
    void lookupDataWriterFindsByNameAndByTopicAndForgetsClosedWriters() {
        DomainParticipant participant = Qos.factory().createParticipant(1321);
        Publisher publisher = participant.createPublisher();
        Topic<byte[]> topic = participant.createTopic("Lookup", byte[].class);
        Topic<byte[]> missing = participant.createTopic("LookupMissing", byte[].class);
        DataWriter<byte[]> writer = publisher.createDataWriter(topic);

        assertSame(writer, publisher.<byte[]>lookupDataWriter("Lookup"));
        assertSame(writer, publisher.lookupDataWriter(topic));
        assertNull(publisher.lookupDataWriter("Nothing"));
        assertNull(publisher.lookupDataWriter(missing));

        writer.close();
        assertNull(publisher.lookupDataWriter("Lookup"));
        assertNull(publisher.lookupDataWriter(topic));
    }

    // --- suspend_publications, resume_publications ----------------------------------------

    @Test
    void resumePublicationsNeedsASuspendPublicationsAndTheCallsNest() {
        DomainParticipant participant = Qos.factory().createParticipant(1322);
        Publisher publisher = participant.createPublisher();
        assertThrows(PreconditionNotMetException.class, publisher::resumePublications);

        publisher.suspendPublications();
        publisher.resumePublications();
        assertThrows(PreconditionNotMetException.class, publisher::resumePublications);

        publisher.suspendPublications();
        publisher.suspendPublications();
        assertTrue(((PublisherImpl) publisher).isSuspended());
        publisher.resumePublications();
        assertTrue(((PublisherImpl) publisher).isSuspended());
        publisher.resumePublications();
        assertFalse(((PublisherImpl) publisher).isSuspended());
        assertThrows(PreconditionNotMetException.class, publisher::resumePublications);

        publisher.close();
        assertThrows(AlreadyClosedException.class, publisher::suspendPublications);
    }

    @Test
    void writesWhileSuspendedAreDeliveredAsBefore() {
        DomainParticipant participant = Qos.factory().createParticipant(1323);
        Publisher publisher = participant.createPublisher();
        Topic<byte[]> topic = participant.createTopic("Suspended", byte[].class);
        DataWriter<byte[]> writer = publisher.createDataWriter(topic);
        DataReader<byte[]> reader = participant.createSubscriber().createDataReader(topic);
        publisher.suspendPublications();
        write(writer, 1);
        // suspend_publications is a hint (DDS 1.4 §2.2.2.4.1.8): the Service may deliver at once
        assertEquals(1, Qos.take(reader).size());
        publisher.resumePublications();
    }

    // --- begin_coherent_changes, end_coherent_changes --------------------------------------

    @Test
    void endCoherentChangesNeedsABeginAndTheCallsNest() {
        DomainParticipant participant = Qos.factory().createParticipant(1324);
        Publisher publisher = participant.createPublisher();
        assertThrows(PreconditionNotMetException.class, publisher::endCoherentChanges);

        publisher.beginCoherentChanges();
        publisher.beginCoherentChanges();
        assertTrue(((PublisherImpl) publisher).inCoherentChanges());
        publisher.endCoherentChanges();
        assertTrue(((PublisherImpl) publisher).inCoherentChanges(),
                "the set ends with the last end_coherent_changes");
        publisher.endCoherentChanges();
        assertFalse(((PublisherImpl) publisher).inCoherentChanges());
        assertThrows(PreconditionNotMetException.class, publisher::endCoherentChanges);
    }

    @Test
    void aCoherentSetIsDeliveredTogetherWhenThePresentationAsksForCoherentAccess() {
        DomainParticipant participant = Qos.factory().createParticipant(1325);
        PublisherQos coherent = Qos.publisher().withPolicy(
                Qos.policies().Presentation().withTopic().withCoherentAccess(true));
        Publisher publisher = participant.createPublisher(coherent);
        Topic<byte[]> first = participant.createTopic("CoherentOne", byte[].class);
        Topic<byte[]> second = participant.createTopic("CoherentTwo", byte[].class);
        DataWriter<byte[]> writerOne = publisher.createDataWriter(first);
        DataWriter<byte[]> writerTwo = publisher.createDataWriter(second);
        DataReader<byte[]> readerOne = participant.createSubscriber().createDataReader(first,
                Qos.readerKeepAll());
        DataReader<byte[]> readerTwo = participant.createSubscriber().createDataReader(second,
                Qos.readerKeepAll());

        write(writerOne, 1); // outside a set: at once
        assertEquals(1, readerOne.read(new ArrayList<>()).size());

        publisher.beginCoherentChanges();
        write(writerOne, 2);
        publisher.beginCoherentChanges(); // nested
        write(writerTwo, 3);
        write(writerOne, 4);
        publisher.endCoherentChanges();
        assertEquals(1, readerOne.read(new ArrayList<>()).size(), "still inside the set");
        assertEquals(0, readerTwo.read(new ArrayList<>()).size(), "still inside the set");

        publisher.endCoherentChanges();
        List<Sample<byte[]>> one = Qos.take(readerOne);
        assertEquals(3, one.size());
        assertEquals(1, one.get(0).getData()[0]);
        assertEquals(2, one.get(1).getData()[0]);
        assertEquals(4, one.get(2).getData()[0]);
        List<Sample<byte[]>> two = Qos.take(readerTwo);
        assertEquals(1, two.size());
        assertEquals(3, two.get(0).getData()[0]);
    }

    @Test
    void withoutCoherentAccessTheSamplesOfASetAreDeliveredAtOnce() {
        DomainParticipant participant = Qos.factory().createParticipant(1326);
        Publisher publisher = participant.createPublisher();
        Topic<byte[]> topic = participant.createTopic("NotCoherent", byte[].class);
        DataWriter<byte[]> writer = publisher.createDataWriter(topic);
        DataReader<byte[]> reader = participant.createSubscriber().createDataReader(topic,
                Qos.readerKeepAll());
        publisher.beginCoherentChanges();
        write(writer, 1);
        assertEquals(1, reader.read(new ArrayList<>()).size());
        publisher.endCoherentChanges();
    }

    // --- wait_for_acknowledgments ------------------------------------------------------------

    @Test
    void waitForAcknowledgmentsReturnsAtOnceForReadersOfTheProcess() throws Exception {
        DomainParticipant participant = Qos.factory().createParticipant(1327);
        Publisher publisher = participant.createPublisher();
        Topic<byte[]> topic = participant.createTopic("Acknowledged", byte[].class);
        DataWriter<byte[]> writer = publisher.createDataWriter(topic);
        participant.createSubscriber().createDataReader(topic);
        write(writer, 1);
        // the in-process bus delivers synchronously: the sample is at the reader when write returns
        publisher.waitForAcknowledgments(5, TimeUnit.SECONDS);
        publisher.waitForAcknowledgments(Qos.env().getSPI().zeroDuration());
        publisher.waitForAcknowledgments(Qos.env().getSPI().infiniteDuration());
        assertThrows(IllegalArgumentException.class,
                () -> publisher.waitForAcknowledgments(-1, TimeUnit.SECONDS));
        publisher.close();
        assertThrows(AlreadyClosedException.class,
                () -> publisher.waitForAcknowledgments(1, TimeUnit.SECONDS));
    }

    // --- delete_contained_entities ---------------------------------------------------------------

    @Test
    void closeContainedEntitiesClosesTheWritersAndWithdrawsThemFromTheBuiltinTopic() {
        int domain = 1328;
        DomainParticipant observer = Qos.factory().createParticipant(domain);
        DomainParticipant participant = Qos.factory().createParticipant(domain);
        Publisher publisher = participant.createPublisher();
        Topic<byte[]> topic = participant.createTopic("Contained", byte[].class);
        DataWriter<byte[]> writer = publisher.createDataWriter(topic);
        DataReader<PublicationBuiltinTopicDataImpl> publications =
                observer.getBuiltinSubscriber().lookupDataReader(BuiltinTopics.PUBLICATION);
        boolean announced = false;
        for (Sample<PublicationBuiltinTopicDataImpl> sample : Qos.take(publications)) {
            announced |= sample.getData() != null && "Contained".equals(sample.getData().getTopicName());
        }
        assertTrue(announced);

        publisher.closeContainedEntities();
        assertTrue(Qos.isClosed(writer));
        assertFalse(Qos.isClosed(publisher));
        assertNull(publisher.lookupDataWriter("Contained"));
        assertTrue(((PublisherImpl) publisher).getDataWriters().isEmpty());
        assertFalse(participant.containsEntity(writer.getInstanceHandle()));
        boolean withdrawn = false;
        for (Sample<PublicationBuiltinTopicDataImpl> sample : Qos.take(publications)) {
            withdrawn |= sample.getData() == null
                    && sample.getInstanceState() == InstanceState.NOT_ALIVE_NO_WRITERS;
        }
        assertTrue(withdrawn, "the writer left DCPSPublication");
        // the Publisher can be used afterwards
        assertNotNull(publisher.createDataWriter(topic));
    }

    // --- set_default_datawriter_qos, get_default_datawriter_qos ------------------------------------

    @Test
    void theDefaultWriterQosIsTheQosOfTheWritersCreatedNext() {
        DomainParticipant participant = Qos.factory().createParticipant(1329);
        Publisher publisher = participant.createPublisher();
        Topic<byte[]> topic = participant.createTopic("DefaultWriterQos", byte[].class);
        DataWriterQos before = publisher.getDefaultDataWriterQos();
        assertNotNull(before);
        assertEquals(before, publisher.createDataWriter(topic).getQos());

        DataWriterQos mine = before.withPolicy(Qos.pf().OwnershipStrength().withValue(21));
        publisher.setDefaultDataWriterQos(mine);
        assertSame(mine, publisher.getDefaultDataWriterQos());
        assertEquals(mine, publisher.createDataWriter(topic).getQos());
        // a QoS given to the call takes precedence
        DataWriterQos given = before.withPolicy(Qos.pf().OwnershipStrength().withValue(22));
        assertEquals(given, publisher.createDataWriter(topic, given).getQos());
    }

    @Test
    void aDefaultWriterQosThatIsNotConsistentIsRefusedAndTheOldOneStays() {
        DomainParticipant participant = Qos.factory().createParticipant(1330);
        Publisher publisher = participant.createPublisher();
        DataWriterQos before = publisher.getDefaultDataWriterQos();
        DataWriterQos inconsistent = before
                .withPolicy(Qos.pf().History().withKeepLast(10))
                .withPolicy(Qos.pf().ResourceLimits().withMaxSamplesPerInstance(5));
        assertThrows(InconsistentPolicyException.class,
                () -> publisher.setDefaultDataWriterQos(inconsistent));
        assertSame(before, publisher.getDefaultDataWriterQos());
        // and no writer is created with it
        Topic<byte[]> topic = participant.createTopic("InconsistentWriterQos", byte[].class);
        assertThrows(InconsistentPolicyException.class,
                () -> publisher.createDataWriter(topic, inconsistent));
        assertNull(publisher.lookupDataWriter(topic));
    }

    // --- copy_from_topic_qos ---------------------------------------------------------------------------

    @Test
    void copyFromTopicQosLetsThePoliciesOfTheTopicReplaceThoseOfTheWriter() {
        DomainParticipant participant = Qos.factory().createParticipant(1331);
        Publisher publisher = participant.createPublisher();
        byte[] user = {1, 2};
        // a writer QoS with values that differ from the defaults, one of them a policy the topic
        // QoS does not have
        DataWriterQos writerQos = Qos.writer()
                .withPolicy(Qos.pf().Deadline().withPeriod(5, TimeUnit.SECONDS))
                .withPolicy(Qos.pf().Reliability().withBestEffort())
                .withPolicy(Qos.pf().UserData().withValue(user, 0, user.length))
                .withPolicy(Qos.pf().OwnershipStrength().withValue(5));
        TopicQos topicQos = Qos.topic()
                .withPolicy(Qos.pf().Deadline().withPeriod(7, TimeUnit.SECONDS))
                .withPolicy(Qos.pf().Reliability().withReliable());

        DataWriterQos merged = publisher.copyFromTopicQos(writerQos, topicQos);
        Deadline deadline = merged.getDeadline();
        assertEquals(7, deadline.getPeriod().getDuration(TimeUnit.SECONDS));
        Reliability reliability = merged.getReliability();
        assertEquals(Reliability.Kind.RELIABLE, reliability.getKind());
        // the policies the topic QoS does not have stay those of the writer QoS
        UserData userData = merged.getUserData();
        assertArrayEquals(user, userData.getValue());
        OwnershipStrength strength = merged.getOwnershipStrength();
        assertEquals(5, strength.getValue());

        // a topic QoS that holds the default replaces a writer value that differs from it
        DataWriterQos fromDefaults = publisher.copyFromTopicQos(writerQos, Qos.topic());
        assertTrue(fromDefaults.getDeadline().getPeriod().isInfinite());
        assertEquals(Reliability.Kind.BEST_EFFORT, fromDefaults.getReliability().getKind());
        // the QoS given is not changed
        assertEquals(5, writerQos.getDeadline().getPeriod().getDuration(TimeUnit.SECONDS));
    }

    // --- PublisherListener and DomainParticipantListener (§2.2.4.3.1) -----------------------------------

    @Test
    void aWriterWithoutListenerPassesOfferedIncompatibleQosToThePublisher() {
        DomainParticipant participant = Qos.factory().createParticipant(1332);
        Topic<byte[]> topic = participant.createTopic("PublisherIncompatible", byte[].class);
        AtomicReference<OfferedIncompatibleQosEvent<?>> seen = new AtomicReference<>();
        Publisher publisher = participant.createPublisher(Qos.publisher(), new PublisherAdapter() {
            @Override
            public void onOfferedIncompatibleQos(OfferedIncompatibleQosEvent<?> status) {
                seen.set(status);
            }
        }, null);
        DataWriter<byte[]> writer = publisher.createDataWriter(topic,
                Qos.writer().withPolicy(Qos.pf().Reliability().withBestEffort()));
        participant.createSubscriber().createDataReader(topic,
                Qos.reader().withPolicy(Qos.pf().Reliability().withReliable()));
        assertNotNull(seen.get());
        assertSame(writer, seen.get().getSource());
        assertEquals(1, seen.get().getStatus().getTotalCount());
        assertEquals(Reliability.class, seen.get().getStatus().getLastPolicyClass());
    }

    @Test
    void aWriterWithoutListenerAndAPublisherWithoutOnePassItToTheParticipantListener() {
        AtomicInteger calls = new AtomicInteger();
        DomainParticipant participant = Qos.factory().createParticipant(1333,
                Qos.factory().getDefaultParticipantQos(), new DomainParticipantAdapter() {
                    @Override
                    public void onOfferedIncompatibleQos(OfferedIncompatibleQosEvent<?> status) {
                        calls.incrementAndGet();
                    }
                }, null);
        Topic<byte[]> topic = participant.createTopic("ParticipantIncompatible", byte[].class);
        participant.createPublisher().createDataWriter(topic,
                Qos.writer().withPolicy(Qos.pf().Reliability().withBestEffort()));
        participant.createSubscriber().createDataReader(topic,
                Qos.reader().withPolicy(Qos.pf().Reliability().withReliable()));
        assertEquals(1, calls.get());
    }

    @Test
    void aWriterThatAWriterListenerTakesIsNotPassedUp() {
        AtomicInteger atPublisher = new AtomicInteger();
        AtomicInteger atWriter = new AtomicInteger();
        DomainParticipant participant = Qos.factory().createParticipant(1334);
        Topic<byte[]> topic = participant.createTopic("OwnListener", byte[].class);
        Publisher publisher = participant.createPublisher(Qos.publisher(), new PublisherAdapter() {
            @Override
            public void onOfferedIncompatibleQos(OfferedIncompatibleQosEvent<?> status) {
                atPublisher.incrementAndGet();
            }
        }, null);
        publisher.createDataWriter(topic, Qos.writer().withPolicy(Qos.pf().Reliability().withBestEffort()),
                new org.omg.dds.pub.DataWriterAdapter<byte[]>() {
                    @Override
                    public void onOfferedIncompatibleQos(OfferedIncompatibleQosEvent<byte[]> status) {
                        atWriter.incrementAndGet();
                    }
                }, null);
        participant.createSubscriber().createDataReader(topic,
                Qos.reader().withPolicy(Qos.pf().Reliability().withReliable()));
        assertEquals(1, atWriter.get());
        assertEquals(0, atPublisher.get());
    }

    @Test
    void aNewPartitionOfThePublisherMatchesItsWritersAgain() {
        DomainParticipant participant = Qos.factory().createParticipant(1336);
        Topic<byte[]> topic = participant.createTopic("PartitionChange", byte[].class);
        Publisher publisher = participant.createPublisher(Qos.publisher().withPolicy(
                Qos.pf().Partition().withName(java.util.Arrays.asList("A"))));
        org.omg.dds.sub.Subscriber subscriber = participant.createSubscriber(Qos.subscriber().withPolicy(
                Qos.pf().Partition().withName(java.util.Arrays.asList("B"))));
        DataWriter<byte[]> writer = publisher.createDataWriter(topic);
        DataReader<byte[]> reader = subscriber.createDataReader(topic);
        assertEquals(0, writer.getPublicationMatchedStatus().getCurrentCount());

        // PARTITION can change after enable (DDS 1.4 §2.2.3): the writer meets the readers of B
        publisher.setQos(publisher.getQos().withPolicy(
                Qos.pf().Partition().withName(java.util.Arrays.asList("B"))));
        assertEquals(1, writer.getPublicationMatchedStatus().getCurrentCount());
        write(writer, 1);
        assertEquals(1, Qos.take(reader).size());

        publisher.setQos(publisher.getQos().withPolicy(
                Qos.pf().Partition().withName(java.util.Arrays.asList("C"))));
        assertEquals(0, writer.getPublicationMatchedStatus().getCurrentCount());
        write(writer, 2);
        assertEquals(0, Qos.take(reader).size());
    }

    @Test
    void theWriterAndPublisherStayConsistentWhenAWriterIsClosedTwice() {
        DomainParticipant participant = Qos.factory().createParticipant(1335);
        Publisher publisher = participant.createPublisher();
        DataWriter<byte[]> writer = publisher.createDataWriter(
                participant.createTopic("ClosedTwice", byte[].class));
        writer.close();
        writer.close();
        assertTrue(((DataWriterImpl<byte[]>) writer).isClosed());
        assertTrue(((PublisherImpl) publisher).getDataWriters().isEmpty());
    }
}
