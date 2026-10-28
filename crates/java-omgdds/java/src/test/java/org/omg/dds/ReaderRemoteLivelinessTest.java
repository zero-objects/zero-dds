// SPDX-License-Identifier: Apache-2.0
package org.omg.dds;

import java.util.function.BooleanSupplier;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.omg.dds.domain.DomainParticipant;
import org.omg.dds.sub.DataReader;
import org.omg.dds.topic.Topic;
import org.zerodds.internal.InProcessBus;
import org.zerodds.rtps.QosParameters;
import org.zerodds.rtps.RtpsParticipant;
import org.zerodds.testing.TestDomains;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * LIVELINESS_CHANGED of a DataReader whose writer is in another process (DDS 1.4 2.2.4.1,
 * DDSI-RTPS 2.5 8.3.7.5): the writer counts as alive within the lease it announced, renewed by
 * its DATA and by a HEARTBEAT with the L flag. The other process is a second RTPS participant,
 * as in {@code WriterWireAckTest}. Each test leases its domain from the band of the Rust tests
 * ({@code org.zerodds.testing.TestDomains}) and gives it back with no participant left in it.
 */
class ReaderRemoteLivelinessTest {

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
    void aRemoteWriterIsAliveWithinItsLeaseAndAliveAgainAfterAnAssertion() throws Exception {
        DomainParticipant participant = Qos.factory().createParticipant(domain);
        Topic<byte[]> topic = participant.createTopic("WireLiveliness", byte[].class);
        DataReader<byte[]> reader = participant.createSubscriber().createDataReader(topic);
        RtpsParticipant remote = RtpsParticipant.newParticipant(domain);
        try {
            QosParameters qos = new QosParameters();
            qos.liveliness = new QosParameters.Liveliness();
            qos.liveliness.kind = 2; // MANUAL_BY_TOPIC_LIVELINESS_QOS
            qos.liveliness.lease = QosParameters.Dur.ofNanos(1, 0);
            qos.reliability = new QosParameters.Reliability();
            qos.reliability.kind = org.zerodds.rtps.Rtps.RELIABILITY_RELIABLE;
            RtpsParticipant.WireWriter writer = remote.createWriter("WireLiveliness",
                    topic.getTypeName(), new byte[] {0, 7, 0, 0}, new byte[0], new byte[0],
                    false, true, qos);
            waitFor(() -> reader.getLivelinessChangedStatus().getAliveCount() == 1,
                    "the writer of the other participant is alive when it is matched");
            waitFor(() -> reader.getLivelinessChangedStatus().getNotAliveCount() == 1,
                    "the lease of one second passes without a sign of the writer");
            assertEquals(0, reader.getLivelinessChangedStatus().getAliveCount());
            writer.assertLiveliness();
            waitFor(() -> reader.getLivelinessChangedStatus().getAliveCount() == 1,
                    "the HEARTBEAT with the L flag renews the lease");
        } finally {
            remote.close();
            participant.close();
        }
    }
}
