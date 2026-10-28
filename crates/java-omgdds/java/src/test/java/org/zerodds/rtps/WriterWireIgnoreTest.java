// SPDX-License-Identifier: Apache-2.0
package org.zerodds.rtps;

import org.junit.jupiter.api.Test;
import org.zerodds.testing.TestDomains;

import java.nio.charset.StandardCharsets;
import java.util.concurrent.LinkedBlockingQueue;
import java.util.concurrent.TimeUnit;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * ignore_participant, ignore_publication, ignore_subscription and ignore_topic on the RTPS
 * stack (DDS 1.4 §2.2.2.2.1.14 to §2.2.2.2.1.17): a writer does not match, and sends nothing
 * to, a reader that is ignored, though SEDP announces it again. Three participants of a domain
 * meet over UDP, as in {@code RtpsWireTest}. Each test leases its domain from the band of the Rust tests
 * ({@code org.zerodds.testing.TestDomains}) and gives it back with no participant left in it.
 */
class WriterWireIgnoreTest {


    private static void waitFor(java.util.function.BooleanSupplier condition, String what)
            throws InterruptedException {
        long end = System.currentTimeMillis() + 20_000;
        while (!condition.getAsBoolean() && System.currentTimeMillis() < end) {
            Thread.sleep(50);
        }
        assertTrue(condition.getAsBoolean(), what);
    }

    /** A writer of one participant and a reader in each of two others, all three met. */
    private static final class Rig implements AutoCloseable {
        final TestDomains.Lease lease = TestDomains.lease();
        final RtpsParticipant publisher = RtpsParticipant.newParticipant(lease.id());
        final RtpsParticipant first = RtpsParticipant.newParticipant(lease.id());
        final RtpsParticipant second = RtpsParticipant.newParticipant(lease.id());
        final LinkedBlockingQueue<byte[]> firstGot = new LinkedBlockingQueue<>();
        final LinkedBlockingQueue<byte[]> secondGot = new LinkedBlockingQueue<>();
        final RtpsParticipant.WireReader firstReader;
        final RtpsParticipant.WireWriter writer;

        Rig(String topic) throws InterruptedException {
            firstReader = first.createReader(topic, "zerodds::RawBytes", false, firstGot::add);
            second.createReader(topic, "zerodds::RawBytes", false, secondGot::add);
            writer = publisher.createWriter(topic, "zerodds::RawBytes", Rtps.ENCAP_CDR_LE,
                    new byte[0], false, false);
            waitFor(() -> writer.matchedCount() == 2, "the writer meets both readers over SEDP");
        }

        @Override
        public void close() {
            publisher.close();
            first.close();
            second.close();
            lease.close();
        }
    }

    private static void expectOnlySecond(Rig rig) throws Exception {
        byte[] message = "after the ignore".getBytes(StandardCharsets.UTF_8);
        byte[] received = null;
        for (int i = 0; i < 20 && received == null; i++) {
            rig.writer.write(message);
            received = rig.secondGot.poll(500, TimeUnit.MILLISECONDS);
        }
        assertNotNull(received, "the reader that is not ignored still gets the samples");
        assertNull(rig.firstGot.poll(500, TimeUnit.MILLISECONDS),
                "the ignored reader gets nothing");
        // SEDP announces the reader again and again; the ignore holds
        Thread.sleep(2_000);
        assertEquals(1, rig.writer.matchedCount());
    }

    @Test
    void ignoreParticipantEndsTheMatchWithItsReadersAndKeepsItEnded() throws Exception {
        try (Rig rig = new Rig("IgnoreWireParticipant")) {
            rig.publisher.ignoreParticipant(rig.first.guidPrefix());
            assertEquals(1, rig.writer.matchedCount());
            expectOnlySecond(rig);
        }
    }

    @Test
    void ignoreEndpointEndsTheMatchWithOneReader() throws Exception {
        try (Rig rig = new Rig("IgnoreWireEndpoint")) {
            rig.publisher.ignoreEndpoint(rig.firstReader.guid);
            assertEquals(1, rig.writer.matchedCount());
            expectOnlySecond(rig);
        }
    }

    @Test
    void ignoreTopicEndsTheMatchWithTheReadersOfTheTopic() throws Exception {
        try (Rig rig = new Rig("IgnoreWireTopic")) {
            rig.publisher.ignoreTopic("IgnoreWireTopic");
            assertEquals(0, rig.writer.matchedCount());
            Thread.sleep(2_000);
            assertEquals(0, rig.writer.matchedCount());
        }
    }
}
