// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// example_reliable app: argv = <peer-port> [N]. Submits N samples through
// the AsyncReliableWriter (org.zerodds.endpoint) -- the producer path is a
// pure BlockingQueue enqueue; a drain Thread owns the reliable sender state,
// sends WRITE_DATA over a real DatagramSocket, fires HEARTBEAT on a timer,
// and retransmits on ACKNACK until the send window drains. The peer
// (zerodds-endpoint-e2e's bind_reliable_peer/reliable_receive) injects loss
// and replies ACKNACK; loss recovery is proven when every sample lands
// gap-free and in order on the peer.

import java.net.InetAddress;

import org.zerodds.endpoint.AsyncReliableWriter;

public final class ExampleReliable {

    private ExampleReliable() {}

    public static void main(String[] args) throws Exception {
        int port = Integer.parseInt(args[0]);
        int n = args.length > 1 ? Integer.parseInt(args[1]) : 12;

        InetAddress host = InetAddress.getByName("127.0.0.1");
        AsyncReliableWriter writer = new AsyncReliableWriter(host, port);
        writer.start();

        for (int i = 0; i < n; i++) {
            // Sample i = its index as 4-byte little-endian (the peer/test decodes it).
            byte[] sample = new byte[] {
                (byte) (i & 0xFF), (byte) ((i >> 8) & 0xFF),
                (byte) ((i >> 16) & 0xFF), (byte) ((i >> 24) & 0xFF)
            };
            writer.submit(sample);
        }

        boolean ok = writer.finish(25_000);
        writer.close();
        if (ok) {
            System.err.println(
                "RELIABLE OK: all " + n + " acknowledged (delivered=" + writer.delivered() + ")");
            System.exit(0);
        } else {
            Exception err = writer.drainError();
            System.err.println("RELIABLE INCOMPLETE" + (err != null ? ": " + err : ""));
            System.exit(1);
        }
    }
}
