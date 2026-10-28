// SPDX-License-Identifier: Apache-2.0
package org.omg.dds;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.omg.dds.domain.DomainParticipant;
import org.omg.dds.domain.DomainParticipantFactory;
import org.omg.dds.pub.DataWriter;
import org.omg.dds.pub.Publisher;
import org.omg.dds.sub.DataReader;
import org.omg.dds.sub.Sample;
import org.omg.dds.sub.Subscriber;
import org.omg.dds.topic.Topic;
import org.zerodds.cdr.EndianMode;
import org.zerodds.cdr.ExtensibilityKind;
import org.zerodds.cdr.TopicTypeSupport;
import org.zerodds.internal.InProcessBus;
import org.zerodds.internal.Xcdr2Codec;

import java.nio.ByteBuffer;
import java.util.List;

import static org.junit.jupiter.api.Assertions.*;

class PubSubLoopbackTest {

    /** Sample user data type. */
    record Reading(int sensorId, double value) {}

    /** Manual TypeSupport (in production: emitted by idl-java codegen). */
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

    @BeforeEach
    void setUp() throws Exception {
        InProcessBus.instance().reset();
    }

    @AfterEach
    void tearDown() throws Exception {
        InProcessBus.instance().reset();
    }

    @Test
    void singleWriterSingleReaderRoundTrip() throws Exception {
        DomainParticipantFactory dpf = Qos.factory();
        DomainParticipant participant = dpf.createParticipant(0);
        Topic<Reading> topic = participant.createTopic("Readings", Reading.class);
        Publisher pub = participant.createPublisher();
        Subscriber sub = participant.createSubscriber();
        try (DataWriter<Reading> dw = Qos.writer(pub, topic, SUPPORT);
             DataReader<Reading> dr = Qos.reader(sub, topic, SUPPORT, Qos.readerKeepAll())) {

            dw.write(new Reading(1, 23.5));
            dw.write(new Reading(2, 24.0));

            List<Sample<Reading>> samples = Qos.take(dr);
            assertEquals(2, samples.size());
            assertEquals(1, samples.get(0).getData().sensorId());
            assertEquals(23.5, samples.get(0).getData().value(), 0.0);
            assertEquals(2, samples.get(1).getData().sensorId());
            assertEquals(24.0, samples.get(1).getData().value(), 0.0);
        }
    }

    @Test
    void readReturnsSamplesWithoutRemoving() throws Exception {
        DomainParticipantFactory dpf = Qos.factory();
        DomainParticipant participant = dpf.createParticipant(1);
        Topic<Reading> topic = participant.createTopic("ReadOnly", Reading.class);
        try (Publisher pub = participant.createPublisher();
             Subscriber sub = participant.createSubscriber();
             DataWriter<Reading> dw = Qos.writer(pub, topic, SUPPORT);
             DataReader<Reading> dr = Qos.reader(sub, topic, SUPPORT)) {

            dw.write(new Reading(7, 99.9));
            assertEquals(1, Qos.read(dr).size());
            assertEquals(1, Qos.read(dr).size()); // still there.
            assertEquals(1, Qos.take(dr).size()); // now removed.
            assertEquals(0, Qos.read(dr).size());
        }
    }

    @Test
    void multipleReadersAllReceiveSample() throws Exception {
        DomainParticipantFactory dpf = Qos.factory();
        DomainParticipant participant = dpf.createParticipant(2);
        Topic<Reading> topic = participant.createTopic("FanOut", Reading.class);
        try (Publisher pub = participant.createPublisher();
             Subscriber sub1 = participant.createSubscriber();
             Subscriber sub2 = participant.createSubscriber();
             DataWriter<Reading> dw = Qos.writer(pub, topic, SUPPORT);
             DataReader<Reading> dr1 = Qos.reader(sub1, topic, SUPPORT);
             DataReader<Reading> dr2 = Qos.reader(sub2, topic, SUPPORT)) {

            dw.write(new Reading(42, 1.0));
            assertEquals(1, Qos.take(dr1).size());
            assertEquals(1, Qos.take(dr2).size());
        }
    }

    @Test
    void cleanupRemovesSubscription() throws Exception {
        DomainParticipantFactory dpf = Qos.factory();
        DomainParticipant participant = dpf.createParticipant(3);
        Topic<Reading> topic = participant.createTopic("CleanupTest", Reading.class);
        Publisher pub = participant.createPublisher();
        Subscriber sub = participant.createSubscriber();
        DataWriter<Reading> dw = Qos.writer(pub, topic, SUPPORT);
        DataReader<Reading> dr = Qos.reader(sub, topic, SUPPORT);

        dr.close();
        dw.write(new Reading(1, 1.0));
        // Table 7.1: an operation on a closed entity is an AlreadyClosedException; the
        // closed reader is no longer a subscription of the topic, so the write reached no one.
        assertThrows(org.zerodds.core.AlreadyClosedException.class, () -> Qos.take(dr));
        assertEquals(0, dw.getMatchedSubscriptions().size());

        dw.close();
        sub.close();
        pub.close();
        topic.close();
    }
}
