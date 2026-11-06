// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// The Java publisher of tests/integration/tests/bindings_ffi.rs, for
// PUBLICATION_MATCHED (DDS 1.4 §2.2.4.1) with a reader of another process: a
// DataWriter of the Java PSM on its RTPS path (`zerodds.rtps.enable`) against
// a Rust subscriber.
//
//   java -Dzerodds.rtps.enable=true -cp <classes>:<this> FfiPub \
//       <domain> <topic> <type name> <samples>
//
// It waits for its match on PUBLICATION_MATCHED and prints `EVENT status
// matched total=<t> current=<c>` and `EVENT matched`; writes the samples of
// `@final struct FfiSample { unsigned long seq; string text; }` the scenario
// states (`seq`, `text` "ffi <seq, three digits>"), XCDR2 little endian as
// DDS-XTypes 1.3 §7.4.3.5 lays them out; prints `EVENT acknowledged` once the
// reader acknowledged them; its listener prints `EVENT listener total=<t>
// total_change=<t> current=<c> current_change=<c>` for each change. After
// standard input closed it ignores the subscriber's reader (DDS 1.4
// §2.2.2.2.1.17), which ends the match, prints `EVENT status unmatched
// total=<t> current=<c>` and `EVENT done`.

import java.io.IOException;
import java.io.InputStream;
import java.nio.charset.StandardCharsets;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicInteger;

import org.omg.dds.core.InstanceHandle;
import org.omg.dds.core.ServiceEnvironment;
import org.omg.dds.core.event.PublicationMatchedEvent;
import org.omg.dds.core.policy.PolicyFactory;
import org.omg.dds.core.status.PublicationMatchedStatus;
import org.omg.dds.domain.DomainParticipant;
import org.omg.dds.domain.DomainParticipantFactory;
import org.omg.dds.pub.DataWriter;
import org.omg.dds.pub.DataWriterAdapter;
import org.omg.dds.pub.DataWriterQos;
import org.omg.dds.pub.Publisher;
import org.omg.dds.topic.Topic;
import org.zerodds.ServiceEnvironmentImpl;
import org.zerodds.pub.PublisherImpl;

public final class FfiPub {
    private FfiPub() {}

    /** The body of sample {@code seq}, as the scenario states it. */
    private static byte[] body(int seq) {
        byte[] text = String.format("ffi %03d", seq).getBytes(StandardCharsets.US_ASCII);
        byte[] out = new byte[8 + text.length + 1];
        putLe(out, 0, seq);
        putLe(out, 4, text.length + 1);
        System.arraycopy(text, 0, out, 8, text.length);
        return out;
    }

    private static void putLe(byte[] out, int at, int value) {
        out[at] = (byte) value;
        out[at + 1] = (byte) (value >>> 8);
        out[at + 2] = (byte) (value >>> 16);
        out[at + 3] = (byte) (value >>> 24);
    }

    /** Waits until PUBLICATION_MATCHED's current_count is {@code wanted}, at most 10 s. */
    private static PublicationMatchedStatus awaitCurrent(DataWriter<byte[]> writer, int wanted)
            throws InterruptedException {
        long deadline = System.nanoTime() + 10_000_000_000L;
        PublicationMatchedStatus status = writer.getPublicationMatchedStatus();
        while (status.getCurrentCount() != wanted && System.nanoTime() < deadline) {
            Thread.sleep(10);
            status = writer.getPublicationMatchedStatus();
        }
        return status;
    }

    private static void drain(InputStream in) throws IOException {
        byte[] buf = new byte[256];
        while (in.read(buf) >= 0) {
            // Until the scenario closes standard input.
        }
    }

    public static void main(String[] args) throws Exception {
        if (args.length != 4) {
            System.err.println("usage: FfiPub <domain> <topic> <type name> <samples>");
            System.exit(2);
        }
        System.setProperty("zerodds.rtps.enable", "true");
        int domain = Integer.parseInt(args[0]);
        int samples = Integer.parseInt(args[3]);
        ServiceEnvironment env = ServiceEnvironmentImpl.getDefaultEnvironment();
        PolicyFactory pf = env.getSPI().getPolicyFactory();
        DomainParticipant participant =
                DomainParticipantFactory.getInstance(env).createParticipant(domain);
        int status = 0;
        try {
            Topic<byte[]> topic = participant.createTopic(args[1], byte[].class);
            // The writer is created disabled, so its listener is there before it matches.
            Publisher publisher = participant.createPublisher(participant.getDefaultPublisherQos()
                    .withPolicy(pf.EntityFactory().withAutoEnableCreatedEntities(false)));
            // RELIABLE and KEEP_ALL, as the subscriber's reader (DDS 1.4
            // §2.2.3.14, §2.2.3.18).
            DataWriterQos qos = publisher.getDefaultDataWriterQos()
                    .withPolicy(pf.Reliability().withReliable())
                    .withPolicy(pf.History().withKeepAll());
            DataWriter<byte[]> writer = ((PublisherImpl) publisher)
                    .createDataWriter(topic, new FfiSub.BodyTypeSupport(args[2]), qos);
            AtomicInteger heard = new AtomicInteger();
            writer.setListener(new DataWriterAdapter<byte[]>() {
                @Override
                public void onPublicationMatched(PublicationMatchedEvent<byte[]> event) {
                    PublicationMatchedStatus s = event.getStatus();
                    System.out.println("EVENT listener total=" + s.getTotalCount()
                            + " total_change=" + s.getTotalCountChange()
                            + " current=" + s.getCurrentCount()
                            + " current_change=" + s.getCurrentCountChange());
                    heard.incrementAndGet();
                }
            });
            writer.enable();
            PublicationMatchedStatus matched = awaitCurrent(writer, 1);
            if (matched.getCurrentCount() != 1) {
                System.out.println("EVENT unmatched");
                status = 1;
                return;
            }
            InstanceHandle subscription = matched.getLastSubscriptionHandle();
            System.out.println("EVENT status matched total=" + matched.getTotalCount()
                    + " current=" + matched.getCurrentCount());
            System.out.println("EVENT matched");
            for (int seq = 0; seq < samples; seq++) {
                writer.write(body(seq));
            }
            writer.waitForAcknowledgments(10, TimeUnit.SECONDS);
            System.out.println("EVENT acknowledged");
            drain(System.in);
            participant.ignoreSubscription(subscription);
            PublicationMatchedStatus ended = awaitCurrent(writer, 0);
            System.out.println("EVENT status unmatched total=" + ended.getTotalCount()
                    + " current=" + ended.getCurrentCount());
            // The listener runs on a thread of the RTPS participant: its call for the end of
            // the match comes after the counters changed.
            long deadline = System.nanoTime() + 10_000_000_000L;
            while (heard.get() < 2 && System.nanoTime() < deadline) {
                Thread.sleep(10);
            }
            System.out.println("EVENT done");
        } finally {
            participant.close();
            System.out.flush();
            System.exit(status);
        }
    }
}
