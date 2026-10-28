// SPDX-License-Identifier: Apache-2.0
package org.omg.dds;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.omg.dds.domain.DomainParticipant;
import org.omg.dds.domain.DomainParticipantFactory;
import org.zerodds.internal.InProcessBus;

import static org.junit.jupiter.api.Assertions.assertNotSame;
import static org.junit.jupiter.api.Assertions.assertThrows;
import org.zerodds.core.AlreadyClosedException;

/**
 * DDS 1.2 §7.1.2.2.2.1 — create_participant creates a new DomainParticipant
 * object on every call; §7.2.3 — an Entity's factory operations refuse a
 * closed Entity.
 */
class DomainParticipantFactoryTest {

    private static int dom = 40;

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
    void createParticipantCreatesANewParticipantEachCall() {
        int domain = nextDom();
        DomainParticipant first =
                Qos.factory().createParticipant(domain);
        DomainParticipant second =
                Qos.factory().createParticipant(domain);
        assertNotSame(first, second, "create_participant returns a new object each call");
        first.close();
        second.close();
    }

    @Test
    void createPublisherOnClosedParticipantThrows() {
        DomainParticipant participant =
                Qos.factory().createParticipant(nextDom());
        participant.close();
        assertThrows(AlreadyClosedException.class, participant::createPublisher);
    }

    @Test
    void createTopicOnClosedParticipantThrows() {
        DomainParticipant participant =
                Qos.factory().createParticipant(nextDom());
        participant.close();
        assertThrows(AlreadyClosedException.class,
                () -> participant.createTopic("t", byte[].class));
    }
}
