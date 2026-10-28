// SPDX-License-Identifier: Apache-2.0
package org.zerodds.rtps;

import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.net.DatagramPacket;
import java.net.DatagramSocket;
import java.net.InetAddress;
import java.net.InetSocketAddress;
import java.net.SocketAddress;
import java.net.SocketTimeoutException;
import java.nio.charset.StandardCharsets;
import java.util.Arrays;

import org.junit.jupiter.api.Test;
import org.zerodds.testing.TestDomains;

/**
 * A reliable writer answers an ACKNACK at the locator of the reader proxy, not at the source of
 * the ACKNACK (DDSI-RTPS 2.5 §8.4.9.2.8, the StatefulWriter answers through the ReaderProxy that
 * discovery gave it; §8.3.4.1: the reply locator a receiver takes from the source of a message has
 * the port LOCATOR_PORT_INVALID). A reader that sends from another socket than the one it
 * announced, as the Rust runtime does, got the repair at its send socket and never saw it: measured
 * with {@code crates/rpc-rust-e2e/tests/java_peer.rs}, where the request of a Java client written
 * before the Rust service had matched the Java writer was asked for again by every ACKNACK and
 * repaired to the ACKNACK's source port each time.
 *
 * <p>The reader here is a participant that is closed after the match: the writer keeps its proxy,
 * the test takes over the announced port, and answers for the reader from another socket.
 */
class WriterRepairLocatorTest {

    private static void waitFor(java.util.function.BooleanSupplier condition, String what)
            throws InterruptedException {
        long end = System.currentTimeMillis() + 20_000;
        while (!condition.getAsBoolean() && System.currentTimeMillis() < end) {
            Thread.sleep(50);
        }
        assertTrue(condition.getAsBoolean(), what);
    }

    /** The first datagram on {@code socket} within {@code millis} with a DATA of {@code sequence}, or null. */
    private static DatagramPacket dataOf(DatagramSocket socket, long sequence, long millis) throws Exception {
        long end = System.currentTimeMillis() + millis;
        while (System.currentTimeMillis() < end) {
            byte[] buffer = new byte[65536];
            DatagramPacket packet = new DatagramPacket(buffer, buffer.length);
            socket.setSoTimeout((int) Math.max(1, end - System.currentTimeMillis()));
            try {
                socket.receive(packet);
            } catch (SocketTimeoutException quiet) {
                return null;
            }
            RtpsMessage.Parsed parsed = RtpsMessage.decode(packet.getData(), packet.getLength());
            for (RtpsMessage.Data data : parsed.data) {
                if (data.writerSn == sequence) {
                    return packet;
                }
            }
        }
        return null;
    }

    @Test
    void theRepairGoesToTheLocatorTheReaderAnnouncedAndNotToTheSourceOfItsAckNack() throws Exception {
        try (TestDomains.Lease lease = TestDomains.lease()) {
            RtpsParticipant publisher = RtpsParticipant.newParticipant(lease.id());
            RtpsParticipant staying = RtpsParticipant.newParticipant(lease.id());
            RtpsParticipant leaving = RtpsParticipant.newParticipant(lease.id());
            DatagramSocket announced = null;
            DatagramSocket source = null;
            try {
                staying.createReader("RepairLocator", "zerodds::RawBytes", false, body -> { });
                leaving.createReader("RepairLocator", "zerodds::RawBytes", false, body -> { });
                RtpsParticipant.WireWriter writer = publisher.createWriter("RepairLocator",
                        "zerodds::RawBytes", Rtps.ENCAP_CDR_LE, new byte[0], false, true);
                waitFor(() -> writer.matchedCount() == 2, "the writer meets both readers over SEDP");
                RtpsParticipant.MatchedReader proxy = null;
                for (RtpsParticipant.MatchedReader reader : writer.matched.values()) {
                    if (Arrays.equals(reader.participantPrefix, leaving.guidPrefix())) {
                        proxy = reader;
                    }
                }
                assertNotNull(proxy, "the proxy of the reader that leaves");
                byte[] readerEntity = proxy.readerEntityId;
                int port = (int) proxy.locator.port;
                leaving.close();
                announced = new DatagramSocket(null);
                announced.setReuseAddress(true);
                announced.bind(new InetSocketAddress(port));
                source = new DatagramSocket(new InetSocketAddress(InetAddress.getByAddress(proxy.locator.ipv4()), 0));

                writer.write("asked for again".getBytes(StandardCharsets.UTF_8));
                DatagramPacket first = dataOf(announced, 1, 5_000);
                assertNotNull(first, "the first DATA goes to the announced locator");
                SocketAddress publisherAt = first.getSocketAddress();

                byte[] ackNack = new RtpsMessage.Builder(leaving.guidPrefix())
                        .infoDestination(publisher.guidPrefix())
                        .ackNack(readerEntity, writer.entityId, 1, 1, 77, false)
                        .toBytes();
                source.send(new DatagramPacket(ackNack, ackNack.length, publisherAt));

                assertNotNull(dataOf(announced, 1, 3_000),
                        "the repair of SN 1 reaches the locator the reader announced");
                assertNull(dataOf(source, 1, 1_000),
                        "no repair goes to the port the ACKNACK came from");
            } finally {
                if (announced != null) {
                    announced.close();
                }
                if (source != null) {
                    source.close();
                }
                publisher.close();
                staying.close();
            }
        }
    }
}
