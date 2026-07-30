// SPDX-License-Identifier: Apache-2.0
package org.zerodds.rtps;

import java.nio.ByteBuffer;
import java.nio.charset.StandardCharsets;

import org.omg.dds.domain.DomainParticipant;
import org.omg.dds.pub.DataWriter;
import org.omg.dds.pub.Publisher;
import org.omg.dds.sub.DataReader;
import org.omg.dds.sub.Sample;
import org.omg.dds.sub.Subscriber;
import org.omg.dds.topic.Topic;
import org.omg.dds.topic.TopicTypeSupport;

/**
 * Interop demo driving the <em>OMG DDS</em> {@code DataWriter}/{@code DataReader}
 * over the pure-Java RTPS wire path (enabled via {@code zerodds.rtps.enable}).
 * Proves the wired DDS entities — not just the raw {@link RtpsParticipant} —
 * interoperate with the Rust {@code hello_dds_*} examples on topic "Chatter",
 * type "zerodds::RawBytes".
 */
public final class DdsChatterInterop {
    private DdsChatterInterop() {}

    /** Raw-bytes type support advertising the Rust-compatible type name. */
    static final class RawBytesTypeSupport implements TopicTypeSupport<byte[]> {
        @Override
        public String getTypeName() {
            return "zerodds::RawBytes";
        }

        @Override
        public void serialize(byte[] value, ByteBuffer buf) {
            if (value != null) {
                buf.put(value);
            }
        }

        @Override
        public byte[] deserialize(ByteBuffer buf) {
            byte[] out = new byte[buf.remaining()];
            buf.get(out);
            return out;
        }
    }

    public static void main(String[] args) throws Exception {
        System.setProperty("zerodds.rtps.enable", "true");
        String mode = args.length > 0 ? args[0] : "write";
        int domain = args.length > 1 ? Integer.parseInt(args[1]) : 0;

        DomainParticipant participant = new DomainParticipant(domain);
        RawBytesTypeSupport ts = new RawBytesTypeSupport();
        Topic<byte[]> topic = participant.createTopic("Chatter", byte[].class);

        if (mode.equals("read")) {
            Subscriber sub = participant.createSubscriber();
            DataReader<byte[]> reader = sub.createDataReader(topic, ts);
            reader.enable();
            System.out.println("dds-java-reader (wired RTPS) on domain " + domain
                    + " topic 'Chatter' type 'zerodds::RawBytes' — Ctrl-C to stop");
            while (true) {
                for (Sample<byte[]> s : reader.take()) {
                    if (s.getData() != null) {
                        System.out.println("  <- "
                                + new String(s.getData(), StandardCharsets.UTF_8));
                    }
                }
                Thread.sleep(200);
            }
        } else {
            Publisher pub = participant.createPublisher();
            DataWriter<byte[]> writer = pub.createDataWriter(topic, ts);
            writer.enable();
            System.out.println("dds-java-writer (wired RTPS) on domain " + domain
                    + " topic 'Chatter' type 'zerodds::RawBytes'");
            Thread.sleep(4000); // allow SPDP+SEDP discovery
            int count = args.length > 2 ? Integer.parseInt(args[2]) : 20;
            for (int i = 0; i < count; i++) {
                String msg = "hello #" + i;
                writer.write(msg.getBytes(StandardCharsets.UTF_8));
                System.out.println("  -> " + msg);
                Thread.sleep(1000);
            }
        }
    }
}
