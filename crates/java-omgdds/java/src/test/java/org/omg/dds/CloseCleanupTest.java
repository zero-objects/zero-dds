// SPDX-License-Identifier: Apache-2.0
package org.omg.dds;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.omg.dds.core.PreconditionNotMetException;
import org.omg.dds.domain.DomainParticipant;
import org.omg.dds.pub.DataWriter;
import org.omg.dds.pub.Publisher;
import org.omg.dds.sub.DataReader;
import org.omg.dds.sub.Subscriber;
import org.omg.dds.topic.Topic;
import org.zerodds.internal.InProcessBus;
import org.zerodds.topic.TopicImpl;

import java.util.ArrayList;

import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * A closed entity is gone from the entity that contained it (DDS 1.4 §2.2.2: delete_*), and a
 * Topic that a DataWriter or DataReader refers to cannot be deleted (delete_topic:
 * PRECONDITION_NOT_MET).
 */
class CloseCleanupTest {

    @BeforeEach
    void setUp() {
        InProcessBus.instance().reset();
    }

    @AfterEach
    void tearDown() {
        InProcessBus.instance().reset();
    }

    @Test
    void aClosedWriterAndReaderAreNotListedOrFoundAnyMore() {
        DomainParticipant participant = Qos.factory().createParticipant(970);
        Topic<byte[]> topic = participant.createTopic("Gone", byte[].class);
        Publisher publisher = participant.createPublisher();
        Subscriber subscriber = participant.createSubscriber();
        DataWriter<byte[]> writer = publisher.createDataWriter(topic);
        DataReader<byte[]> reader = subscriber.createDataReader(topic);
        assertTrue(publisher.<byte[]>lookupDataWriter("Gone") == writer);
        assertTrue(subscriber.getDataReaders(new ArrayList<>()).contains(reader));

        writer.close();
        reader.close();
        assertNull(publisher.lookupDataWriter("Gone"));
        assertNull(publisher.lookupDataWriter(topic));
        assertFalse(subscriber.getDataReaders(new ArrayList<>()).contains(reader));
        assertNull(subscriber.lookupDataReader("Gone"));
    }

    @Test
    void closeContainedEntitiesEmptiesPublisherAndSubscriber() {
        DomainParticipant participant = Qos.factory().createParticipant(971);
        Topic<byte[]> topic = participant.createTopic("Emptied", byte[].class);
        Publisher publisher = participant.createPublisher();
        Subscriber subscriber = participant.createSubscriber();
        publisher.createDataWriter(topic);
        subscriber.createDataReader(topic);
        publisher.closeContainedEntities();
        subscriber.closeContainedEntities();
        assertNull(publisher.lookupDataWriter("Emptied"));
        assertTrue(subscriber.getDataReaders(new ArrayList<>()).isEmpty());
    }

    @Test
    void aTopicInUseCannotBeClosedUntilItsWritersAndReadersAre() {
        DomainParticipant participant = Qos.factory().createParticipant(972);
        Topic<byte[]> topic = participant.createTopic("InUse", byte[].class);
        DataWriter<byte[]> writer = participant.createPublisher().createDataWriter(topic);
        DataReader<byte[]> reader = participant.createSubscriber().createDataReader(topic);
        assertThrows(PreconditionNotMetException.class, topic::close);
        writer.close();
        assertThrows(PreconditionNotMetException.class, topic::close);
        reader.close();
        topic.close();
        assertTrue(TopicImpl.isClosed(topic));
        assertNull(participant.lookupTopicDescription("InUse"));
    }

    @Test
    void aClosedPublisherAndSubscriberLeaveTheParticipant() {
        DomainParticipant participant = Qos.factory().createParticipant(973);
        Publisher publisher = participant.createPublisher();
        Subscriber subscriber = participant.createSubscriber();
        Topic<byte[]> topic = participant.createTopic("Container", byte[].class);
        DataWriter<byte[]> writer = publisher.createDataWriter(topic);
        assertTrue(participant.containsEntity(writer.getInstanceHandle()));
        publisher.close();
        subscriber.close();
        assertFalse(participant.containsEntity(publisher.getInstanceHandle()));
        assertFalse(participant.containsEntity(subscriber.getInstanceHandle()));
        assertFalse(participant.containsEntity(writer.getInstanceHandle()));
    }
}
