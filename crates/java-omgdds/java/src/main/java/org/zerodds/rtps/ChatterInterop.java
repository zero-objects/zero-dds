// SPDX-License-Identifier: Apache-2.0
package org.zerodds.rtps;

import java.nio.charset.StandardCharsets;

/**
 * Cross-implementation interop demo for the pure-Java RTPS stack. Publishes /
 * subscribes on topic "Chatter" with type "zerodds::RawBytes" — matching the
 * Rust {@code hello_dds_publisher}/{@code hello_dds_subscriber} examples — so a
 * Java writer feeds a real Rust subscriber over UDP (and vice versa).
 *
 * <pre>
 *   # Java writer -> Rust subscriber
 *   java -cp target/classes org.zerodds.rtps.ChatterInterop write
 *   cargo run -p zerodds-dcps --example hello_dds_subscriber
 *
 *   # Rust publisher -> Java reader
 *   cargo run -p zerodds-dcps --example hello_dds_publisher
 *   java -cp target/classes org.zerodds.rtps.ChatterInterop read
 * </pre>
 */
public final class ChatterInterop {
    private ChatterInterop() {}

    public static void main(String[] args) throws Exception {
        String mode = args.length > 0 ? args[0] : "write";
        int domain = args.length > 1 ? Integer.parseInt(args[1]) : 0;
        String topic = args.length > 2 ? args[2] : "Chatter";
        String type = args.length > 3 ? args[3] : "zerodds::RawBytes";

        RtpsParticipant participant = RtpsParticipant.get(domain);

        if (mode.equals("read")) {
            participant.createReader(topic, type, false, body -> {
                String s = new String(body, StandardCharsets.UTF_8);
                System.out.println("  <- " + s + "  [" + body.length + " bytes]");
            });
            System.out.println("java-reader on domain " + domain + " topic '" + topic
                    + "' type '" + type + "' — Ctrl-C to stop");
            Thread.currentThread().join();
        } else {
            // RawBytes = identity payload with an XCDR1 CDR_LE encapsulation header.
            RtpsParticipant.WireWriter w = participant.createWriter(topic, type,
                    Rtps.ENCAP_CDR_LE, new byte[0], false, true);
            System.out.println("java-writer on domain " + domain + " topic '" + topic
                    + "' type '" + type + "' — waiting for discovery...");
            long deadline = System.currentTimeMillis() + 8000;
            while (w.matchedCount() == 0 && System.currentTimeMillis() < deadline) {
                Thread.sleep(100);
            }
            System.out.println("matched readers: " + w.matchedCount());
            int count = args.length > 4 ? Integer.parseInt(args[4]) : 20;
            for (int i = 0; i < count; i++) {
                String msg = "hello #" + i;
                w.write(msg.getBytes(StandardCharsets.UTF_8));
                System.out.println("  -> " + msg + " (matched=" + w.matchedCount() + ")");
                Thread.sleep(1000);
            }
        }
    }
}
