// SPDX-License-Identifier: Apache-2.0
package org.omg.dds;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.omg.dds.domain.DomainParticipant;
import org.omg.dds.pub.Publisher;
import org.omg.dds.sub.DataReader;
import org.omg.dds.sub.Sample;
import org.omg.dds.topic.ParticipantBuiltinTopicData;
import org.omg.dds.topic.PublicationBuiltinTopicData;
import org.omg.dds.topic.SubscriptionBuiltinTopicData;
import org.omg.dds.topic.Topic;
import org.omg.dds.topic.TopicBuiltinTopicData;
import org.omg.dds.topic.TopicDescription;
import org.zerodds.internal.InProcessBus;
import org.zerodds.topic.BuiltinTopics;
import org.zerodds.topic.ParticipantBuiltinTopicDataImpl;
import org.zerodds.topic.PublicationBuiltinTopicDataImpl;
import org.zerodds.topic.SubscriptionBuiltinTopicDataImpl;
import org.zerodds.topic.TopicBuiltinTopicDataImpl;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * DDS 1.2 §2.2.1.3.1 / DDSI-RTPS 2.5 §9.3.1.2: the built-in topics a
 * participant creates for itself, what they carry, and that
 * {@code create_topic} with their name returns the built-in topic.
 */
class BuiltinTopicsTest {

    private static int dom = 80;

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

    @SuppressWarnings("unchecked")
    private <T> Topic<T> builtin(DomainParticipant participant, String name,
            Class<T> dataType) {
        return (Topic<T>) participant.lookupTopicDescription(name);
    }

    @Test
    void theParticipantCreatesTheFourBuiltinTopics() {
        DomainParticipant participant = Qos.factory().createParticipant(nextDom());
        for (String name : new String[] {BuiltinTopics.PARTICIPANT,
                BuiltinTopics.PUBLICATION, BuiltinTopics.SUBSCRIPTION,
                BuiltinTopics.TOPIC}) {
            TopicDescription<?> topic = participant.lookupTopicDescription(name);
            assertNotNull(topic, name + " exists");
            assertEquals(BuiltinTopics.typeNameOf(name), topic.getTypeName(),
                    name + " carries its DDS type name");
        }
    }

    @Test
    void createTopicWithTheBuiltinNameReturnsTheBuiltinTopic() {
        DomainParticipant participant = Qos.factory().createParticipant(nextDom());
        Topic<ParticipantBuiltinTopicDataImpl> created =
                participant.createTopic(BuiltinTopics.PARTICIPANT,
                        ParticipantBuiltinTopicDataImpl.class);
        assertSame(builtin(participant, BuiltinTopics.PARTICIPANT,
                ParticipantBuiltinTopicDataImpl.class), created);
        // A built-in name is taken by the built-in topic of that type.
        assertThrows(IllegalArgumentException.class,
                () -> participant.createTopic(BuiltinTopics.PARTICIPANT,
                        byte[].class));
    }

    @Test
    void theBuiltinReaderOfAParticipantSeesTheEntitiesOfTheDomain() {
        int domain = nextDom();
        DomainParticipant observer = Qos.factory().createParticipant(domain);
        // The readers are created first: the built-in data is VOLATILE, so a
        // reader that joins later sees only what is announced after it.
        DataReader<ParticipantBuiltinTopicDataImpl> participants =
                observer.getBuiltinSubscriber().createDataReader(
                        builtin(observer, BuiltinTopics.PARTICIPANT,
                                ParticipantBuiltinTopicDataImpl.class));
        DataReader<PublicationBuiltinTopicDataImpl> publications =
                observer.getBuiltinSubscriber().createDataReader(
                        builtin(observer, BuiltinTopics.PUBLICATION,
                                PublicationBuiltinTopicDataImpl.class));
        DataReader<SubscriptionBuiltinTopicDataImpl> subscriptions =
                observer.getBuiltinSubscriber().createDataReader(
                        builtin(observer, BuiltinTopics.SUBSCRIPTION,
                                SubscriptionBuiltinTopicDataImpl.class));
        DataReader<TopicBuiltinTopicDataImpl> topics =
                observer.getBuiltinSubscriber().createDataReader(
                        builtin(observer, BuiltinTopics.TOPIC,
                                TopicBuiltinTopicDataImpl.class));

        DomainParticipant other = Qos.factory().createParticipant(domain);
        Topic<String> topic = other.createTopic("Chatter", String.class);
        other.createPublisher().createDataWriter(topic);
        other.createSubscriber().createDataReader(topic);

        // A built-in reader is matched with its own participant's built-in
        // writer, so what it sees is every announcement made after it was
        // created — including the ones of the observer itself.
        Sample<ParticipantBuiltinTopicDataImpl> participantSample =
                find(Qos.take(participants), data -> data != null
                        && data.getKey() != null
                        && data.getKey().equals(
                                org.zerodds.topic.BuiltinTopicKeyImpl.of(
                                        Qos.env(), other.getInstanceHandle())));
        assertNotNull(participantSample, "the other participant is discovered");
        ParticipantBuiltinTopicData participantData =
                participantSample.getData();
        assertNotNull(participantData.getUserData());

        Sample<PublicationBuiltinTopicDataImpl> publicationSample =
                find(Qos.take(publications), data -> data != null
                        && "Chatter".equals(data.getTopicName()));
        assertNotNull(publicationSample, "the other publication is discovered");
        PublicationBuiltinTopicData publication = publicationSample.getData();
        assertEquals(topic.getTypeName(), publication.getTypeName());
        assertNotNull(publication.getParticipantKey());
        assertNotNull(publication.getReliability());

        assertNotNull(find(Qos.take(subscriptions), data -> data != null
                && "Chatter".equals(data.getTopicName())),
                "the other subscription is discovered");

        Sample<TopicBuiltinTopicDataImpl> topicSample =
                find(Qos.take(topics), data -> data != null
                        && "Chatter".equals(data.getName()));
        assertNotNull(topicSample, "the other topic is discovered");
        TopicBuiltinTopicData topicData = topicSample.getData();
        assertEquals(topic.getTypeName(), topicData.getTypeName());
        assertNotNull(topicData.getReliability());
    }

    /** The first sample of {@code samples} whose data matches, if any. */
    private static <T> Sample<T> find(List<Sample<T>> samples,
            java.util.function.Predicate<T> matches) {
        for (Sample<T> sample : samples) {
            if (matches.test(sample.getData())) {
                return sample;
            }
        }
        return null;
    }

    @Test
    void closingAPublisherWithdrawsItsWriterFromTheBuiltinTopic() {
        int domain = nextDom();
        DomainParticipant observer = Qos.factory().createParticipant(domain);
        DataReader<PublicationBuiltinTopicDataImpl> publications =
                observer.getBuiltinSubscriber().createDataReader(
                        builtin(observer, BuiltinTopics.PUBLICATION,
                                PublicationBuiltinTopicDataImpl.class));

        DomainParticipant other = Qos.factory().createParticipant(domain);
        Topic<String> topic = other.createTopic("Chatter", String.class);
        Publisher publisher = other.createPublisher();
        publisher.createDataWriter(topic);
        assertNotNull(find(Qos.take(publications), data -> data != null
                && "Chatter".equals(data.getTopicName())), "announced");

        publisher.close();
        // The unregister leaves the instance NOT_ALIVE_NO_WRITERS
        // (DDS 1.2 §2.2.2.4.2.7) with no data.
        Sample<PublicationBuiltinTopicDataImpl> after = find(
                Qos.take(publications), data -> data == null);
        assertNotNull(after, "the withdrawn publication has no data");
        assertEquals(org.omg.dds.sub.InstanceState.NOT_ALIVE_NO_WRITERS,
                after.getInstanceState());
    }

    @Test
    void getDiscoveredParticipantDataCarriesTheUserData() {
        int domain = nextDom();
        DomainParticipant mine = Qos.factory().createParticipant(domain);
        DomainParticipant other = Qos.factory().createParticipant(domain);

        ParticipantBuiltinTopicData data =
                mine.getDiscoveredParticipantData(other.getInstanceHandle());
        assertNotNull(data.getKey());
        assertEquals(0, data.getUserData().getValue().length);
        assertThrows(org.zerodds.core.PreconditionNotMetException.class,
                () -> mine.getDiscoveredParticipantData(
                        Qos.env().getSPI().nilHandle()));
    }

    @Test
    void getDiscoveredTopicDataCarriesTheTopicQos() {
        DomainParticipant participant = Qos.factory().createParticipant(nextDom());
        Topic<String> topic = participant.createTopic("Chatter", String.class);
        assertTrue(participant.getDiscoveredTopics()
                .contains(topic.getInstanceHandle()));

        TopicBuiltinTopicData data = participant.getDiscoveredTopicData(
                topic.getInstanceHandle());
        assertEquals("Chatter", data.getName());
        assertNotNull(data.getReliability());
        assertNotNull(data.getHistory());
    }
}
