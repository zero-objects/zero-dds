// SPDX-License-Identifier: Apache-2.0
package org.omg.dds;

import org.omg.dds.sub.InstanceState;
import org.zerodds.core.Values;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.omg.dds.core.Duration;
import org.omg.dds.core.policy.Deadline;
import org.omg.dds.core.policy.Durability;
import org.omg.dds.core.policy.History;
import org.omg.dds.core.policy.Liveliness;
import org.omg.dds.core.policy.Ownership;
import org.omg.dds.core.policy.OwnershipStrength;
import org.omg.dds.core.policy.Partition;
import org.omg.dds.core.policy.Reliability;
import org.omg.dds.core.status.LivelinessChangedStatus;
import org.omg.dds.core.status.RequestedDeadlineMissedStatus;
import org.omg.dds.domain.DomainParticipant;
import org.omg.dds.domain.DomainParticipantFactory;
import org.omg.dds.pub.DataWriter;
import org.omg.dds.pub.Publisher;
import org.omg.dds.pub.DataWriterQos;
import org.omg.dds.sub.DataReader;
import org.omg.dds.sub.Sample;
import org.omg.dds.sub.Subscriber;
import org.omg.dds.topic.ContentFilteredTopic;
import org.zerodds.topic.ContentFilteredTopicImpl;
import org.omg.dds.topic.Topic;
import org.zerodds.cdr.EndianMode;
import org.zerodds.cdr.ExtensibilityKind;
import org.zerodds.cdr.TopicTypeSupport;
import org.zerodds.internal.InProcessBus;

import java.nio.ByteBuffer;
import java.util.List;

import java.util.concurrent.TimeUnit;

import static org.junit.jupiter.api.Assertions.*;
import org.omg.dds.sub.DataReaderQos;
import org.zerodds.core.qos.QosCompatibility;

/**
 * Behavioral QoS + keyed-lifecycle regression for the org.omg.dds Java PSM
 * over the InProcessBus runtime. Each test OBSERVES the QoS effect, not just
 * a setter that doesn't throw (DDS-DCPS 1.4 §2.2.3 / §2.2.4).
 */
class QosLifecycleTest {

    /** Keyed type: struct Reading { @key int id; int seq; double value; }. */
    record Reading(int id, int seq, double value) {}

    /** Keyed TopicTypeSupport — key = id (DDS-DCPS 1.4 §2.2.1.2.2). */
    static final TopicTypeSupport<Reading> SUPPORT = new TopicTypeSupport<>() {
        public String getTypeName() { return "Reading"; }
        public void serialize(Reading r, ByteBuffer buf) {
            buf.putInt(r.id()); buf.putInt(r.seq()); buf.putDouble(r.value());
        }
        public Reading deserialize(ByteBuffer buf) {
            return new Reading(buf.getInt(), buf.getInt(), buf.getDouble());
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

    static int dom = 500;
    static int nextDom() { return dom++; }

    @BeforeEach void setUp() { InProcessBus.instance().reset(); }
    @AfterEach void tearDown() { InProcessBus.instance().reset(); }

    private record Rig(DomainParticipant p, Topic<Reading> t, Publisher pub, Subscriber sub) {}

    private Rig rig() {
        DomainParticipant p = Qos.factory().createParticipant(nextDom());
        Topic<Reading> t = p.createTopic("Reading", Reading.class);
        return new Rig(p, t, p.createPublisher(), p.createSubscriber());
    }

    @Test
    void historyKeepLastCapsPerInstance() throws Exception {
        Rig r = rig();
        DataReaderQos q = Qos.reader().withPolicy(Qos.policies().History().withKeepLast(1));
        DataWriterQos wq = Qos.writer().withPolicy(Qos.policies().History().withKeepLast(1));
        DataReader<Reading> dr = Qos.reader(r.sub, r.t, SUPPORT, q);
        DataWriter<Reading> dw = Qos.writer(r.pub, r.t, SUPPORT, wq);
        for (int i = 0; i < 5; i++) dw.write(new Reading(1, i, i));
        List<Sample<Reading>> got = Qos.read(dr);
        // KEEP_LAST(1) on a single key -> exactly the last sample retained.
        assertEquals(1, got.size());
        assertEquals(4, got.get(0).getData().seq());
    }

    @Test
    void historyKeepLastIsPerInstanceNotGlobal() throws Exception {
        Rig r = rig();
        DataReaderQos q = Qos.reader().withPolicy(Qos.policies().History().withKeepLast(1));
        DataWriterQos wq = Qos.writer().withPolicy(Qos.policies().History().withKeepLast(1));
        DataReader<Reading> dr = Qos.reader(r.sub, r.t, SUPPORT, q);
        DataWriter<Reading> dw = Qos.writer(r.pub, r.t, SUPPORT, wq);
        for (int i = 0; i < 3; i++) dw.write(new Reading(1, i, i));
        for (int i = 0; i < 3; i++) dw.write(new Reading(2, i, i));
        // Two keys, KEEP_LAST(1) each -> 2 samples total.
        assertEquals(2, Qos.read(dr).size());
    }

    @Test
    void transientLocalReplaysToLateJoiner() throws Exception {
        Rig r = rig();
        DataWriterQos tl = Qos.writer().withPolicy(Qos.policies().Durability().withTransientLocal())
                .withPolicy(Qos.policies().History().withKeepLast(1));
        DataReaderQos rtl = Qos.reader().withPolicy(Qos.policies().Durability().withTransientLocal())
                .withPolicy(Qos.policies().History().withKeepLast(1));
        DataWriter<Reading> dw = Qos.writer(r.pub, r.t, SUPPORT, tl);
        dw.write(new Reading(1, 0, 10.0));
        dw.write(new Reading(1, 1, 11.0)); // KEEP_LAST(1) -> only seq=1 retained
        // Late joiner created AFTER the writes.
        DataReader<Reading> dr = Qos.reader(r.sub, r.t, SUPPORT, rtl);
        List<Sample<Reading>> got = Qos.take(dr);
        assertEquals(1, got.size());
        assertEquals(1, got.get(0).getData().seq());
    }

    @Test
    void volatileDoesNotReplayToLateJoiner() throws Exception {
        Rig r = rig();
        DataWriterQos vol = Qos.writer().withPolicy(Qos.policies().Durability().withVolatile());
        DataWriter<Reading> dw = Qos.writer(r.pub, r.t, SUPPORT, vol);
        dw.write(new Reading(1, 0, 10.0));
        DataReader<Reading> dr = Qos.reader(r.sub, r.t, SUPPORT, Qos.reader());
        assertEquals(0, Qos.take(dr).size());
    }

    @Test
    void exclusiveOwnershipArbitratesToHighestStrength() throws Exception {
        Rig r = rig();
        DataReaderQos drq = Qos.readerKeepAll().withPolicy(Qos.policies().Ownership().withExclusive());
        DataReader<Reading> dr = Qos.reader(r.sub, r.t, SUPPORT, drq);
        DataWriterQos low = Qos.writer().withPolicy(Qos.policies().Ownership().withExclusive())
                .withPolicy(Qos.policies().OwnershipStrength().withValue(1));
        DataWriterQos high = Qos.writer().withPolicy(Qos.policies().Ownership().withExclusive())
                .withPolicy(Qos.policies().OwnershipStrength().withValue(10));
        DataWriter<Reading> dwLow = Qos.writer(r.pub, r.t, SUPPORT, low);
        DataWriter<Reading> dwHigh = Qos.writer(r.pub, r.t, SUPPORT, high);
        dwHigh.write(new Reading(1, 0, 100.0)); // owner = high (strength 10)
        dwLow.write(new Reading(1, 1, 1.0));     // filtered out (lower strength)
        dwHigh.write(new Reading(1, 2, 102.0));  // owner accepted
        List<Sample<Reading>> got = Qos.take(dr);
        // Only the owner's two samples delivered.
        assertEquals(2, got.size());
        assertTrue(got.stream().allMatch(s -> s.getData().value() >= 100.0));
    }

    @Test
    void sharedOwnershipDeliversFromAllWriters() throws Exception {
        Rig r = rig();
        DataReader<Reading> dr = Qos.reader(r.sub, r.t, SUPPORT, Qos.readerKeepAll());
        DataWriter<Reading> a = Qos.writer(r.pub, r.t, SUPPORT, Qos.writer());
        DataWriter<Reading> b = Qos.writer(r.pub, r.t, SUPPORT, Qos.writer());
        a.write(new Reading(1, 0, 1.0));
        b.write(new Reading(1, 1, 2.0));
        assertEquals(2, Qos.take(dr).size());
    }

    @Test
    void partitionIsolatesNonOverlappingEndpoints() throws Exception {
        Rig r = rig();
        // §7.1.3.9: PARTITION lives on the Publisher and the Subscriber.
        Publisher pubA = r.p.createPublisher(Qos.publisher()
                .withPolicy(Qos.policies().Partition().withName(List.of("A"))));
        Subscriber subB = r.p.createSubscriber(Qos.subscriber()
                .withPolicy(Qos.policies().Partition().withName(List.of("B"))));
        DataReader<Reading> dr = Qos.reader(subB, r.t, SUPPORT, Qos.reader());
        DataWriter<Reading> dw = Qos.writer(pubA, r.t, SUPPORT, Qos.writer());
        dw.write(new Reading(1, 0, 1.0));
        assertEquals(0, Qos.take(dr).size(), "A and B partitions must not communicate");
    }

    @Test
    void partitionMatchesOnOverlap() throws Exception {
        Rig r = rig();
        Publisher pubA = r.p.createPublisher(Qos.publisher()
                .withPolicy(Qos.policies().Partition().withName(List.of("A", "X"))));
        Subscriber subA = r.p.createSubscriber(Qos.subscriber()
                .withPolicy(Qos.policies().Partition().withName(List.of("X"))));
        DataReader<Reading> dr = Qos.reader(subA, r.t, SUPPORT, Qos.reader());
        DataWriter<Reading> dw = Qos.writer(pubA, r.t, SUPPORT, Qos.writer());
        dw.write(new Reading(1, 0, 1.0));
        assertEquals(1, Qos.take(dr).size());
    }

    @Test
    void partitionWildcardMatches() throws Exception {
        Rig r = rig();
        Publisher pub = r.p.createPublisher(Qos.publisher()
                .withPolicy(Qos.policies().Partition().withName(List.of("sensor.temp"))));
        Subscriber sub = r.p.createSubscriber(Qos.subscriber()
                .withPolicy(Qos.policies().Partition().withName(List.of("sensor.*"))));
        DataReader<Reading> dr = Qos.reader(sub, r.t, SUPPORT, Qos.reader());
        DataWriter<Reading> dw = Qos.writer(pub, r.t, SUPPORT, Qos.writer());
        dw.write(new Reading(1, 0, 1.0));
        assertEquals(1, Qos.take(dr).size());
    }

    @Test
    void contentFilteredTopicDropsNonMatching() throws Exception {
        Rig r = rig();
        ContentFilteredTopic<Reading> cft = new ContentFilteredTopicImpl<>(
                Values.getDefaultEnvironment(), "HighValue", r.t, "value > 50.0",
                java.util.List.of(), rd -> rd.value() > 50.0);
        DataReader<Reading> dr = Qos.reader(r.sub, cft, SUPPORT, Qos.reader());
        DataWriter<Reading> dw = Qos.writer(r.pub, r.t, SUPPORT, Qos.writer());
        dw.write(new Reading(1, 0, 10.0));   // dropped
        dw.write(new Reading(2, 1, 99.0));   // kept
        dw.write(new Reading(3, 2, 5.0));    // dropped
        List<Sample<Reading>> got = Qos.take(dr);
        assertEquals(1, got.size());
        assertEquals(99.0, got.get(0).getData().value(), 0.0);
    }

    @Test
    void disposeMarksInstanceNotAliveDisposed() throws Exception {
        Rig r = rig();
        DataReader<Reading> dr = Qos.reader(r.sub, r.t, SUPPORT, Qos.reader());
        DataWriter<Reading> dw = Qos.writer(r.pub, r.t, SUPPORT, Qos.writer());
        dw.write(new Reading(7, 0, 1.0));
        Qos.dispose(dw, new Reading(7, 0, 0.0));
        List<Sample<Reading>> got = Qos.take(dr);
        assertTrue(got.stream().anyMatch(
                s -> s.getInstanceState() == InstanceState.NOT_ALIVE_DISPOSED),
                "dispose -> NOT_ALIVE_DISPOSED");
    }

    /** The default writer QoS with WRITER_DATA_LIFECYCLE autodispose_unregistered_instances false. */
    private static DataWriterQos withoutAutodispose() {
        return Qos.writer().withPolicy(
                Qos.pf().WriterDataLifecycle().withAutDisposeUnregisteredInstances(false));
    }

    @Test
    void unregisterMarksInstanceNotAliveNoWriters() throws Exception {
        // With autodispose_unregistered_instances true (the default) the unregister also
        // disposes; NOT_ALIVE_NO_WRITERS is what an unregister without dispose leaves
        // (DDS 1.4 §2.2.3.21). QosWriterDataLifecycleTest checks the default.
        Rig r = rig();
        DataReader<Reading> dr = Qos.reader(r.sub, r.t, SUPPORT, Qos.reader());
        DataWriter<Reading> dw = Qos.writer(r.pub, r.t, SUPPORT, withoutAutodispose());
        dw.write(new Reading(7, 0, 1.0));
        Qos.unregister(dw, new Reading(7, 0, 0.0));
        List<Sample<Reading>> got = Qos.take(dr);
        assertTrue(got.stream().anyMatch(
                s -> s.getInstanceState() == InstanceState.NOT_ALIVE_NO_WRITERS),
                "unregister -> NOT_ALIVE_NO_WRITERS");
    }

    @Test
    void closingOneWriterDoesNotMarkNoWritersWhileAnotherWrites() throws Exception {
        // DDS 1.2 §7.1.2.4.2.7: NOT_ALIVE_NO_WRITERS only once *no* writer is
        // still writing the instance.
        Rig r = rig();
        DataReader<Reading> dr = Qos.reader(r.sub, r.t, SUPPORT, Qos.reader());
        DataWriter<Reading> w1 = Qos.writer(r.pub, r.t, SUPPORT, withoutAutodispose());
        DataWriter<Reading> w2 = Qos.writer(r.pub, r.t, SUPPORT, withoutAutodispose());
        w1.write(new Reading(7, 0, 1.0));
        w2.write(new Reading(7, 1, 2.0));

        w1.close();
        List<Sample<Reading>> afterFirstClose = Qos.read(dr);
        for (Sample<Reading> s : afterFirstClose) {
            assertNotEquals(InstanceState.NOT_ALIVE_NO_WRITERS, s.getInstanceState(),
                    "w2 still writes the instance: no NOT_ALIVE_NO_WRITERS");
        }

        w2.close();
        List<Sample<Reading>> afterSecondClose = Qos.take(dr);
        assertTrue(afterSecondClose.stream().anyMatch(
                s -> s.getInstanceState() == InstanceState.NOT_ALIVE_NO_WRITERS),
                "with no writer left the instance is NOT_ALIVE_NO_WRITERS");
    }

    @Test
    void registerAndLookupInstance() throws Exception {
        Rig r = rig();
        DataWriter<Reading> dw = Qos.writer(r.pub, r.t, SUPPORT, Qos.writer());
        var h = dw.registerInstance(new Reading(42, 0, 0.0));
        assertFalse(h.isNil());
        assertEquals(h, dw.lookupInstance(new Reading(42, 9, 9.0)));
        assertTrue(dw.lookupInstance(new Reading(99, 0, 0.0)).isNil());
    }

    @Test
    void disposedThenWrittenIsRebornAlive() throws Exception {
        Rig r = rig();
        DataReader<Reading> dr = Qos.reader(r.sub, r.t, SUPPORT, Qos.reader());
        DataWriter<Reading> dw = Qos.writer(r.pub, r.t, SUPPORT, Qos.writer());
        dw.write(new Reading(7, 0, 1.0));
        Qos.dispose(dw, new Reading(7, 0, 0.0));
        dw.write(new Reading(7, 1, 2.0)); // resurrect
        List<Sample<Reading>> got = Qos.take(dr);
        Sample<Reading> last = got.get(got.size() - 1);
        assertEquals(InstanceState.ALIVE, last.getInstanceState());
        assertEquals(2.0, last.getData().value(), 0.0);
    }

    @Test
    void requestedDeadlineMissedIsRaised() throws Exception {
        Rig r = rig();
        DataReaderQos q = Qos.reader().withPolicy(Qos.policies().Deadline().withPeriod(50, TimeUnit.MILLISECONDS));
        DataWriterQos wq = Qos.writer().withPolicy(Qos.policies().Deadline().withPeriod(50, TimeUnit.MILLISECONDS));
        DataReader<Reading> dr = Qos.reader(r.sub, r.t, SUPPORT, q);
        DataWriter<Reading> dw = Qos.writer(r.pub, r.t, SUPPORT, wq);
        dw.write(new Reading(1, 0, 1.0));
        RequestedDeadlineMissedStatus before = dr.getRequestedDeadlineMissedStatus();
        assertEquals(0, before.getTotalCount());
        Thread.sleep(140); // > 2 deadline periods with no new sample
        RequestedDeadlineMissedStatus after = dr.getRequestedDeadlineMissedStatus();
        assertTrue(after.getTotalCount() >= 1, "deadline should be missed at least once");
        assertTrue(after.getTotalCountChange() >= 1);
    }

    @Test
    void infiniteDeadlineNeverMisses() throws Exception {
        Rig r = rig();
        DataReader<Reading> dr = Qos.reader(r.sub, r.t, SUPPORT, Qos.reader());
        DataWriter<Reading> dw = Qos.writer(r.pub, r.t, SUPPORT, Qos.writer());
        dw.write(new Reading(1, 0, 1.0));
        Thread.sleep(60);
        assertEquals(0, dr.getRequestedDeadlineMissedStatus().getTotalCount());
    }

    @Test
    void livelinessChangedReflectsWriterPresence() throws Exception {
        // §7.1.3.7: a MANUAL_BY_TOPIC writer that stops writing is no longer
        // alive once its lease duration elapses.
        Rig r = rig();
        DataReaderQos readerQos = Qos.reader().withPolicy(
                Qos.policies().Liveliness().withManualByTopic()
                        .withLeaseDuration(40, TimeUnit.MILLISECONDS));
        DataWriterQos writerQos = Qos.writer().withPolicy(
                Qos.policies().Liveliness().withManualByTopic()
                        .withLeaseDuration(40, TimeUnit.MILLISECONDS));
        DataReader<Reading> dr = Qos.reader(r.sub, r.t, SUPPORT, readerQos);
        DataWriter<Reading> dw = Qos.writer(r.pub, r.t, SUPPORT, writerQos);
        LivelinessChangedStatus alive = dr.getLivelinessChangedStatus();
        assertEquals(1, alive.getAliveCount());
        assertEquals(0, alive.getNotAliveCount());
        Thread.sleep(200); // the writer's lease expires without a status poll
        LivelinessChangedStatus lost = dr.getLivelinessChangedStatus();
        assertEquals(0, lost.getAliveCount());
        assertEquals(1, lost.getNotAliveCount());
    }

    @Test
    void rxoIncompatibleReliabilityDetected() throws Exception {
        // §7.1.3.6: a RELIABLE reader is not served by a BEST_EFFORT writer.
        DataReaderQos reader = Qos.reader()
                .withPolicy(Qos.policies().DataReaderReliability().withReliable());
        DataWriterQos writer = Qos.writer()
                .withPolicy(Qos.policies().DataWriterReliability().withBestEffort());
        assertFalse(QosCompatibility.isCompatible(
                QosCompatibility.policiesOf(reader), QosCompatibility.policiesOf(writer)));
    }

    @Test
    void rxoDeadlineCompatibility() throws Exception {
        // requested(reader) must be >= offered(writer)
        DataReaderQos reader = Qos.reader()
                .withPolicy(Qos.policies().Deadline().withPeriod(100, TimeUnit.MILLISECONDS));
        DataWriterQos fastWriter = Qos.writer()
                .withPolicy(Qos.policies().Deadline().withPeriod(50, TimeUnit.MILLISECONDS));
        DataWriterQos slowWriter = Qos.writer()
                .withPolicy(Qos.policies().Deadline().withPeriod(200, TimeUnit.MILLISECONDS));
        assertTrue(QosCompatibility.isCompatible(
                QosCompatibility.policiesOf(reader), QosCompatibility.policiesOf(fastWriter)));
        assertFalse(QosCompatibility.isCompatible(
                QosCompatibility.policiesOf(reader), QosCompatibility.policiesOf(slowWriter)));
    }

    @Test
    void ownershipKindMustMatch() throws Exception {
        // DDS 1.4 §2.2.3.9: the kinds must be equal, in either direction (the other cases are
        // in QosRequestedOfferedTest).
        Ownership exclusive = Qos.policies().Ownership().withExclusive();
        Ownership shared = Qos.policies().Ownership().withShared();
        assertFalse(exclusive.compareTo(shared) <= 0);
        assertTrue(exclusive.compareTo(Qos.policies().Ownership().withExclusive()) <= 0);
        assertFalse(QosCompatibility.isCompatible(
                QosCompatibility.policiesOf(Qos.reader().withPolicy(shared)),
                QosCompatibility.policiesOf(Qos.writer().withPolicy(exclusive))));
        assertFalse(QosCompatibility.isCompatible(
                QosCompatibility.policiesOf(Qos.reader().withPolicy(exclusive)),
                QosCompatibility.policiesOf(Qos.writer().withPolicy(shared))));
        assertTrue(QosCompatibility.isCompatible(
                QosCompatibility.policiesOf(Qos.reader().withPolicy(exclusive)),
                QosCompatibility.policiesOf(Qos.writer().withPolicy(exclusive))));
    }

    @Test
    void livelinessRxoKindAndLease() throws Exception {
        // §7.1.3.7: the offered kind must be at least the requested one and, for
        // equal kinds, the offered lease must not exceed the requested one.
        Liveliness reqAuto = Qos.policies().Liveliness().withAutomatic()
                .withLeaseDuration(100, TimeUnit.MILLISECONDS);
        Liveliness offManualTopic = Qos.policies().Liveliness().withManualByTopic()
                .withLeaseDuration(50, TimeUnit.MILLISECONDS);
        assertTrue(QosCompatibility.isCompatible(
                QosCompatibility.policiesOf(Qos.reader().withPolicy(reqAuto)),
                QosCompatibility.policiesOf(Qos.writer().withPolicy(offManualTopic))));

        Liveliness reqManualTopic = Qos.policies().Liveliness().withManualByTopic()
                .withLeaseDuration(100, TimeUnit.MILLISECONDS);
        Liveliness offAuto = Qos.policies().Liveliness().withAutomatic()
                .withLeaseDuration(50, TimeUnit.MILLISECONDS);
        assertFalse(QosCompatibility.isCompatible(
                QosCompatibility.policiesOf(Qos.reader().withPolicy(reqManualTopic)),
                QosCompatibility.policiesOf(Qos.writer().withPolicy(offAuto))));
    }
}
