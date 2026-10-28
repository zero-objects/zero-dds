// SPDX-License-Identifier: Apache-2.0
package org.omg.dds;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.omg.dds.core.InstanceHandle;
import org.omg.dds.core.PreconditionNotMetException;
import org.omg.dds.domain.DomainParticipant;
import org.omg.dds.pub.DataWriter;
import org.omg.dds.sub.DataReader;
import org.omg.dds.sub.Sample;
import org.omg.dds.topic.Topic;
import org.zerodds.internal.Ignores;
import org.zerodds.internal.InProcessBus;

import java.util.List;
import java.util.concurrent.TimeoutException;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * DDS 1.4 §2.2.2.2.1.14 to §2.2.2.2.1.17: ignore_participant, ignore_topic, ignore_publication
 * and ignore_subscription. What a participant ignores is no longer matched with its endpoints
 * and sends it nothing. Domain ids 1230 to 1259.
 */
class ParticipantIgnoreTest {

    @BeforeEach
    void setUp() {
        InProcessBus.instance().reset();
    }

    @AfterEach
    void tearDown() {
        InProcessBus.instance().reset();
    }

    private static int received(DataReader<byte[]> reader) {
        List<Sample<byte[]>> samples = Qos.take(reader);
        return samples.size();
    }

    private static void write(DataWriter<byte[]> writer, int value) {
        try {
            writer.write(new byte[] {(byte) value});
        } catch (TimeoutException e) {
            throw new AssertionError(e);
        }
    }

    @Test
    void ignoreParticipantBeforeTheWriterExistsKeepsItsDataAndItsMatchAway() {
        int domain = 1230;
        DomainParticipant mine = Qos.factory().createParticipant(domain);
        DomainParticipant ignored = Qos.factory().createParticipant(domain);
        DomainParticipant friend = Qos.factory().createParticipant(domain);
        Topic<byte[]> topic = mine.createTopic("IgnoreParticipantFirst", byte[].class);
        DataReader<byte[]> reader = mine.createSubscriber().createDataReader(topic);

        mine.ignoreParticipant(ignored.getInstanceHandle());
        assertFalse(mine.getDiscoveredParticipants().contains(ignored.getInstanceHandle()));
        assertTrue(mine.getDiscoveredParticipants().contains(friend.getInstanceHandle()));
        assertThrows(PreconditionNotMetException.class,
                () -> mine.getDiscoveredParticipantData(ignored.getInstanceHandle()));

        DataWriter<byte[]> fromIgnored = ignored.createPublisher().createDataWriter(
                ignored.createTopic("IgnoreParticipantFirst", byte[].class));
        DataWriter<byte[]> fromFriend = friend.createPublisher().createDataWriter(
                friend.createTopic("IgnoreParticipantFirst", byte[].class));
        write(fromIgnored, 1);
        write(fromFriend, 2);

        assertEquals(1, reader.getSubscriptionMatchedStatus().getCurrentCount());
        assertEquals(0, fromIgnored.getPublicationMatchedStatus().getCurrentCount());
        assertEquals(1, fromFriend.getPublicationMatchedStatus().getCurrentCount());
        List<Sample<byte[]>> samples = Qos.take(reader);
        assertEquals(1, samples.size());
        assertEquals(2, samples.get(0).getData()[0]);
    }

    @Test
    void ignoreParticipantAfterTheMatchEndsItAndStopsTheData() {
        int domain = 1231;
        DomainParticipant mine = Qos.factory().createParticipant(domain);
        DomainParticipant other = Qos.factory().createParticipant(domain);
        Topic<byte[]> mineTopic = mine.createTopic("IgnoreParticipantLater", byte[].class);
        Topic<byte[]> otherTopic = other.createTopic("IgnoreParticipantLater", byte[].class);
        DataReader<byte[]> reader = mine.createSubscriber().createDataReader(mineTopic);
        DataWriter<byte[]> writer = other.createPublisher().createDataWriter(otherTopic);
        write(writer, 1);
        assertEquals(1, received(reader));
        assertEquals(1, reader.getSubscriptionMatchedStatus().getCurrentCount());

        mine.ignoreParticipant(other.getInstanceHandle());
        assertEquals(0, reader.getSubscriptionMatchedStatus().getCurrentCount());
        assertEquals(0, writer.getPublicationMatchedStatus().getCurrentCount());
        write(writer, 2);
        assertEquals(0, received(reader));
    }

    @Test
    void anIgnoredParticipantDoesNotIgnoreTheOtherWay() {
        int domain = 1232;
        DomainParticipant mine = Qos.factory().createParticipant(domain);
        DomainParticipant other = Qos.factory().createParticipant(domain);
        mine.ignoreParticipant(other.getInstanceHandle());
        // the other participant still lists mine: it ignored nobody
        assertTrue(other.getDiscoveredParticipants().contains(mine.getInstanceHandle()));
        assertEquals(org.zerodds.topic.BuiltinTopicKeyImpl.of(Qos.env(), mine.getInstanceHandle()),
                other.getDiscoveredParticipantData(mine.getInstanceHandle()).getKey());
    }

    @Test
    void ignoreTopicSeparatesTheTopicOfTheIgnoredHandleFromTheEndpointsOfTheParticipant() {
        int domain = 1233;
        DomainParticipant mine = Qos.factory().createParticipant(domain);
        DomainParticipant other = Qos.factory().createParticipant(domain);
        Topic<byte[]> mineTopic = mine.createTopic("IgnoreTopic", byte[].class);
        Topic<byte[]> otherTopic = other.createTopic("IgnoreTopic", byte[].class);
        Topic<byte[]> keptTopic = other.createTopic("KeptTopic", byte[].class);
        Topic<byte[]> mineKept = mine.createTopic("KeptTopic", byte[].class);

        DataReader<byte[]> reader = mine.createSubscriber().createDataReader(mineTopic);
        DataReader<byte[]> keptReader = mine.createSubscriber().createDataReader(mineKept);
        DataWriter<byte[]> writer = other.createPublisher().createDataWriter(otherTopic);
        DataWriter<byte[]> keptWriter = other.createPublisher().createDataWriter(keptTopic);
        write(writer, 1);
        write(keptWriter, 1);
        assertEquals(1, received(reader));
        assertEquals(1, received(keptReader));
        assertTrue(mine.getDiscoveredTopics().contains(otherTopic.getInstanceHandle()));

        mine.ignoreTopic(otherTopic.getInstanceHandle());
        assertFalse(mine.getDiscoveredTopics().contains(otherTopic.getInstanceHandle()));
        assertTrue(mine.getDiscoveredTopics().contains(keptTopic.getInstanceHandle()));
        assertThrows(PreconditionNotMetException.class,
                () -> mine.getDiscoveredTopicData(otherTopic.getInstanceHandle()));
        assertEquals(0, reader.getSubscriptionMatchedStatus().getCurrentCount());
        write(writer, 2);
        write(keptWriter, 2);
        assertEquals(0, received(reader));
        assertEquals(1, received(keptReader));
    }

    @Test
    void ignorePublicationStopsTheDataOfOneWriterOnly() {
        int domain = 1234;
        DomainParticipant mine = Qos.factory().createParticipant(domain);
        DomainParticipant other = Qos.factory().createParticipant(domain);
        Topic<byte[]> mineTopic = mine.createTopic("IgnorePublication", byte[].class);
        Topic<byte[]> otherTopic = other.createTopic("IgnorePublication", byte[].class);
        DataReader<byte[]> reader = mine.createSubscriber().createDataReader(mineTopic,
                Qos.readerKeepAll());
        DataWriter<byte[]> ignored = other.createPublisher().createDataWriter(otherTopic);
        DataWriter<byte[]> kept = other.createPublisher().createDataWriter(otherTopic);
        assertEquals(2, reader.getMatchedPublications().size());

        InstanceHandle handle = ignored.getInstanceHandle();
        mine.ignorePublication(handle);
        assertEquals(1, reader.getMatchedPublications().size());
        assertEquals(1, reader.getSubscriptionMatchedStatus().getCurrentCount());
        write(ignored, 1);
        write(kept, 2);
        List<Sample<byte[]>> samples = Qos.take(reader);
        assertEquals(1, samples.size());
        assertEquals(2, samples.get(0).getData()[0]);
        assertEquals(0, ignored.getPublicationMatchedStatus().getCurrentCount());
        assertEquals(1, kept.getPublicationMatchedStatus().getCurrentCount());
    }

    @Test
    void ignoreSubscriptionStopsTheDataToOneReaderOnly() {
        int domain = 1235;
        DomainParticipant mine = Qos.factory().createParticipant(domain);
        DomainParticipant other = Qos.factory().createParticipant(domain);
        Topic<byte[]> mineTopic = mine.createTopic("IgnoreSubscription", byte[].class);
        Topic<byte[]> otherTopic = other.createTopic("IgnoreSubscription", byte[].class);
        DataWriter<byte[]> writer = mine.createPublisher().createDataWriter(mineTopic);
        DataReader<byte[]> ignored = other.createSubscriber().createDataReader(otherTopic);
        DataReader<byte[]> kept = other.createSubscriber().createDataReader(otherTopic);
        assertEquals(2, writer.getMatchedSubscriptions().size());

        mine.ignoreSubscription(ignored.getInstanceHandle());
        assertEquals(1, writer.getMatchedSubscriptions().size());
        assertTrue(writer.getMatchedSubscriptions().contains(kept.getInstanceHandle()));
        assertEquals(1, writer.getPublicationMatchedStatus().getCurrentCount());
        write(writer, 5);
        assertEquals(0, received(ignored));
        assertEquals(1, received(kept));
    }

    @Test
    void theIgnoreSetsOfAParticipantEndWithIt() {
        int domain = 1236;
        DomainParticipant mine = Qos.factory().createParticipant(domain);
        DomainParticipant other = Qos.factory().createParticipant(domain);
        mine.ignoreParticipant(other.getInstanceHandle());
        assertTrue(Ignores.of(mine.getInstanceHandle()).ignoresParticipant(other.getInstanceHandle()));
        mine.close();
        // a participant that is gone takes its ignore sets with it
        assertFalse(Ignores.of(mine.getInstanceHandle()).ignoresParticipant(other.getInstanceHandle()));
    }

    @Test
    void theBuiltinReaderOfAParticipantGivesNoDataAboutItsOwnEntities() {
        int domain = 1237;
        DomainParticipant mine = Qos.factory().createParticipant(domain);
        DomainParticipant other = Qos.factory().createParticipant(domain);
        Topic<byte[]> topic = mine.createTopic("OwnEntities", byte[].class);
        mine.createPublisher().createDataWriter(topic);
        other.createPublisher().createDataWriter(other.createTopic("OtherEntities", byte[].class));

        DataReader<org.zerodds.topic.PublicationBuiltinTopicDataImpl> publications =
                mine.getBuiltinSubscriber().lookupDataReader(
                        org.zerodds.topic.BuiltinTopics.PUBLICATION);
        boolean own = false;
        boolean foreign = false;
        for (Sample<org.zerodds.topic.PublicationBuiltinTopicDataImpl> sample : Qos.take(publications)) {
            if (sample.getData() == null) {
                continue;
            }
            own |= "OwnEntities".equals(sample.getData().getTopicName());
            foreign |= "OtherEntities".equals(sample.getData().getTopicName());
        }
        assertFalse(own, "DDS 1.4 §2.2.5: nothing about the entities of the participant itself");
        assertTrue(foreign);
    }
}
