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
import org.omg.dds.topic.TopicTypeSupport;
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
    };

    @BeforeEach
    void setUp() {
        InProcessBus.instance().reset();
    }

    @AfterEach
    void tearDown() {
        InProcessBus.instance().reset();
    }

    @Test
    void singleWriterSingleReaderRoundTrip() {
        DomainParticipantFactory dpf = DomainParticipantFactory.getInstance();
        DomainParticipant participant = dpf.createParticipant(0);
        Topic<Reading> topic = participant.createTopic("Readings", Reading.class);
        Publisher pub = participant.createPublisher();
        Subscriber sub = participant.createSubscriber();
        try (DataWriter<Reading> dw = pub.createDataWriter(topic, SUPPORT);
             DataReader<Reading> dr = sub.createDataReader(topic, SUPPORT)) {

            dw.write(new Reading(1, 23.5));
            dw.write(new Reading(2, 24.0));

            List<Sample<Reading>> samples = dr.take();
            assertEquals(2, samples.size());
            assertEquals(1, samples.get(0).data().sensorId());
            assertEquals(23.5, samples.get(0).data().value(), 0.0);
            assertEquals(2, samples.get(1).data().sensorId());
            assertEquals(24.0, samples.get(1).data().value(), 0.0);
        }
    }

    @Test
    void readReturnsSamplesWithoutRemoving() {
        DomainParticipantFactory dpf = DomainParticipantFactory.getInstance();
        DomainParticipant participant = dpf.createParticipant(1);
        Topic<Reading> topic = participant.createTopic("ReadOnly", Reading.class);
        try (Publisher pub = participant.createPublisher();
             Subscriber sub = participant.createSubscriber();
             DataWriter<Reading> dw = pub.createDataWriter(topic, SUPPORT);
             DataReader<Reading> dr = sub.createDataReader(topic, SUPPORT)) {

            dw.write(new Reading(7, 99.9));
            assertEquals(1, dr.read().size());
            assertEquals(1, dr.read().size()); // still there.
            assertEquals(1, dr.take().size()); // now removed.
            assertEquals(0, dr.read().size());
        }
    }

    @Test
    void multipleReadersAllReceiveSample() {
        DomainParticipantFactory dpf = DomainParticipantFactory.getInstance();
        DomainParticipant participant = dpf.createParticipant(2);
        Topic<Reading> topic = participant.createTopic("FanOut", Reading.class);
        try (Publisher pub = participant.createPublisher();
             Subscriber sub1 = participant.createSubscriber();
             Subscriber sub2 = participant.createSubscriber();
             DataWriter<Reading> dw = pub.createDataWriter(topic, SUPPORT);
             DataReader<Reading> dr1 = sub1.createDataReader(topic, SUPPORT);
             DataReader<Reading> dr2 = sub2.createDataReader(topic, SUPPORT)) {

            dw.write(new Reading(42, 1.0));
            assertEquals(1, dr1.take().size());
            assertEquals(1, dr2.take().size());
        }
    }

    @Test
    void cleanupRemovesSubscription() {
        DomainParticipantFactory dpf = DomainParticipantFactory.getInstance();
        DomainParticipant participant = dpf.createParticipant(3);
        Topic<Reading> topic = participant.createTopic("CleanupTest", Reading.class);
        Publisher pub = participant.createPublisher();
        Subscriber sub = participant.createSubscriber();
        DataWriter<Reading> dw = pub.createDataWriter(topic, SUPPORT);
        DataReader<Reading> dr = sub.createDataReader(topic, SUPPORT);

        dr.close();
        dw.write(new Reading(1, 1.0));
        // Reader is closed → take() returns empty.
        assertEquals(0, dr.take().size());

        dw.close();
        sub.close();
        pub.close();
        topic.close();
    }
}
