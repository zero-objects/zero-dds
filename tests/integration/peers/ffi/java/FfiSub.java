// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// The Java subscriber of tests/integration/tests/bindings_ffi.rs: the Java
// PSM of crates/java-omgdds (zerodds-ffi-loader-1.0 §3.2, an implementation
// in pure Java with its own RTPS stack, no libzerodds), its DataReader on the
// RTPS wire path (`zerodds.rtps.enable`):
//
//   java -Dzerodds.rtps.enable=true -cp <classes>:<this> FfiSub \
//       <domain> <topic> <type name> <samples>
//
// The protocol of sub.c: `EVENT matched`, `EVENT sample <hex>` per body,
// `EVENT received n=<count>`, one more take after standard input closed,
// `EVENT done`. Besides, for DDS 1.4 §2.2.4.1: the reader waits for its match
// on SUBSCRIPTION_MATCHED and prints `EVENT status matched total=<t>
// current=<c>`; its listener prints `EVENT listener total=<t> total_change=<t>
// current=<c> current_change=<c>` for each change; after standard input
// closed it ignores the publisher's writer (DDS 1.4 §2.2.2.2.1.16), which ends
// the match, and prints `EVENT status unmatched total=<t> current=<c>`.

import java.io.IOException;
import java.io.InputStream;

import org.omg.dds.core.InstanceHandle;
import org.omg.dds.core.ServiceEnvironment;
import org.omg.dds.core.event.SubscriptionMatchedEvent;
import org.omg.dds.core.policy.PolicyFactory;
import org.omg.dds.core.status.SubscriptionMatchedStatus;
import org.omg.dds.domain.DomainParticipant;
import org.omg.dds.domain.DomainParticipantFactory;
import org.omg.dds.sub.DataReader;
import org.omg.dds.sub.DataReaderAdapter;
import org.omg.dds.sub.DataReaderQos;
import org.omg.dds.sub.Sample;
import org.omg.dds.sub.Subscriber;
import org.omg.dds.topic.Topic;
import org.zerodds.ServiceEnvironmentImpl;
import org.zerodds.cdr.EndianMode;
import org.zerodds.cdr.ExtensibilityKind;
import org.zerodds.cdr.TopicTypeSupport;
import org.zerodds.sub.SubscriberImpl;

public final class FfiSub {
    private FfiSub() {}

    /**
     * The topic's type by name, its body passed through as octets: the
     * subscriber prints what it took, the scenario holds it to the layout of
     * {@code @final struct FfiSample { unsigned long seq; string text; }}.
     */
    static final class BodyTypeSupport implements TopicTypeSupport<byte[]> {
        private final String typeName;

        BodyTypeSupport(String typeName) {
            this.typeName = typeName;
        }

        @Override
        public String getTypeName() {
            return typeName;
        }

        @Override
        public boolean isKeyed() {
            return false;
        }

        @Override
        public ExtensibilityKind getExtensibility() {
            return ExtensibilityKind.FINAL;
        }

        @Override
        public byte[] encode(byte[] sample) {
            return sample.clone();
        }

        @Override
        public byte[] encode(byte[] sample, EndianMode endian) {
            return sample.clone();
        }

        @Override
        public byte[] decode(byte[] bytes) {
            return bytes.clone();
        }

        @Override
        public byte[] decode(byte[] bytes, int offset, int length) {
            byte[] out = new byte[length];
            System.arraycopy(bytes, offset, out, 0, length);
            return out;
        }
    }

    private static String hex(byte[] bytes) {
        StringBuilder out = new StringBuilder(bytes.length * 2);
        for (byte b : bytes) {
            out.append(Character.forDigit((b >> 4) & 0xf, 16));
            out.append(Character.forDigit(b & 0xf, 16));
        }
        return out.toString();
    }

    private static int takeAll(DataReader<byte[]> reader) throws IOException {
        int count = 0;
        Sample.Iterator<byte[]> walk = reader.take();
        try {
            while (walk.hasNext()) {
                Sample<byte[]> sample = walk.next();
                if (sample.getData() != null) {
                    System.out.println("EVENT sample " + hex(sample.getData()));
                    count += 1;
                }
            }
        } finally {
            walk.close();
        }
        return count;
    }

    /** Waits until SUBSCRIPTION_MATCHED's current_count is {@code wanted}, at most 10 s. */
    private static SubscriptionMatchedStatus awaitCurrent(DataReader<byte[]> reader, int wanted)
            throws InterruptedException {
        long deadline = System.nanoTime() + 10_000_000_000L;
        SubscriptionMatchedStatus status = reader.getSubscriptionMatchedStatus();
        while (status.getCurrentCount() != wanted && System.nanoTime() < deadline) {
            Thread.sleep(10);
            status = reader.getSubscriptionMatchedStatus();
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
            System.err.println("usage: FfiSub <domain> <topic> <type name> <samples>");
            System.exit(2);
        }
        System.setProperty("zerodds.rtps.enable", "true");
        int domain = Integer.parseInt(args[0]);
        int wanted = Integer.parseInt(args[3]);
        ServiceEnvironment env = ServiceEnvironmentImpl.getDefaultEnvironment();
        PolicyFactory pf = env.getSPI().getPolicyFactory();
        DomainParticipant participant =
                DomainParticipantFactory.getInstance(env).createParticipant(domain);
        int status = 0;
        try {
            Topic<byte[]> topic = participant.createTopic(args[1], byte[].class);
            // The reader is created disabled, so its listener is there before it matches.
            Subscriber subscriber = participant.createSubscriber(participant.getDefaultSubscriberQos()
                    .withPolicy(pf.EntityFactory().withAutoEnableCreatedEntities(false)));
            // RELIABLE and KEEP_ALL, as the publisher's writer (DDS 1.4
            // §2.2.3.14, §2.2.3.18).
            DataReaderQos qos = subscriber.getDefaultDataReaderQos()
                    .withPolicy(pf.Reliability().withReliable())
                    .withPolicy(pf.History().withKeepAll());
            DataReader<byte[]> reader = ((SubscriberImpl) subscriber)
                    .createDataReader(topic, new BodyTypeSupport(args[2]), qos);
            java.util.concurrent.atomic.AtomicInteger heard = new java.util.concurrent.atomic.AtomicInteger();
            reader.setListener(new DataReaderAdapter<byte[]>() {
                @Override
                public void onSubscriptionMatched(SubscriptionMatchedEvent<byte[]> event) {
                    SubscriptionMatchedStatus s = event.getStatus();
                    System.out.println("EVENT listener total=" + s.getTotalCount()
                            + " total_change=" + s.getTotalCountChange()
                            + " current=" + s.getCurrentCount()
                            + " current_change=" + s.getCurrentCountChange());
                    heard.incrementAndGet();
                }
            });
            reader.enable();
            SubscriptionMatchedStatus matched = awaitCurrent(reader, 1);
            if (matched.getCurrentCount() != 1) {
                System.out.println("EVENT unmatched");
                status = 1;
                return;
            }
            InstanceHandle publication = matched.getLastPublicationHandle();
            System.out.println("EVENT status matched total=" + matched.getTotalCount()
                    + " current=" + matched.getCurrentCount());
            System.out.println("EVENT matched");
            int taken = 0;
            long deadline = System.nanoTime() + 10_000_000_000L;
            while (taken < wanted && System.nanoTime() < deadline) {
                int now = takeAll(reader);
                taken += now;
                if (now == 0) {
                    Thread.sleep(5);
                }
            }
            System.out.println("EVENT received n=" + taken);
            drain(System.in);
            // A sample taken now was delivered twice.
            takeAll(reader);
            participant.ignorePublication(publication);
            SubscriptionMatchedStatus ended = awaitCurrent(reader, 0);
            System.out.println("EVENT status unmatched total=" + ended.getTotalCount()
                    + " current=" + ended.getCurrentCount());
            // The listener runs on a thread of the RTPS participant: its call for the end of
            // the match comes after the counters changed.
            deadline = System.nanoTime() + 10_000_000_000L;
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
