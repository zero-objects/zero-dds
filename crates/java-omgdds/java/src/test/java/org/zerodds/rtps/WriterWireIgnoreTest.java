// SPDX-License-Identifier: Apache-2.0
package org.zerodds.rtps;

import org.junit.jupiter.api.Test;
import org.zerodds.testing.TestDomains;
import org.zerodds.testing.Waits;

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
 * meet over UDP. Each test leases its domain from the band of the Rust tests
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

    /**
     * Has the participant of the ignored reader announce its endpoints again (a new endpoint
     * makes it announce all of them, RtpsParticipant.announceAllToPeers) and returns once the
     * publishing participant has discovered the new one, which it does after the announcement
     * of the ignored reader that precedes it on the same SEDP writer.
     */
    private static void announceAgain(Rig rig, String topic) {
        java.util.Set<String> seen = java.util.concurrent.ConcurrentHashMap.newKeySet();
        rig.publisher.addDiscoveryListener(new RtpsParticipant.DiscoveryListener() {
            @Override
            public void participantDiscovered(DiscoveryData.Participant participant) {
            }

            @Override
            public void participantLost(DiscoveryData.Participant participant) {
            }

            @Override
            public void endpointDiscovered(DiscoveryData.Endpoint endpoint) {
                seen.add(java.util.HexFormat.of().formatHex(endpoint.endpointGuid));
            }

            @Override
            public void endpointLost(DiscoveryData.Endpoint endpoint) {
            }
        });
        RtpsParticipant.WireReader fresh = rig.first.createReader(topic + "Again", "zerodds::RawBytes",
                false, body -> { });
        String key = java.util.HexFormat.of().formatHex(fresh.guid);
        Waits.until(() -> seen.contains(key), "the publisher discovers the endpoint announced after the ignored one");
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
    }

    @Test
    void javapsm_2_ignore_participant_ends_the_match_with_its_readers_and_keeps_it_ended() throws Exception {
        try (Rig rig = new Rig("IgnoreWireParticipant")) {
            rig.publisher.ignoreParticipant(rig.first.guidPrefix());
            assertEquals(1, rig.writer.matchedCount());
            expectOnlySecond(rig);
            // SEDP announces the reader again; the ignore holds
            announceAgain(rig, "IgnoreWireParticipant");
            assertEquals(1, rig.writer.matchedCount());
        }
    }

    @Test
    void javapsm_2_ignore_subscription_ends_the_match_with_one_reader() throws Exception {
        try (Rig rig = new Rig("IgnoreWireEndpoint")) {
            rig.publisher.ignoreEndpoint(rig.firstReader.guid);
            assertEquals(1, rig.writer.matchedCount());
            expectOnlySecond(rig);
            announceAgain(rig, "IgnoreWireEndpoint");
            assertEquals(1, rig.writer.matchedCount());
        }
    }

    @Test
    void javapsm_2_ignore_topic_ends_the_match_with_the_readers_of_the_topic() throws Exception {
        try (Rig rig = new Rig("IgnoreWireTopic")) {
            rig.publisher.ignoreTopic("IgnoreWireTopic");
            assertEquals(0, rig.writer.matchedCount());
            announceAgain(rig, "IgnoreWireTopic");
            assertEquals(0, rig.writer.matchedCount());
        }
    }
}
