// SPDX-License-Identifier: Apache-2.0
package org.zerodds.pub;

import java.util.concurrent.CountDownLatch;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicReference;

import org.junit.jupiter.api.Test;
import org.omg.dds.Qos;
import org.omg.dds.domain.DomainParticipant;
import org.zerodds.rtps.RtpsParticipant;

import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * DDSI-RTPS 2.5 8.3.7.5: a HEARTBEAT announces only group sequence numbers whose changes are
 * entered in the history of their writer. The number is assigned and the change entered under one
 * lock; the HEARTBEAT reads the number under the same lock.
 */
class GroupHeartbeatLockTest {

    @Test
    void theHeartbeatWaitsForTheChangeOfANumberThatIsAssigned() throws Exception {
        DomainParticipant participant = Qos.factory().createParticipant(231);
        try {
            PublisherImpl publisher = (PublisherImpl) participant.createPublisher(Qos.publisher().withPolicy(
                    Qos.policies().Presentation().withGroup().withOrderedAccess(true).withCoherentAccess(true)));
            assertNull(publisher.currentGroupStamp(), "no number given out yet: no group info");
            final CountDownLatch assigned = new CountDownLatch(1);
            final CountDownLatch entered = new CountDownLatch(1);
            Thread writer = new Thread(new Runnable() {
                @Override
                public void run() {
                    publisher.stampAndWrite(new java.util.function.Function<RtpsParticipant.GroupStamp, Object>() {
                        @Override
                        public Object apply(RtpsParticipant.GroupStamp stamp) {
                            assigned.countDown();
                            try {
                                entered.await(10, TimeUnit.SECONDS);
                            } catch (InterruptedException e) {
                                Thread.currentThread().interrupt();
                            }
                            return null;
                        }
                    });
                }
            });
            writer.start();
            assertTrue(assigned.await(10, TimeUnit.SECONDS));
            final AtomicReference<RtpsParticipant.GroupStamp> seen = new AtomicReference<>();
            Thread heartbeat = new Thread(new Runnable() {
                @Override
                public void run() {
                    seen.set(publisher.currentGroupStamp());
                }
            });
            heartbeat.start();
            heartbeat.join(500);
            assertTrue(heartbeat.isAlive(), "the HEARTBEAT must not announce a number before its change is entered");
            assertNull(seen.get());
            entered.countDown();
            heartbeat.join(10_000);
            assertFalse(heartbeat.isAlive());
            assertNotNull(seen.get(), "after the change is entered the number is announced");
            writer.join(10_000);
        } finally {
            participant.close();
        }
    }
}
