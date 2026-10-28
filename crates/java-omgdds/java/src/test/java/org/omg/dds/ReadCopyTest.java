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

import java.nio.ByteBuffer;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;

/**
 * DDS-Java-PSM 1.0 §7.7.3 — the read/take "copy" flavor deeply copies the
 * sample data into a fresh object, so mutating a returned sample does not
 * corrupt the reader's internal state.
 */
class ReadCopyTest {

    /** A mutable sample type (unlike a record) so aliasing is observable. */
    static final class Reading {
        int value;

        Reading() {}

        Reading(int value) {
            this.value = value;
        }
    }

    static final TopicTypeSupport<Reading> SUPPORT = new TopicTypeSupport<>() {
        public String getTypeName() { return "Reading"; }
        public void serialize(Reading r, ByteBuffer buf) { buf.putInt(r.value); }
        public Reading deserialize(ByteBuffer buf) { return new Reading(buf.getInt()); }

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

    @BeforeEach void setUp() { InProcessBus.instance().reset(); }
    @AfterEach void tearDown() { InProcessBus.instance().reset(); }

    @Test
    void readReturnsAFreshCopyNotTheReadersObject() throws Exception {
        DomainParticipant p =
                Qos.factory().createParticipant(530);
        Topic<Reading> t = p.createTopic("CopyTopic", Reading.class);
        try (Publisher pub = p.createPublisher();
             Subscriber sub = p.createSubscriber();
             DataReader<Reading> dr = Qos.reader(sub, t, SUPPORT, Qos.reader());
             DataWriter<Reading> dw = Qos.writer(pub, t, SUPPORT, Qos.writer())) {

            dw.write(new Reading(42));

            List<Sample<Reading>> first = Qos.read(dr);
            assertEquals(1, first.size());
            assertEquals(42, first.get(0).getData().value);
            first.get(0).getData().value = 99; // mutate what was handed back

            List<Sample<Reading>> second = Qos.read(dr);
            assertEquals(1, second.size());
            assertEquals(42, second.get(0).getData().value,
                    "a subsequent read must not observe the mutated value");
        }
        p.close();
    }

    @Test
    void takeReturnsAFreshCopyNotTheReadersObject() throws Exception {
        DomainParticipant p =
                Qos.factory().createParticipant(531);
        Topic<Reading> t = p.createTopic("CopyTakeTopic", Reading.class);
        try (Publisher pub = p.createPublisher();
             Subscriber sub = p.createSubscriber();
             DataReader<Reading> dr = Qos.reader(sub, t, SUPPORT, Qos.reader());
             DataWriter<Reading> dw = Qos.writer(pub, t, SUPPORT, Qos.writer())) {

            dw.write(new Reading(7));
            List<Sample<Reading>> got = Qos.take(dr);
            assertEquals(1, got.size());
            assertEquals(7, got.get(0).getData().value);
            got.get(0).getData().value = -1; // the returned object is the app's now
        }
        p.close();
    }
}
