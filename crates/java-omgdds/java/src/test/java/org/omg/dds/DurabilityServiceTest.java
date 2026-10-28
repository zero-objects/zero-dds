// SPDX-License-Identifier: Apache-2.0
package org.omg.dds;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.omg.dds.core.policy.Durability;
import org.omg.dds.core.policy.History;
import org.omg.dds.pub.DataWriterQos;
import org.omg.dds.sub.DataReaderQos;
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
import org.zerodds.internal.DurabilityService;
import org.zerodds.internal.InProcessBus;

import java.nio.ByteBuffer;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;

/**
 * DDS 1.2 §7.1.3.4 — TRANSIENT and PERSISTENT samples survive their DataWriter
 * (via the durability service), unlike TRANSIENT_LOCAL which dies with the
 * writer. TRANSIENT data is dropped when the DomainParticipant closes;
 * PERSISTENT data outlives it.
 */
class DurabilityServiceTest {

    record Reading(int id, int seq) {}

    static final TopicTypeSupport<Reading> SUPPORT = new TopicTypeSupport<>() {
        public String getTypeName() { return "Reading"; }
        public void serialize(Reading r, ByteBuffer buf) {
            buf.putInt(r.id()); buf.putInt(r.seq());
        }
        public Reading deserialize(ByteBuffer buf) {
            return new Reading(buf.getInt(), buf.getInt());
        }
        public boolean isKeyed() { return true; }
        public byte[] keyHash(Reading r) {
            return ByteBuffer.allocate(4).putInt(r.id()).array();
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

    static int dom = 520;
    static int nextDom() { return dom++; }

    @BeforeEach void setUp() {
        InProcessBus.instance().reset();
        DurabilityService.instance().reset();
    }
    @AfterEach void tearDown() {
        InProcessBus.instance().reset();
        DurabilityService.instance().reset();
    }

    /**
     * A writer QoS with the given durability and HISTORY KEEP_LAST depth 1. Closing a writer
     * disposes its instances when autodispose_unregistered_instances is true (DDS 1.4
     * §2.2.3.21) and the service then forgets them; these tests are about data that outlives
     * the writer, so the writer does not dispose.
     */
    private static DataWriterQos durable(Durability.Kind kind) {
        return Qos.writer()
                .withPolicy(Qos.policies().Durability().withKind(kind))
                .withPolicy(Qos.policies().History().withKeepLast(1))
                .withPolicy(Qos.policies().WriterDataLifecycle()
                        .withAutDisposeUnregisteredInstances(false));
    }

    /** A reader QoS with the given durability and HISTORY KEEP_LAST depth 1. */
    private static DataReaderQos requested(Durability.Kind kind) {
        return Qos.reader()
                .withPolicy(Qos.policies().Durability().withKind(kind))
                .withPolicy(Qos.policies().History().withKeepLast(1));
    }

    @Test
    void transientSurvivesTheWriter() throws Exception {
        int d = nextDom();
        DomainParticipant p = Qos.factory().createParticipant(d);
        Topic<Reading> t = p.createTopic("TransientTopic", Reading.class);
        Publisher pub = p.createPublisher();

        DataWriter<Reading> dw = Qos.writer(pub, t, SUPPORT, durable(Durability.Kind.TRANSIENT));
        dw.write(new Reading(1, 0));
        dw.close(); // the writer dies; TRANSIENT data must survive it

        Subscriber sub = p.createSubscriber();
        DataReader<Reading> dr = Qos.reader(sub, t, SUPPORT, requested(Durability.Kind.TRANSIENT));
        List<Sample<Reading>> got = Qos.take(dr);
        assertEquals(1, got.size());
        assertEquals(0, got.get(0).getData().seq());

        dr.close();
        p.close();
    }

    @Test
    void persistentSurvivesTheWriter() throws Exception {
        int d = nextDom();
        DomainParticipant p = Qos.factory().createParticipant(d);
        Topic<Reading> t = p.createTopic("PersistentTopic", Reading.class);
        Publisher pub = p.createPublisher();

        DataWriter<Reading> dw = Qos.writer(pub, t, SUPPORT, durable(Durability.Kind.PERSISTENT));
        dw.write(new Reading(1, 0));
        dw.close();

        Subscriber sub = p.createSubscriber();
        DataReader<Reading> dr = Qos.reader(sub, t, SUPPORT, requested(Durability.Kind.PERSISTENT));
        List<Sample<Reading>> got = Qos.take(dr);
        assertEquals(1, got.size());
        assertEquals(0, got.get(0).getData().seq());

        dr.close();
        p.close();
    }

    @Test
    void transientIsClearedWhenTheParticipantCloses() throws Exception {
        int d = nextDom();
        DomainParticipant p = Qos.factory().createParticipant(d);
        Topic<Reading> t = p.createTopic("TransientClearedTopic", Reading.class);
        Publisher pub = p.createPublisher();
        DataWriter<Reading> dw = Qos.writer(pub, t, SUPPORT, durable(Durability.Kind.TRANSIENT));
        dw.write(new Reading(1, 0));
        dw.close();
        p.close(); // TRANSIENT data dies with the participant

        DomainParticipant p2 = Qos.factory().createParticipant(d);
        Topic<Reading> t2 = p2.createTopic("TransientClearedTopic", Reading.class);
        Subscriber sub = p2.createSubscriber();
        DataReader<Reading> dr = Qos.reader(sub, t2, SUPPORT, requested(Durability.Kind.TRANSIENT));
        assertEquals(0, Qos.take(dr).size(), "TRANSIENT data must not survive the participant");

        dr.close();
        p2.close();
    }

    @Test
    void persistentSurvivesTheParticipant() throws Exception {
        int d = nextDom();
        DomainParticipant p = Qos.factory().createParticipant(d);
        Topic<Reading> t = p.createTopic("PersistentSurvivesTopic", Reading.class);
        Publisher pub = p.createPublisher();
        DataWriter<Reading> dw = Qos.writer(pub, t, SUPPORT, durable(Durability.Kind.PERSISTENT));
        dw.write(new Reading(1, 0));
        dw.close();
        p.close();

        DomainParticipant p2 = Qos.factory().createParticipant(d);
        Topic<Reading> t2 = p2.createTopic("PersistentSurvivesTopic", Reading.class);
        Subscriber sub = p2.createSubscriber();
        DataReader<Reading> dr = Qos.reader(sub, t2, SUPPORT, requested(Durability.Kind.PERSISTENT));
        List<Sample<Reading>> got = Qos.take(dr);
        assertEquals(1, got.size(), "PERSISTENT data must survive the participant");
        assertEquals(0, got.get(0).getData().seq());

        dr.close();
        p2.close();
    }
}
