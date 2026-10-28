// SPDX-License-Identifier: Apache-2.0
package org.omg.dds;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.omg.dds.core.policy.PolicyFactory;
import org.omg.dds.core.policy.Presentation;
import org.omg.dds.domain.DomainParticipant;
import org.omg.dds.pub.DataWriter;
import org.omg.dds.pub.DataWriterQos;
import org.omg.dds.pub.Publisher;
import org.omg.dds.sub.DataReader;
import org.omg.dds.sub.DataReaderQos;
import org.omg.dds.sub.Subscriber;
import org.omg.dds.sub.SubscriberQos;
import org.omg.dds.topic.BuiltinTopicKey;
import org.omg.dds.topic.PublicationBuiltinTopicData;
import org.omg.dds.topic.SubscriptionBuiltinTopicData;
import org.omg.dds.topic.Topic;
import org.omg.dds.topic.TopicBuiltinTopicData;
import org.omg.dds.topic.TopicQos;
import org.zerodds.domain.DomainParticipantImpl;
import org.zerodds.internal.InProcessBus;
import org.zerodds.topic.BuiltinData;
import org.zerodds.topic.BuiltinTopicKeyImpl;
import org.zerodds.topic.BuiltinTopicWire;
import org.zerodds.topic.BuiltinTopics;
import org.zerodds.topic.ParticipantBuiltinTopicDataImpl;
import org.zerodds.topic.PublicationBuiltinTopicDataImpl;
import org.zerodds.topic.SubscriptionBuiltinTopicDataImpl;
import org.zerodds.topic.TopicBuiltinTopicDataImpl;

import java.util.Arrays;
import java.util.concurrent.TimeUnit;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;

/**
 * The built-in topic data with values that differ from the defaults, through the wire form and
 * back (DDS 1.4 §2.2.5, DDSI-RTPS 2.5 §9.6.2): DCPSParticipant with USER_DATA, DCPSTopic and
 * DCPSSubscription with every policy of the structure set. Domain ids 1390 to 1397.
 */
class ParticipantBuiltinDataTest {

    private static final PolicyFactory PF = Qos.pf();

    @BeforeEach
    void setUp() {
        InProcessBus.instance().reset();
    }

    @AfterEach
    void tearDown() {
        InProcessBus.instance().reset();
    }

    private static byte[] bytes(int... values) {
        byte[] out = new byte[values.length];
        for (int i = 0; i < out.length; i++) {
            out[i] = (byte) values[i];
        }
        return out;
    }

    @Test
    void aParticipantWithUserDataRoundTripsItsKeyAndUserData() {
        DomainParticipant participant = Qos.factory().createParticipant(1390,
                Qos.factory().getDefaultParticipantQos().withPolicy(
                        PF.UserData().withValue(bytes(10, 20, 30, 40, 50), 0, 5)), null, null);
        ParticipantBuiltinTopicDataImpl data = new ParticipantBuiltinTopicDataImpl(Qos.env(),
                BuiltinTopicKeyImpl.of(Qos.env(), participant.getInstanceHandle()),
                participant.getQos().getUserData());
        byte[] wire = BuiltinTopicWire.encode(data);
        ParticipantBuiltinTopicDataImpl back = (ParticipantBuiltinTopicDataImpl)
                BuiltinTopicWire.decode(Qos.env(), BuiltinTopics.PARTICIPANT, wire, 0, wire.length);
        assertEquals(data.getKey(), back.getKey());
        assertArrayEquals(bytes(10, 20, 30, 40, 50), back.getUserData().getValue());
    }

    @Test
    void aTopicWithEveryPolicyOffItsDefaultRoundTripsTheKeyAndThePolicies() {
        DomainParticipant participant = Qos.factory().createParticipant(1391);
        TopicQos topicQos = Qos.topic()
                .withPolicy(PF.LatencyBudget().withDuration(2, TimeUnit.SECONDS))
                .withPolicy(PF.Liveliness().withManualByParticipant()
                        .withLeaseDuration(6, TimeUnit.SECONDS))
                .withPolicy(PF.Lifespan().withDuration(20, TimeUnit.SECONDS))
                .withPolicy(PF.DestinationOrder().withSourceTimestamp())
                .withPolicy(PF.Ownership().withExclusive())
                .withPolicy(PF.Reliability().withReliable()
                        .withMaxBlockingTime(3, TimeUnit.SECONDS));
        Topic<String> topic = ((DomainParticipantImpl) participant)
                .createTopic("AllPolicies", String.class, topicQos);
        TopicBuiltinTopicDataImpl data = BuiltinData.topic(Qos.env(), topic);
        byte[] wire = BuiltinTopicWire.encode(data);
        TopicBuiltinTopicData back = (TopicBuiltinTopicData) BuiltinTopicWire.decode(
                Qos.env(), BuiltinTopics.TOPIC, wire, 0, wire.length);

        assertEquals(BuiltinTopicKeyImpl.of(Qos.env(), topic.getInstanceHandle()), back.getKey());
        assertEquals("AllPolicies", back.getName());
        assertEquals(topic.getTypeName(), back.getTypeName());
        assertEquals(2, back.getLatencyBudget().getDuration().getDuration(TimeUnit.SECONDS));
        assertEquals(org.omg.dds.core.policy.Liveliness.Kind.MANUAL_BY_PARTICIPANT,
                back.getLiveliness().getKind());
        assertEquals(6, back.getLiveliness().getLeaseDuration().getDuration(TimeUnit.SECONDS));
        assertEquals(20, back.getLifespan().getDuration().getDuration(TimeUnit.SECONDS));
        assertEquals(org.omg.dds.core.policy.DestinationOrder.Kind.BY_SOURCE_TIMESTAMP,
                back.getDestinationOrder().getKind());
        assertEquals(org.omg.dds.core.policy.Ownership.Kind.EXCLUSIVE, back.getOwnership().getKind());
        assertEquals(org.omg.dds.core.policy.Reliability.Kind.RELIABLE, back.getReliability().getKind());
        assertEquals(3, back.getReliability().getMaxBlockingTime().getDuration(TimeUnit.SECONDS));
        assertEquals(data.getLatencyBudget(), back.getLatencyBudget());
        assertEquals(data.getLiveliness(), back.getLiveliness());
        assertEquals(data.getLifespan(), back.getLifespan());
        assertEquals(data.getDestinationOrder(), back.getDestinationOrder());
        assertEquals(data.getOwnership(), back.getOwnership());
        assertEquals(data.getReliability(), back.getReliability());
    }

    @Test
    void aSubscriptionWithEveryPolicyOffItsDefaultRoundTripsTheKeysTheTypeAndThePolicies() {
        DomainParticipant participant = Qos.factory().createParticipant(1392);
        Topic<String> topic = participant.createTopic("AllReaderPolicies", String.class);
        SubscriberQos subscriberQos = Qos.subscriber()
                .withPolicy(PF.GroupData().withValue(bytes(1, 1, 2, 3, 5), 0, 5))
                .withPolicy(Qos.policies().Presentation().withTopic().withCoherentAccess(true)
                        .withOrderedAccess(true))
                .withPolicy(PF.Partition().withName(Arrays.asList("P", "Q")));
        Subscriber subscriber = participant.createSubscriber(subscriberQos);
        DataReaderQos readerQos = Qos.reader()
                .withPolicy(PF.Deadline().withPeriod(3, TimeUnit.SECONDS))
                .withPolicy(PF.LatencyBudget().withDuration(1, TimeUnit.SECONDS))
                .withPolicy(PF.Ownership().withExclusive())
                .withPolicy(PF.DestinationOrder().withSourceTimestamp())
                .withPolicy(PF.Reliability().withReliable())
                .withPolicy(PF.Durability().withTransientLocal());
        DataReader<String> reader = subscriber.createDataReader(topic, readerQos);
        BuiltinTopicKey participantKey =
                BuiltinTopicKeyImpl.of(Qos.env(), participant.getInstanceHandle());

        SubscriptionBuiltinTopicDataImpl data = BuiltinData.subscription(
                Qos.env(), reader, subscriber, participantKey);
        byte[] wire = BuiltinTopicWire.encode(data);
        SubscriptionBuiltinTopicData back = (SubscriptionBuiltinTopicData) BuiltinTopicWire.decode(
                Qos.env(), BuiltinTopics.SUBSCRIPTION, wire, 0, wire.length);

        assertEquals(BuiltinTopicKeyImpl.of(Qos.env(), reader.getInstanceHandle()), back.getKey());
        assertEquals(participantKey, back.getParticipantKey());
        assertEquals("AllReaderPolicies", back.getTopicName());
        assertEquals(topic.getTypeName(), back.getTypeName());
        assertEquals(3, back.getDeadline().getPeriod().getDuration(TimeUnit.SECONDS));
        assertEquals(1, back.getLatencyBudget().getDuration().getDuration(TimeUnit.SECONDS));
        assertEquals(org.omg.dds.core.policy.Ownership.Kind.EXCLUSIVE, back.getOwnership().getKind());
        assertEquals(org.omg.dds.core.policy.DestinationOrder.Kind.BY_SOURCE_TIMESTAMP,
                back.getDestinationOrder().getKind());
        assertEquals(Presentation.AccessScopeKind.TOPIC, back.getPresentation().getAccessScope());
        assertEquals(true, back.getPresentation().isCoherentAccess());
        assertEquals(true, back.getPresentation().isOrderedAccess());
        assertArrayEquals(bytes(1, 1, 2, 3, 5), back.getGroupData().getValue());
        assertEquals(data.getDeadline(), back.getDeadline());
        assertEquals(data.getLatencyBudget(), back.getLatencyBudget());
        assertEquals(data.getOwnership(), back.getOwnership());
        assertEquals(data.getDestinationOrder(), back.getDestinationOrder());
        assertEquals(data.getPresentation(), back.getPresentation());
        assertEquals(data.getGroupData(), back.getGroupData());
        assertEquals(data.getPartition(), back.getPartition());
        assertEquals(data.getDurability(), back.getDurability());
        assertEquals(data.getReliability(), back.getReliability());
    }

    @Test
    void aPublicationKeepsItsOwnDurabilityServiceAndTheTopicDataOfItsTopic() {
        DomainParticipant participant = Qos.factory().createParticipant(1393);
        byte[] topicData = bytes(7, 7, 7);
        TopicQos topicQos = Qos.topic().withPolicy(
                ((org.zerodds.core.policy.PolicyFactoryImpl) PF).TopicData()
                        .withValue(topicData, 0, 3));
        Topic<String> topic = ((DomainParticipantImpl) participant)
                .createTopic("PublicationData", String.class, topicQos);
        Publisher publisher = participant.createPublisher();
        DataWriterQos writerQos = Qos.writer()
                .withPolicy(PF.Durability().withTransient())
                .withPolicy(PF.DurabilityService().withHistoryDepth(9).withMaxSamples(80)
                        .withServiceCleanupDelay(4, TimeUnit.SECONDS));
        DataWriter<String> writer = publisher.createDataWriter(topic, writerQos);
        BuiltinTopicKey participantKey =
                BuiltinTopicKeyImpl.of(Qos.env(), participant.getInstanceHandle());

        PublicationBuiltinTopicDataImpl data = BuiltinData.publication(
                Qos.env(), writer, publisher, participantKey);
        byte[] wire = BuiltinTopicWire.encode(data);
        PublicationBuiltinTopicData back = (PublicationBuiltinTopicData) BuiltinTopicWire.decode(
                Qos.env(), BuiltinTopics.PUBLICATION, wire, 0, wire.length);
        assertEquals(data.getDurabilityService(), back.getDurabilityService());
        assertEquals(9, back.getDurabilityService().getHistoryDepth());
        assertEquals(80, back.getDurabilityService().getMaxSamples());
        assertArrayEquals(topicData, back.getTopicData().getValue());
        assertEquals(participantKey, back.getParticipantKey());
        assertEquals(org.omg.dds.core.policy.Durability.Kind.TRANSIENT, back.getDurability().getKind());
    }
}
