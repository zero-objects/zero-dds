// SPDX-License-Identifier: Apache-2.0
package org.omg.dds;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.omg.dds.core.InstanceHandle;
import org.omg.dds.core.PreconditionNotMetException;
import org.omg.dds.core.status.InconsistentTopicStatus;
import org.omg.dds.domain.DomainParticipant;
import org.omg.dds.topic.ParticipantBuiltinTopicData;
import org.omg.dds.topic.Topic;
import org.omg.dds.topic.TopicBuiltinTopicData;
import org.zerodds.core.Values;
import org.zerodds.internal.InProcessBus;
import org.zerodds.rtps.Rtps;
import org.zerodds.rtps.RtpsParticipant;
import org.zerodds.testing.TestDomains;
import org.zerodds.topic.BuiltinData;

import java.util.function.BooleanSupplier;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * What SPDP and SEDP announce reaches the discovery operations of the participant
 * (DDS 1.4 §2.2.2.2.1.27 to §2.2.2.2.1.30) and INCONSISTENT_TOPIC (§2.2.4.1): a participant of
 * another process is simulated by a second RTPS participant in the domain, as {@code RtpsWireTest}
 * does. Each test leases its domain from the band of the Rust tests
 * ({@code org.zerodds.testing.TestDomains}) and gives it back with no participant left in it.
 */
class ParticipantWireDiscoveryTest {

    private String previous;
    /** The domain of the test, leased from the band of crates/test-domains. */
    private TestDomains.Lease lease;
    private int domain;

    @BeforeEach
    void setUp() {
        lease = TestDomains.lease();
        domain = lease.id();
        previous = System.getProperty("zerodds.rtps.enable");
        System.setProperty("zerodds.rtps.enable", "true");
        InProcessBus.instance().reset();
    }

    @AfterEach
    void tearDown() {
        if (previous == null) {
            System.clearProperty("zerodds.rtps.enable");
        } else {
            System.setProperty("zerodds.rtps.enable", previous);
        }
        InProcessBus.instance().reset();
        // The participant the PSM made for the domain leaves it before the lease goes.
        RtpsParticipant.closeDomain(domain);
        lease.close();
    }

    private static void waitFor(BooleanSupplier condition, String what) throws InterruptedException {
        long end = System.currentTimeMillis() + 20_000;
        while (!condition.getAsBoolean() && System.currentTimeMillis() < end) {
            Thread.sleep(50);
        }
        assertTrue(condition.getAsBoolean(), what);
    }

    @Test
    void aParticipantOfAnotherProcessIsDiscoveredAndCanBeIgnored() throws Exception {
        DomainParticipant mine = Qos.factory().createParticipant(domain);
        RtpsParticipant remote = RtpsParticipant.newParticipant(domain);
        try {
            byte[] guid = BuiltinData.participantGuid(remote.guidPrefix());
            InstanceHandle handle = Values.of(Qos.env(), guid);
            waitFor(() -> mine.getDiscoveredParticipants().contains(handle),
                    "SPDP announces the participant");

            ParticipantBuiltinTopicData data = mine.getDiscoveredParticipantData(handle);
            assertEquals(BuiltinData.keyOfGuid(Qos.env(), guid), data.getKey());

            mine.ignoreParticipant(handle);
            assertFalse(mine.getDiscoveredParticipants().contains(handle));
            assertThrows(PreconditionNotMetException.class,
                    () -> mine.getDiscoveredParticipantData(handle));
        } finally {
            remote.close();
            mine.close();
        }
    }

    @Test
    void aTopicOfAnotherProcessIsDiscoveredAndATypeThatDiffersIsInconsistent() throws Exception {
        DomainParticipant mine = Qos.factory().createParticipant(domain);
        Topic<String> local = mine.createTopic("WireTopic", String.class);
        RtpsParticipant remote = RtpsParticipant.newParticipant(domain);
        try {
            remote.createWriter("WireTopic", "other::Type", Rtps.ENCAP_CDR_LE, new byte[0],
                    false, false);
            waitFor(() -> local.getStatusChanges().contains(InconsistentTopicStatus.class),
                    "SEDP announces a writer of the topic with another type");
            InconsistentTopicStatus status = local.getInconsistentTopicStatus();
            assertTrue(status.getTotalCount() >= 1);
            assertEquals(status.getTotalCount(), status.getTotalCountChange());

            InstanceHandle remoteTopic = null;
            for (InstanceHandle handle : mine.getDiscoveredTopics()) {
                TopicBuiltinTopicData data = mine.getDiscoveredTopicData(handle);
                if ("WireTopic".equals(data.getName()) && "other::Type".equals(data.getTypeName())) {
                    remoteTopic = handle;
                }
            }
            assertNotNull(remoteTopic, "the topic of the announced writer is a discovered topic");
            // it is not the topic this participant created
            assertTrue(!remoteTopic.equals(local.getInstanceHandle()));
        } finally {
            remote.close();
            mine.close();
        }
    }
}
