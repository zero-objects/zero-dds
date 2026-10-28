// SPDX-License-Identifier: Apache-2.0
package org.omg.dds;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.omg.dds.domain.DomainParticipant;
import org.omg.dds.pub.DataWriter;
import org.omg.dds.pub.Publisher;
import org.omg.dds.topic.Topic;
import org.zerodds.internal.InProcessBus;
import org.zerodds.rtps.RtpsParticipant;
import org.zerodds.testing.TestDomains;

import java.util.concurrent.TimeUnit;
import java.util.concurrent.TimeoutException;
import java.util.function.BooleanSupplier;

import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * wait_for_acknowledgments of a DataWriter whose reader is in another process (DDS 1.4
 * §2.2.2.4.2.12): the wait ends when the reader has acknowledged the sample with ACKNACK, and
 * is a TimeoutException while it has not. The other process is a second RTPS participant in
 * the domain, as in {@code RtpsWireTest}. Each test leases its domain from the band of the Rust tests
 * ({@code org.zerodds.testing.TestDomains}) and gives it back with no participant left in it.
 */
class WriterWireAckTest {

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
    void aSampleTheRemoteReaderAcknowledgedEndsTheWait() throws Exception {
        DomainParticipant participant = Qos.factory().createParticipant(domain);
        Topic<byte[]> topic = participant.createTopic("WireAck", byte[].class);
        Publisher publisher = participant.createPublisher();
        DataWriter<byte[]> writer = publisher.createDataWriter(topic);
        RtpsParticipant remote = RtpsParticipant.newParticipant(domain);
        try {
            remote.createReader("WireAck", topic.getTypeName(), false, body -> { });
            waitFor(() -> writer.getMatchedSubscriptions().size() == 1,
                    "the writer meets the reader of the other participant over SEDP");
            writer.write(new byte[] {1, 2, 3});
            // HEARTBEAT from the writer, ACKNACK from the reader (DDSI-RTPS 2.5 8.4.7.1)
            writer.waitForAcknowledgments(20, TimeUnit.SECONDS);
            publisher.waitForAcknowledgments(20, TimeUnit.SECONDS);
        } finally {
            remote.close();
            participant.close();
        }
    }

    @Test
    void aSampleNobodyAcknowledgesIsATimeoutException() throws Exception {
        DomainParticipant participant = Qos.factory().createParticipant(domain);
        Topic<byte[]> topic = participant.createTopic("WireNoAck", byte[].class);
        Publisher publisher = participant.createPublisher();
        DataWriter<byte[]> writer = publisher.createDataWriter(topic);
        RtpsParticipant remote = RtpsParticipant.newParticipant(domain);
        try {
            remote.createReader("WireNoAck", topic.getTypeName(), false, body -> { });
            waitFor(() -> writer.getMatchedSubscriptions().size() == 1,
                    "the writer meets the reader of the other participant over SEDP");
            // the reader goes away without telling: its lease has not run out, the writer
            // still sends to it, and nothing acknowledges
            remote.close();
            writer.write(new byte[] {1, 2, 3});
            assertThrows(TimeoutException.class,
                    () -> writer.waitForAcknowledgments(500, TimeUnit.MILLISECONDS));
            assertThrows(TimeoutException.class,
                    () -> publisher.waitForAcknowledgments(100, TimeUnit.MILLISECONDS));
        } finally {
            remote.close();
            participant.close();
        }
    }

    @Test
    void aWriteBeyondMaxSamplesWaitsForAcknowledgmentsAndTimesOut() throws Exception {
        DomainParticipant participant = Qos.factory().createParticipant(domain);
        Topic<byte[]> topic = participant.createTopic("WireLimit", byte[].class);
        DataWriter<byte[]> writer = participant.createPublisher().createDataWriter(topic,
                Qos.writer()
                        .withPolicy(Qos.pf().History().withKeepAll())
                        .withPolicy(Qos.pf().ResourceLimits().withMaxSamples(1))
                        .withPolicy(Qos.policies().DataWriterReliability().withReliable()
                                .withMaxBlockingTime(300, TimeUnit.MILLISECONDS)));
        RtpsParticipant remote = RtpsParticipant.newParticipant(domain);
        try {
            remote.createReader("WireLimit", topic.getTypeName(), false, body -> { });
            waitFor(() -> writer.getMatchedSubscriptions().size() == 1,
                    "the writer meets the reader of the other participant over SEDP");
            // the reader goes away without telling: nothing acknowledges the first sample
            remote.close();
            writer.write(new byte[] {1});
            assertThrows(TimeoutException.class, () -> writer.write(new byte[] {2}));
        } finally {
            remote.close();
            participant.close();
        }
    }
}
