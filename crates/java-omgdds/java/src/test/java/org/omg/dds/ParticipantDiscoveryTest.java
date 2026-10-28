// SPDX-License-Identifier: Apache-2.0
package org.omg.dds;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.omg.dds.core.PreconditionNotMetException;
import org.omg.dds.domain.DomainParticipant;
import org.omg.dds.domain.DomainParticipantQos;
import org.omg.dds.sub.DataReader;
import org.omg.dds.sub.Sample;
import org.omg.dds.sub.Subscriber;
import org.omg.dds.topic.ParticipantBuiltinTopicData;
import org.omg.dds.topic.Topic;
import org.omg.dds.topic.TopicBuiltinTopicData;
import org.zerodds.internal.InProcessBus;
import org.zerodds.topic.BuiltinTopicKeyImpl;
import org.zerodds.topic.BuiltinTopics;
import org.zerodds.topic.ParticipantBuiltinTopicDataImpl;
import org.zerodds.topic.PublicationBuiltinTopicDataImpl;
import org.zerodds.topic.SubscriptionBuiltinTopicDataImpl;
import org.zerodds.topic.TopicBuiltinTopicDataImpl;

import java.util.Arrays;
import java.util.HashSet;
import java.util.List;
import java.util.Set;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * DDS 1.4 §2.2.2.2.1.13 and §2.2.2.2.1.27 to §2.2.2.2.1.30: the built-in subscriber with its
 * DataReaders, and what a participant knows about the participants and topics of the domain,
 * those of other participants of the process included. Domain ids 1260 to 1289.
 */
class ParticipantDiscoveryTest {

    @BeforeEach
    void setUp() {
        InProcessBus.instance().reset();
    }

    @AfterEach
    void tearDown() {
        InProcessBus.instance().reset();
    }

    private static DomainParticipant withUserData(int domain, byte[] value) {
        DomainParticipantQos qos = Qos.factory().getDefaultParticipantQos().withPolicy(
                Qos.pf().UserData().withValue(value, 0, value.length));
        return Qos.factory().createParticipant(domain, qos, null, null);
    }

    // --- get_discovered_participants and get_discovered_participant_data ----------

    @Test
    void theDiscoveredParticipantsAreTheOtherParticipantsOfTheDomainAndTheirUserData() {
        int domain = 1260;
        DomainParticipant mine = Qos.factory().createParticipant(domain);
        byte[] value = {4, 5, 6};
        DomainParticipant other = withUserData(domain, value);
        DomainParticipant elsewhere = Qos.factory().createParticipant(domain + 1);

        Set<org.omg.dds.core.InstanceHandle> discovered = mine.getDiscoveredParticipants();
        assertTrue(discovered.contains(other.getInstanceHandle()));
        assertFalse(discovered.contains(elsewhere.getInstanceHandle()), "another domain");

        ParticipantBuiltinTopicData data = mine.getDiscoveredParticipantData(other.getInstanceHandle());
        assertEquals(BuiltinTopicKeyImpl.of(Qos.env(), other.getInstanceHandle()), data.getKey());
        assertArrayEquals(value, data.getUserData().getValue());
        // a participant of another domain is not a discovered participant of this one
        assertThrows(PreconditionNotMetException.class,
                () -> mine.getDiscoveredParticipantData(elsewhere.getInstanceHandle()));
    }

    @Test
    void aClosedParticipantIsNotDiscoveredAnyMore() {
        int domain = 1262;
        DomainParticipant mine = Qos.factory().createParticipant(domain);
        DomainParticipant other = Qos.factory().createParticipant(domain);
        assertTrue(mine.getDiscoveredParticipants().contains(other.getInstanceHandle()));
        org.omg.dds.core.InstanceHandle handle = other.getInstanceHandle();
        other.close();
        assertFalse(mine.getDiscoveredParticipants().contains(handle));
        assertThrows(PreconditionNotMetException.class, () -> mine.getDiscoveredParticipantData(handle));
    }

    // --- get_discovered_topics and get_discovered_topic_data ------------------------

    @Test
    void theTopicsOfOtherParticipantsOfTheDomainAreDiscovered() {
        int domain = 1263;
        DomainParticipant mine = Qos.factory().createParticipant(domain);
        DomainParticipant other = Qos.factory().createParticipant(domain);
        Topic<String> topic = other.createTopic("DiscoveredTopic", String.class);

        assertTrue(mine.getDiscoveredTopics().contains(topic.getInstanceHandle()));
        TopicBuiltinTopicData data = mine.getDiscoveredTopicData(topic.getInstanceHandle());
        assertEquals("DiscoveredTopic", data.getName());
        assertEquals(topic.getTypeName(), data.getTypeName());
        assertEquals(topic.getQos().getReliability(), data.getReliability());
        // a handle that names no topic of the domain
        assertThrows(PreconditionNotMetException.class,
                () -> mine.getDiscoveredTopicData(other.getInstanceHandle()));

        topic.close();
        assertFalse(mine.getDiscoveredTopics().contains(topic.getInstanceHandle()));
    }

    @Test
    void aTopicOfAnIgnoredParticipantIsNotDiscovered() {
        int domain = 1264;
        DomainParticipant mine = Qos.factory().createParticipant(domain);
        DomainParticipant other = Qos.factory().createParticipant(domain);
        Topic<String> topic = other.createTopic("IgnoredOwner", String.class);
        assertTrue(mine.getDiscoveredTopics().contains(topic.getInstanceHandle()));
        mine.ignoreParticipant(other.getInstanceHandle());
        assertFalse(mine.getDiscoveredTopics().contains(topic.getInstanceHandle()));
        assertThrows(PreconditionNotMetException.class,
                () -> mine.getDiscoveredTopicData(topic.getInstanceHandle()));
    }

    // --- get_builtin_subscriber ------------------------------------------------------

    @Test
    void theBuiltinSubscriberHoldsOneReaderPerBuiltinTopic() {
        DomainParticipant participant = Qos.factory().createParticipant(1265);
        Subscriber builtin = participant.getBuiltinSubscriber();
        Set<DataReader<?>> readers = new HashSet<>();
        for (String name : Arrays.asList(BuiltinTopics.PARTICIPANT, BuiltinTopics.PUBLICATION,
                BuiltinTopics.SUBSCRIPTION, BuiltinTopics.TOPIC)) {
            DataReader<?> reader = builtin.lookupDataReader(name);
            assertNotNull(reader, name);
            assertSame(reader, builtin.lookupDataReader(name));
            assertEquals(name, reader.getTopicDescription().getName());
            assertTrue(participant.containsEntity(reader.getInstanceHandle()));
            readers.add(reader);
        }
        assertEquals(4, readers.size());
        // the readers are not announced on DCPSSubscription: nothing about the builtin
        // readers shows up in a later observer
        DomainParticipant observer = Qos.factory().createParticipant(1265);
        DataReader<SubscriptionBuiltinTopicDataImpl> subscriptions =
                observer.getBuiltinSubscriber().lookupDataReader(BuiltinTopics.SUBSCRIPTION);
        for (Sample<SubscriptionBuiltinTopicDataImpl> sample : Qos.take(subscriptions)) {
            if (sample.getData() != null) {
                assertFalse(sample.getData().getTopicName().startsWith("DCPS"),
                        sample.getData().getTopicName());
            }
        }
    }

    @Test
    void aBuiltinReaderHasTheEntitiesThatExistedBeforeAndNoneOfItsOwnParticipant() {
        int domain = 1266;
        DomainParticipant early = Qos.factory().createParticipant(domain);
        byte[] value = {1, 2};
        DomainParticipant other = withUserData(domain, value);
        Topic<String> topic = other.createTopic("AlreadyThere", String.class);
        other.createPublisher().createDataWriter(topic);
        other.createSubscriber().createDataReader(topic);

        DomainParticipant observer = Qos.factory().createParticipant(domain);
        observer.createTopic("OwnTopic", String.class);
        Subscriber builtin = observer.getBuiltinSubscriber();

        DataReader<ParticipantBuiltinTopicDataImpl> participants =
                builtin.lookupDataReader(BuiltinTopics.PARTICIPANT);
        Set<org.omg.dds.topic.BuiltinTopicKey> keys = new HashSet<>();
        byte[] seenUserData = null;
        for (Sample<ParticipantBuiltinTopicDataImpl> sample : Qos.take(participants)) {
            keys.add(sample.getData().getKey());
            if (sample.getData().getKey().equals(
                    BuiltinTopicKeyImpl.of(Qos.env(), other.getInstanceHandle()))) {
                seenUserData = sample.getData().getUserData().getValue();
            }
        }
        assertTrue(keys.contains(BuiltinTopicKeyImpl.of(Qos.env(), early.getInstanceHandle())));
        assertTrue(keys.contains(BuiltinTopicKeyImpl.of(Qos.env(), other.getInstanceHandle())));
        assertFalse(keys.contains(BuiltinTopicKeyImpl.of(Qos.env(), observer.getInstanceHandle())),
                "DDS 1.4 §2.2.5: not the participant that owns the reader");
        // the participant created with non-empty USER_DATA carries it in DCPSParticipant
        assertArrayEquals(value, seenUserData);

        DataReader<PublicationBuiltinTopicDataImpl> publications =
                builtin.lookupDataReader(BuiltinTopics.PUBLICATION);
        boolean publication = false;
        for (Sample<PublicationBuiltinTopicDataImpl> sample : Qos.take(publications)) {
            publication |= sample.getData() != null
                    && "AlreadyThere".equals(sample.getData().getTopicName());
        }
        assertTrue(publication);

        DataReader<SubscriptionBuiltinTopicDataImpl> subscriptions =
                builtin.lookupDataReader(BuiltinTopics.SUBSCRIPTION);
        boolean subscription = false;
        for (Sample<SubscriptionBuiltinTopicDataImpl> sample : Qos.take(subscriptions)) {
            subscription |= sample.getData() != null
                    && "AlreadyThere".equals(sample.getData().getTopicName());
        }
        assertTrue(subscription);

        DataReader<TopicBuiltinTopicDataImpl> topics = builtin.lookupDataReader(BuiltinTopics.TOPIC);
        boolean other1 = false;
        boolean own = false;
        List<Sample<TopicBuiltinTopicDataImpl>> samples = Qos.take(topics);
        for (Sample<TopicBuiltinTopicDataImpl> sample : samples) {
            other1 |= sample.getData() != null && "AlreadyThere".equals(sample.getData().getName());
            own |= sample.getData() != null && "OwnTopic".equals(sample.getData().getName());
        }
        assertTrue(other1);
        assertFalse(own);
    }

    @Test
    void theBuiltinReaderSeesTheUserDataChangeOfAnotherParticipant() {
        int domain = 1267;
        DomainParticipant observer = Qos.factory().createParticipant(domain);
        DomainParticipant other = withUserData(domain, new byte[] {1});
        DataReader<ParticipantBuiltinTopicDataImpl> participants =
                observer.getBuiltinSubscriber().lookupDataReader(BuiltinTopics.PARTICIPANT);
        assertEquals(1, Qos.take(participants).stream()
                .filter(s -> s.getData() != null && s.getData().getKey().equals(
                        BuiltinTopicKeyImpl.of(Qos.env(), other.getInstanceHandle())))
                .count());

        byte[] changed = {9, 9};
        other.setQos(other.getQos().withPolicy(Qos.pf().UserData().withValue(changed, 0, 2)));
        byte[] seen = null;
        for (Sample<ParticipantBuiltinTopicDataImpl> sample : Qos.take(participants)) {
            if (sample.getData() != null && sample.getData().getKey().equals(
                    BuiltinTopicKeyImpl.of(Qos.env(), other.getInstanceHandle()))) {
                seen = sample.getData().getUserData().getValue();
            }
        }
        assertArrayEquals(changed, seen);
    }
}
