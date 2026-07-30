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
import org.zerodds.DdsAsync;
import org.zerodds.internal.InProcessBus;
import org.zerodds.internal.Xcdr2Codec;

import java.nio.ByteBuffer;
import java.time.Duration;
import java.util.List;

import static org.junit.jupiter.api.Assertions.*;

/** Tests for the CompletableFuture async surface ({@link DdsAsync}). */
class DdsAsyncTest {

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
    void writeAsyncThenTakeAsyncRoundTrip() throws Exception {
        DomainParticipantFactory dpf = DomainParticipantFactory.getInstance();
        DomainParticipant participant = dpf.createParticipant(30);
        Topic<Reading> topic = participant.createTopic("AsyncReadings", Reading.class);
        try (Publisher pub = participant.createPublisher();
             Subscriber sub = participant.createSubscriber();
             DataWriter<Reading> dw = pub.createDataWriter(topic, SUPPORT);
             DataReader<Reading> dr = sub.createDataReader(topic, SUPPORT)) {

            // in-process delivery is synchronous, so after the write completes
            // the sample is queued; takeAsync returns it on the first poll.
            DdsAsync.writeAsync(dw, new Reading(1, 23.5)).get();
            DdsAsync.writeAsync(dw, new Reading(2, 24.0)).get();

            List<Sample<Reading>> samples = DdsAsync.takeAsync(dr, Duration.ofSeconds(2)).get();
            assertEquals(2, samples.size());
            assertEquals(1, samples.get(0).data().sensorId());
            assertEquals(23.5, samples.get(0).data().value(), 0.0);
            assertEquals(2, samples.get(1).data().sensorId());
        }
    }

    @Test
    void waitForSamplesAsyncTrueWhenDataPresent() throws Exception {
        DomainParticipantFactory dpf = DomainParticipantFactory.getInstance();
        DomainParticipant participant = dpf.createParticipant(31);
        Topic<Reading> topic = participant.createTopic("AsyncWait", Reading.class);
        try (Publisher pub = participant.createPublisher();
             Subscriber sub = participant.createSubscriber();
             DataWriter<Reading> dw = pub.createDataWriter(topic, SUPPORT);
             DataReader<Reading> dr = sub.createDataReader(topic, SUPPORT)) {

            dw.write(new Reading(7, 99.9));
            assertTrue(DdsAsync.waitForSamplesAsync(dr, Duration.ofSeconds(2)).get());
        }
    }

    @Test
    void waitForSamplesAsyncFalseOnTimeout() throws Exception {
        DomainParticipantFactory dpf = DomainParticipantFactory.getInstance();
        DomainParticipant participant = dpf.createParticipant(32);
        Topic<Reading> topic = participant.createTopic("AsyncEmpty", Reading.class);
        try (Subscriber sub = participant.createSubscriber();
             DataReader<Reading> dr = sub.createDataReader(topic, SUPPORT)) {

            assertFalse(DdsAsync.waitForSamplesAsync(dr, Duration.ofMillis(50)).get());
        }
    }
}
