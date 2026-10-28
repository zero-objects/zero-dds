// SPDX-License-Identifier: Apache-2.0
package org.omg.dds;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.omg.dds.core.PreconditionNotMetException;
import org.omg.dds.domain.DomainParticipant;
import org.omg.dds.pub.DataWriter;
import org.omg.dds.sub.DataReader;
import org.omg.dds.sub.Subscriber;
import org.omg.dds.topic.ContentFilteredTopic;
import org.omg.dds.topic.MultiTopic;
import org.omg.dds.topic.Topic;
import org.omg.dds.type.Key;
import org.zerodds.core.AlreadyClosedException;
import org.zerodds.internal.InProcessBus;

import java.util.Collections;

import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * DDS-Java-PSM 1.0 §7.2.3: close() on a ContentFilteredTopic and a MultiTopic
 * has the effect of a deletion (no reader can be created from a closed
 * description), and closing a Subscriber or a DomainParticipant closes the
 * DataReaders it contains.
 */
class CoreCloseTest {

    private static final int DOMAIN = 1507;

    /** A sample type with a member to filter on. */
    public static final class Item {
        @Key
        public int id;
        public int value;

        public Item() {}
    }

    private DomainParticipant participant;
    private Topic<Item> topic;

    @BeforeEach
    void setUp() {
        InProcessBus.instance().reset();
        participant = Qos.factory().createParticipant(DOMAIN);
        topic = participant.createTopic("CoreCloseItems", Item.class);
    }

    @AfterEach
    void tearDown() {
        if (!Qos.isClosed(participant)) {
            participant.close();
        }
        InProcessBus.instance().reset();
    }

    @Test
    void aClosedContentFilteredTopicRefusesToCreateAReader() {
        Subscriber subscriber = participant.createSubscriber();
        ContentFilteredTopic<Item> filtered = participant.createContentFilteredTopic(
                "CoreCloseFiltered", topic, "value > 5", Collections.<String>emptyList());
        DataReader<Item> reader = subscriber.createDataReader(filtered);
        assertNotNull(reader);
        // DDS 1.4 2.2.2.2.1.8 delete_contentfilteredtopic: PRECONDITION_NOT_MET while a reader uses it
        assertThrows(PreconditionNotMetException.class, filtered::close);
        reader.close();
        filtered.close();
        assertThrows(AlreadyClosedException.class, () -> subscriber.createDataReader(filtered));
        // the related topic is not closed by it
        assertFalse(Qos.isClosed(topic));
        assertNotNull(subscriber.createDataReader(topic));
    }

    @Test
    void aClosedMultiTopicRefusesToCreateAReader() {
        Subscriber subscriber = participant.createSubscriber();
        MultiTopic<Item> multiTopic = participant.createMultiTopic("CoreCloseMulti", Item.class,
                "SELECT * FROM CoreCloseItems WHERE value > 5", Collections.<String>emptyList());
        DataReader<Item> reader = subscriber.createDataReader(multiTopic);
        assertNotNull(reader);
        // DDS 1.4 2.2.2.2.1.10 delete_multitopic: PRECONDITION_NOT_MET while a reader uses it
        assertThrows(PreconditionNotMetException.class, multiTopic::close);
        reader.close();
        multiTopic.close();
        assertThrows(AlreadyClosedException.class, () -> subscriber.createDataReader(multiTopic));
        assertFalse(Qos.isClosed(topic));
    }

    @Test
    void closingASubscriberClosesItsReaders() {
        Subscriber subscriber = participant.createSubscriber();
        DataReader<Item> reader = subscriber.createDataReader(topic);
        assertFalse(Qos.isClosed(reader));
        subscriber.close();
        assertTrue(Qos.isClosed(subscriber));
        assertTrue(Qos.isClosed(reader), "closing the Subscriber closes its DataReader");
    }

    @Test
    void closingTheParticipantClosesTheReadersAndWritersInIt() {
        DataReader<Item> reader = participant.createSubscriber().createDataReader(topic);
        DataWriter<Item> writer = participant.createPublisher().createDataWriter(topic);
        participant.close();
        assertTrue(Qos.isClosed(reader), "closing the DomainParticipant closes its DataReader");
        assertTrue(Qos.isClosed(writer), "closing the DomainParticipant closes its DataWriter");
        assertTrue(Qos.isClosed(topic));
    }
}
