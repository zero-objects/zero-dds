// SPDX-License-Identifier: Apache-2.0
package org.omg.dds;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.omg.dds.domain.DomainParticipant;
import org.omg.dds.domain.DomainParticipantFactory;
import org.zerodds.ServiceEnvironmentImpl;
import org.zerodds.internal.InProcessBus;

import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.CyclicBarrier;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.Future;
import java.util.concurrent.TimeUnit;

import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotSame;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * The DomainParticipantFactory is one per ServiceEnvironment (DDS-Java-PSM 1.0 §7.4.1) and its
 * operations are reentrant (§7.2.4). Domain id 1399.
 */
class ParticipantFactoryEnvironmentTest {

    @BeforeEach
    void setUp() {
        InProcessBus.instance().reset();
    }

    @AfterEach
    void tearDown() {
        InProcessBus.instance().reset();
    }

    @Test
    void threadsThatAskForTheFactoryOfANewEnvironmentTogetherGetTheSameOne() throws Exception {
        int threads = 8;
        ExecutorService pool = Executors.newFixedThreadPool(threads);
        try {
            for (int round = 0; round < 25; round++) {
                ServiceEnvironmentImpl environment = new ServiceEnvironmentImpl();
                CyclicBarrier together = new CyclicBarrier(threads);
                List<Future<DomainParticipantFactory>> factories = new ArrayList<>();
                for (int t = 0; t < threads; t++) {
                    factories.add(pool.submit(() -> {
                        together.await(20, TimeUnit.SECONDS);
                        return DomainParticipantFactory.getInstance(environment);
                    }));
                }
                DomainParticipantFactory first = factories.get(0).get(30, TimeUnit.SECONDS);
                for (Future<DomainParticipantFactory> factory : factories) {
                    assertSame(first, factory.get(30, TimeUnit.SECONDS));
                }
                assertSame(first, DomainParticipantFactory.getInstance(environment));
            }
        } finally {
            pool.shutdownNow();
        }
    }

    @Test
    void everyEnvironmentHasItsOwnFactoryAndItsOwnParticipantTable() {
        ServiceEnvironmentImpl other = new ServiceEnvironmentImpl();
        DomainParticipantFactory mine = Qos.factory();
        DomainParticipantFactory theirs = DomainParticipantFactory.getInstance(other);
        assertNotSame(mine, theirs);

        DomainParticipant participant = mine.createParticipant(1399);
        assertSame(participant, mine.lookupParticipant(1399));
        assertNull(theirs.lookupParticipant(1399));
        assertTrue(participant.getEnvironment() == Qos.env());

        // the in-process bus is shared, so the participants of both environments in a domain
        // discover each other
        DomainParticipant foreign = theirs.createParticipant(1399);
        assertSame(foreign, theirs.lookupParticipant(1399));
        assertTrue(participant.getDiscoveredParticipants().contains(foreign.getInstanceHandle()));
        assertTrue(foreign.getDiscoveredParticipants().contains(participant.getInstanceHandle()));
        foreign.close();
        assertFalse(participant.getDiscoveredParticipants().contains(foreign.getInstanceHandle()));
    }
}
