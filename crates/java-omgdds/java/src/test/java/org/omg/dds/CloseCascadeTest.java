// SPDX-License-Identifier: Apache-2.0
package org.omg.dds;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.omg.dds.domain.DomainParticipant;
import org.omg.dds.domain.DomainParticipantFactory;
import org.omg.dds.pub.DataWriter;
import org.omg.dds.pub.Publisher;
import org.omg.dds.sub.DataReader;
import org.omg.dds.sub.Subscriber;
import org.omg.dds.topic.Topic;
import org.zerodds.internal.InProcessBus;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;
import org.zerodds.core.AlreadyClosedException;
import org.zerodds.topic.TopicImpl;

/**
 * DDS-Java-PSM 1.0 §7.2.3 — "Closing an Entity implicitly closes all of its
 * contained objects."
 */
class CloseCascadeTest {

    private static int dom = 50;

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
    void publisherCloseClosesItsWriters() {
        DomainParticipant participant =
                Qos.factory().createParticipant(nextDom());
        Topic<byte[]> topic = participant.createTopic("cascade", byte[].class);
        Publisher pub = participant.createPublisher();
        DataWriter<byte[]> writer = pub.createDataWriter(topic);
        Subscriber sub = participant.createSubscriber();
        DataReader<byte[]> reader = sub.createDataReader(topic);

        pub.close();
        assertTrue(Qos.isClosed(writer), "closing the Publisher closes its DataWriter");
        // RETCODE_ALREADY_DELETED surfaces as an exception (§7.3.2, Table 7.1).
        assertThrows(AlreadyClosedException.class, () -> writer.write(new byte[] {1}));
        assertThrows(AlreadyClosedException.class, () -> pub.createDataWriter(topic));

        participant.close();
    }

    @Test
    void participantCloseClosesContainedEntities() {
        DomainParticipant participant =
                Qos.factory().createParticipant(nextDom());
        Topic<byte[]> topic = participant.createTopic("pcascade", byte[].class);
        Publisher pub = participant.createPublisher();
        Subscriber sub = participant.createSubscriber();

        participant.close();
        assertTrue(TopicImpl.isClosed(topic), "Participant.close closes its Topics");
        // Closing an entity makes every operation on it fail (§7.3.2, Table 7.1).
        assertThrows(AlreadyClosedException.class, () -> participant.createPublisher());
        assertThrows(AlreadyClosedException.class, () -> pub.createDataWriter(topic));
        assertThrows(AlreadyClosedException.class, () -> sub.createDataReader(topic));
        assertThrows(AlreadyClosedException.class, () -> topic.enable());
    }
}
