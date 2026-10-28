// SPDX-License-Identifier: Apache-2.0
package org.omg.dds;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.omg.dds.core.policy.Partition;
import org.omg.dds.core.event.PublicationMatchedEvent;
import org.omg.dds.core.status.PublicationMatchedStatus;
import org.omg.dds.core.event.DataAvailableEvent;
import org.omg.dds.core.event.SubscriptionMatchedEvent;
import org.omg.dds.core.status.SubscriptionMatchedStatus;
import org.omg.dds.domain.DomainParticipant;
import org.omg.dds.domain.DomainParticipantFactory;
import org.omg.dds.pub.DataWriter;
import org.omg.dds.pub.DataWriterAdapter;
import org.omg.dds.pub.Publisher;
import org.omg.dds.sub.DataReader;
import org.omg.dds.sub.DataReaderAdapter;
import org.omg.dds.sub.Subscriber;
import org.omg.dds.topic.Topic;
import org.zerodds.cdr.EndianMode;
import org.zerodds.cdr.ExtensibilityKind;
import org.zerodds.cdr.TopicTypeSupport;
import org.zerodds.internal.InProcessBus;
import org.zerodds.internal.Xcdr2Codec;

import java.nio.ByteBuffer;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertSame;
import org.omg.dds.sub.SubscriberQos;
import org.omg.dds.pub.PublisherQos;
import java.util.List;

/**
 * The Java side of the cross-language listener layer — vendor spec
 * {@code zerodds-listener-callbacks-1.1} §7.3: listeners are Java-heap objects
 * on the {@link InProcessBus} and sample arrival invokes the Java method
 * directly, with no C-FFI vtable in between.
 */
class ListenerTest {

    record Reading(int sensorId, double value) {}

    static final TopicTypeSupport<Reading> SUPPORT = new TopicTypeSupport<>() {
        @Override
        public String getTypeName() {
            return "Reading";
        }

        @Override
        public void serialize(Reading r, ByteBuffer buf) {
            Xcdr2Codec.writeInt(buf, r.sensorId());
            Xcdr2Codec.writeDouble(buf, r.value());
        }

        @Override
        public Reading deserialize(ByteBuffer buf) {
            int id = Xcdr2Codec.readInt(buf);
            double v = Xcdr2Codec.readDouble(buf);
            return new Reading(id, v);
        }

        @Override
        public boolean isKeyed() {
            return false;
        }

        @Override
        public ExtensibilityKind getExtensibility() {
            return ExtensibilityKind.APPENDABLE;
        }

        @Override
        public byte[] encode(Reading r) {
            ByteBuffer buf = ByteBuffer.allocate(256);
            serialize(r, buf);
            byte[] out = new byte[buf.position()];
            buf.flip();
            buf.get(out);
            return out;
        }

        @Override
        public byte[] encode(Reading r, EndianMode endian) {
            return encode(r);
        }

        @Override
        public Reading decode(byte[] bytes) {
            return decode(bytes, 0, bytes.length);
        }

        @Override
        public Reading decode(byte[] bytes, int offset, int length) {
            return deserialize(ByteBuffer.wrap(bytes, offset, length));
        }

    };

    /** Counts what it was handed; asserts nothing on its own. */
    static final class RecordingReaderListener extends DataReaderAdapter<Reading> {
        int dataAvailable;
        int matched;
        SubscriptionMatchedStatus lastMatched;
        DataReader<Reading> lastReader;

        @Override
        public void onDataAvailable(DataAvailableEvent<Reading> status) {
            dataAvailable++;
            lastReader = status.getSource();
        }

        @Override
        public void onSubscriptionMatched(SubscriptionMatchedEvent<Reading> status) {
            matched++;
            lastMatched = status.getStatus();
            lastReader = status.getSource();
        }
    }

    static final class RecordingWriterListener extends DataWriterAdapter<Reading> {
        int matched;
        PublicationMatchedStatus lastMatched;
        DataWriter<Reading> lastWriter;

        @Override
        public void onPublicationMatched(PublicationMatchedEvent<Reading> status) {
            matched++;
            lastMatched = status.getStatus();
            lastWriter = status.getSource();
        }
    }

    @BeforeEach
    void setUp() throws Exception {
        InProcessBus.instance().reset();
    }

    @AfterEach
    void tearDown() throws Exception {
        InProcessBus.instance().reset();
    }

    @Test
    void readerListenerFiresOnMatchAndOnEverySample() throws Exception {
        DomainParticipant participant =
                Qos.factory().createParticipant(30);
        Topic<Reading> topic = participant.createTopic("ListenerTopic", Reading.class);
        try (Publisher pub = participant.createPublisher();
             Subscriber sub = participant.createSubscriber();
             DataReader<Reading> dr = Qos.reader(sub, topic, SUPPORT, Qos.readerKeepAll())) {

            RecordingReaderListener l = new RecordingReaderListener();
            dr.setListener(l);
            assertSame(l, dr.getListener());
            assertEquals(0, l.matched, "no writer on the topic yet");

            try (DataWriter<Reading> dw = Qos.writer(pub, topic, SUPPORT)) {
                assertEquals(1, l.matched, "the writer joining is a SUBSCRIPTION_MATCHED event");
                assertEquals(1, l.lastMatched.getCurrentCount());
                assertEquals(1, l.lastMatched.getTotalCount());
                assertSame(dr, l.lastReader, "the callback is handed its own reader");

                dw.write(new Reading(1, 23.5));
                assertEquals(1, l.dataAvailable, "one sample, one callback");
                dw.write(new Reading(2, 24.0));
                assertEquals(2, l.dataAvailable, "the callback is per sample, not per burst");

                // §4.2: a read operation from inside the callback is allowed —
                // the samples the callbacks announced are the ones take() returns.
                assertEquals(2, Qos.take(dr).size());
            }
            // The writer going away unmatches: currentCount drops, totalCount does not.
            assertEquals(2, l.matched);
            assertEquals(0, l.lastMatched.getCurrentCount());
            assertEquals(1, l.lastMatched.getTotalCount());
        }
        participant.close();
    }

    @Test
    void writerListenerFiresOnPublicationMatched() throws Exception {
        DomainParticipant participant =
                Qos.factory().createParticipant(31);
        Topic<Reading> topic = participant.createTopic("WriterListenerTopic", Reading.class);
        try (Publisher pub = participant.createPublisher();
             Subscriber sub = participant.createSubscriber();
             DataWriter<Reading> dw = Qos.writer(pub, topic, SUPPORT)) {

            RecordingWriterListener l = new RecordingWriterListener();
            dw.setListener(l);
            assertEquals(0, l.matched, "no reader on the topic yet");

            DataReader<Reading> dr = Qos.reader(sub, topic, SUPPORT);
            assertEquals(1, l.matched);
            assertEquals(1, l.lastMatched.getCurrentCount());
            assertSame(dw, l.lastWriter);

            dr.close();
            assertEquals(2, l.matched);
            assertEquals(0, l.lastMatched.getCurrentCount());
            assertEquals(1, l.lastMatched.getTotalCount(), "total counts matches ever made");
        }
        participant.close();
    }

    @Test
    void clearingTheListenerStopsDelivery() throws Exception {
        DomainParticipant participant =
                Qos.factory().createParticipant(32);
        Topic<Reading> topic = participant.createTopic("ClearTopic", Reading.class);
        try (Publisher pub = participant.createPublisher();
             Subscriber sub = participant.createSubscriber();
             DataReader<Reading> dr = Qos.reader(sub, topic, SUPPORT, Qos.readerKeepAll());
             DataWriter<Reading> dw = Qos.writer(pub, topic, SUPPORT)) {

            RecordingReaderListener l = new RecordingReaderListener();
            dr.setListener(l);
            dw.write(new Reading(1, 1.0));
            assertEquals(1, l.dataAvailable);

            dr.setListener(null);
            assertNull(dr.getListener());
            dw.write(new Reading(2, 2.0));
            assertEquals(1, l.dataAvailable, "a cleared listener receives nothing further");
            assertEquals(2, Qos.take(dr).size(), "the samples still arrive at the reader");
        }
        participant.close();
    }

    @Test
    void nonOverlappingPartitionIsNotAMatch() throws Exception {
        // §2.2.3.13 — a writer the reader cannot communicate with is not a
        // SUBSCRIPTION_MATCHED event either.
        DomainParticipant participant =
                Qos.factory().createParticipant(33);
        Topic<Reading> topic = participant.createTopic("PartitionedTopic", Reading.class);
        // §7.1.3.9: PARTITION is a Publisher and a Subscriber policy.
        PublisherQos inA = Qos.publisher().withPolicy(Qos.policies().Partition().withName(List.of("A")));
        SubscriberQos inB = Qos.subscriber().withPolicy(Qos.policies().Partition().withName(List.of("B")));
        try (Publisher pub = participant.createPublisher(inA);
             Subscriber sub = participant.createSubscriber(inB);
             DataReader<Reading> dr = Qos.reader(sub, topic, SUPPORT, Qos.reader())) {

            RecordingReaderListener l = new RecordingReaderListener();
            dr.setListener(l);

            try (DataWriter<Reading> dw = Qos.writer(pub, topic, SUPPORT, Qos.writer())) {
                assertEquals(0, l.matched, "partition A and partition B do not match");
                dw.write(new Reading(9, 9.0));
                assertEquals(0, l.dataAvailable, "and no sample crosses either");
            }
        }
        participant.close();
    }

    @Test
    void statusIsReadableWithoutAListener() throws Exception {
        // DDS 1.4 §2.2.4.1: the change counters reset when the status is read
        // OR when it reaches a listener. With no listener bound, the
        // application's own read must still see the change.
        DomainParticipant participant =
                Qos.factory().createParticipant(34);
        Topic<Reading> topic = participant.createTopic("PollTopic", Reading.class);
        try (Publisher pub = participant.createPublisher();
             Subscriber sub = participant.createSubscriber();
             DataReader<Reading> dr = Qos.reader(sub, topic, SUPPORT);
             DataWriter<Reading> dw = Qos.writer(pub, topic, SUPPORT)) {

            SubscriptionMatchedStatus s = dr.getSubscriptionMatchedStatus();
            assertEquals(1, s.getCurrentCount());
            assertEquals(1, s.getCurrentCountChange(), "the first read reports the change");
            assertEquals(0, dr.getSubscriptionMatchedStatus().getCurrentCountChange(),
                    "and the second read reports none");

            PublicationMatchedStatus p = dw.getPublicationMatchedStatus();
            assertEquals(1, p.getCurrentCount());
            assertEquals(1, p.getTotalCount());
        }
        participant.close();
    }
}
