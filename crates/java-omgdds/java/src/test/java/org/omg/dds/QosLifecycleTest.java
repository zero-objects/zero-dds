// SPDX-License-Identifier: Apache-2.0
package org.omg.dds;

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
import org.omg.dds.core.policy.QosProfile;
import org.omg.dds.core.policy.Reliability;
import org.omg.dds.core.status.LivelinessChangedStatus;
import org.omg.dds.core.status.RequestedDeadlineMissedStatus;
import org.omg.dds.domain.DomainParticipant;
import org.omg.dds.domain.DomainParticipantFactory;
import org.omg.dds.pub.DataWriter;
import org.omg.dds.pub.Publisher;
import org.omg.dds.sub.DataReader;
import org.omg.dds.sub.Sample;
import org.omg.dds.sub.Subscriber;
import org.omg.dds.topic.ContentFilteredTopic;
import org.omg.dds.topic.Topic;
import org.omg.dds.topic.TopicTypeSupport;
import org.zerodds.internal.InProcessBus;

import java.nio.ByteBuffer;
import java.util.List;

import static org.junit.jupiter.api.Assertions.*;

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
    };

    static int dom = 500;
    static int nextDom() { return dom++; }

    @BeforeEach void setUp() { InProcessBus.instance().reset(); }
    @AfterEach void tearDown() { InProcessBus.instance().reset(); }

    private record Rig(DomainParticipant p, Topic<Reading> t, Publisher pub, Subscriber sub) {}

    private Rig rig() {
        DomainParticipant p = DomainParticipantFactory.getInstance().createParticipant(nextDom());
        Topic<Reading> t = p.createTopic("Reading", Reading.class);
        return new Rig(p, t, p.createPublisher(), p.createSubscriber());
    }

    @Test
    void historyKeepLastCapsPerInstance() {
        Rig r = rig();
        QosProfile q = QosProfile.DEFAULT.withHistory(History.KEEP_LAST_1);
        DataReader<Reading> dr = r.sub.createDataReader(r.t, SUPPORT, q);
        DataWriter<Reading> dw = r.pub.createDataWriter(r.t, SUPPORT, q);
        for (int i = 0; i < 5; i++) dw.write(new Reading(1, i, i));
        List<Sample<Reading>> got = dr.read();
        // KEEP_LAST(1) on a single key -> exactly the last sample retained.
        assertEquals(1, got.size());
        assertEquals(4, got.get(0).data().seq());
    }

    @Test
    void historyKeepLastIsPerInstanceNotGlobal() {
        Rig r = rig();
        QosProfile q = QosProfile.DEFAULT.withHistory(History.KEEP_LAST_1);
        DataReader<Reading> dr = r.sub.createDataReader(r.t, SUPPORT, q);
        DataWriter<Reading> dw = r.pub.createDataWriter(r.t, SUPPORT, q);
        for (int i = 0; i < 3; i++) dw.write(new Reading(1, i, i));
        for (int i = 0; i < 3; i++) dw.write(new Reading(2, i, i));
        // Two keys, KEEP_LAST(1) each -> 2 samples total.
        assertEquals(2, dr.read().size());
    }

    @Test
    void transientLocalReplaysToLateJoiner() {
        Rig r = rig();
        QosProfile tl = QosProfile.DEFAULT
                .withDurability(Durability.TRANSIENT_LOCAL)
                .withHistory(History.KEEP_LAST_1);
        DataWriter<Reading> dw = r.pub.createDataWriter(r.t, SUPPORT, tl);
        dw.write(new Reading(1, 0, 10.0));
        dw.write(new Reading(1, 1, 11.0)); // KEEP_LAST(1) -> only seq=1 retained
        // Late joiner created AFTER the writes.
        DataReader<Reading> dr = r.sub.createDataReader(r.t, SUPPORT, tl);
        List<Sample<Reading>> got = dr.take();
        assertEquals(1, got.size());
        assertEquals(1, got.get(0).data().seq());
    }

    @Test
    void volatileDoesNotReplayToLateJoiner() {
        Rig r = rig();
        QosProfile vol = QosProfile.DEFAULT.withDurability(Durability.VOLATILE);
        DataWriter<Reading> dw = r.pub.createDataWriter(r.t, SUPPORT, vol);
        dw.write(new Reading(1, 0, 10.0));
        DataReader<Reading> dr = r.sub.createDataReader(r.t, SUPPORT, vol);
        assertEquals(0, dr.take().size());
    }

    @Test
    void exclusiveOwnershipArbitratesToHighestStrength() {
        Rig r = rig();
        QosProfile drq = QosProfile.DEFAULT.withOwnership(Ownership.EXCLUSIVE);
        DataReader<Reading> dr = r.sub.createDataReader(r.t, SUPPORT, drq);
        QosProfile low = QosProfile.DEFAULT.withOwnership(Ownership.EXCLUSIVE)
                .withOwnershipStrength(new OwnershipStrength(1));
        QosProfile high = QosProfile.DEFAULT.withOwnership(Ownership.EXCLUSIVE)
                .withOwnershipStrength(new OwnershipStrength(10));
        DataWriter<Reading> dwLow = r.pub.createDataWriter(r.t, SUPPORT, low);
        DataWriter<Reading> dwHigh = r.pub.createDataWriter(r.t, SUPPORT, high);
        dwHigh.write(new Reading(1, 0, 100.0)); // owner = high (strength 10)
        dwLow.write(new Reading(1, 1, 1.0));     // filtered out (lower strength)
        dwHigh.write(new Reading(1, 2, 102.0));  // owner accepted
        List<Sample<Reading>> got = dr.take();
        // Only the owner's two samples delivered.
        assertEquals(2, got.size());
        assertTrue(got.stream().allMatch(s -> s.data().value() >= 100.0));
    }

    @Test
    void sharedOwnershipDeliversFromAllWriters() {
        Rig r = rig();
        DataReader<Reading> dr = r.sub.createDataReader(r.t, SUPPORT, QosProfile.DEFAULT);
        DataWriter<Reading> a = r.pub.createDataWriter(r.t, SUPPORT, QosProfile.DEFAULT);
        DataWriter<Reading> b = r.pub.createDataWriter(r.t, SUPPORT, QosProfile.DEFAULT);
        a.write(new Reading(1, 0, 1.0));
        b.write(new Reading(1, 1, 2.0));
        assertEquals(2, dr.take().size());
    }

    @Test
    void partitionIsolatesNonOverlappingEndpoints() {
        Rig r = rig();
        QosProfile pubA = QosProfile.DEFAULT.withPartition(new Partition("A"));
        QosProfile subB = QosProfile.DEFAULT.withPartition(new Partition("B"));
        DataReader<Reading> dr = r.sub.createDataReader(r.t, SUPPORT, subB);
        DataWriter<Reading> dw = r.pub.createDataWriter(r.t, SUPPORT, pubA);
        dw.write(new Reading(1, 0, 1.0));
        assertEquals(0, dr.take().size(), "A and B partitions must not communicate");
    }

    @Test
    void partitionMatchesOnOverlap() {
        Rig r = rig();
        QosProfile pubA = QosProfile.DEFAULT.withPartition(new Partition("A", "X"));
        QosProfile subA = QosProfile.DEFAULT.withPartition(new Partition("X"));
        DataReader<Reading> dr = r.sub.createDataReader(r.t, SUPPORT, subA);
        DataWriter<Reading> dw = r.pub.createDataWriter(r.t, SUPPORT, pubA);
        dw.write(new Reading(1, 0, 1.0));
        assertEquals(1, dr.take().size());
    }

    @Test
    void partitionWildcardMatches() {
        Rig r = rig();
        QosProfile pub = QosProfile.DEFAULT.withPartition(new Partition("sensor.temp"));
        QosProfile sub = QosProfile.DEFAULT.withPartition(new Partition("sensor.*"));
        DataReader<Reading> dr = r.sub.createDataReader(r.t, SUPPORT, sub);
        DataWriter<Reading> dw = r.pub.createDataWriter(r.t, SUPPORT, pub);
        dw.write(new Reading(1, 0, 1.0));
        assertEquals(1, dr.take().size());
    }

    @Test
    void contentFilteredTopicDropsNonMatching() {
        Rig r = rig();
        ContentFilteredTopic<Reading> cft = new ContentFilteredTopic<>(
                "HighValue", r.t, "value > 50.0", rd -> rd.value() > 50.0);
        DataReader<Reading> dr = r.sub.createDataReader(cft, SUPPORT, QosProfile.DEFAULT);
        DataWriter<Reading> dw = r.pub.createDataWriter(r.t, SUPPORT, QosProfile.DEFAULT);
        dw.write(new Reading(1, 0, 10.0));   // dropped
        dw.write(new Reading(2, 1, 99.0));   // kept
        dw.write(new Reading(3, 2, 5.0));    // dropped
        List<Sample<Reading>> got = dr.take();
        assertEquals(1, got.size());
        assertEquals(99.0, got.get(0).data().value(), 0.0);
    }

    @Test
    void disposeMarksInstanceNotAliveDisposed() {
        Rig r = rig();
        DataReader<Reading> dr = r.sub.createDataReader(r.t, SUPPORT, QosProfile.DEFAULT);
        DataWriter<Reading> dw = r.pub.createDataWriter(r.t, SUPPORT, QosProfile.DEFAULT);
        dw.write(new Reading(7, 0, 1.0));
        dw.dispose(new Reading(7, 0, 0.0));
        List<Sample<Reading>> got = dr.take();
        assertTrue(got.stream().anyMatch(
                s -> s.instanceState() == Sample.InstanceState.NOT_ALIVE_DISPOSED),
                "dispose -> NOT_ALIVE_DISPOSED");
    }

    @Test
    void unregisterMarksInstanceNotAliveNoWriters() {
        Rig r = rig();
        DataReader<Reading> dr = r.sub.createDataReader(r.t, SUPPORT, QosProfile.DEFAULT);
        DataWriter<Reading> dw = r.pub.createDataWriter(r.t, SUPPORT, QosProfile.DEFAULT);
        dw.write(new Reading(7, 0, 1.0));
        dw.unregisterInstance(new Reading(7, 0, 0.0));
        List<Sample<Reading>> got = dr.take();
        assertTrue(got.stream().anyMatch(
                s -> s.instanceState() == Sample.InstanceState.NOT_ALIVE_NO_WRITERS),
                "unregister -> NOT_ALIVE_NO_WRITERS");
    }

    @Test
    void registerAndLookupInstance() {
        Rig r = rig();
        DataWriter<Reading> dw = r.pub.createDataWriter(r.t, SUPPORT, QosProfile.DEFAULT);
        var h = dw.registerInstance(new Reading(42, 0, 0.0));
        assertFalse(h.isNil());
        assertEquals(h, dw.lookupInstance(new Reading(42, 9, 9.0)));
        assertTrue(dw.lookupInstance(new Reading(99, 0, 0.0)).isNil());
    }

    @Test
    void disposedThenWrittenIsRebornAlive() {
        Rig r = rig();
        DataReader<Reading> dr = r.sub.createDataReader(r.t, SUPPORT, QosProfile.DEFAULT);
        DataWriter<Reading> dw = r.pub.createDataWriter(r.t, SUPPORT, QosProfile.DEFAULT);
        dw.write(new Reading(7, 0, 1.0));
        dw.dispose(new Reading(7, 0, 0.0));
        dw.write(new Reading(7, 1, 2.0)); // resurrect
        List<Sample<Reading>> got = dr.take();
        Sample<Reading> last = got.get(got.size() - 1);
        assertEquals(Sample.InstanceState.ALIVE, last.instanceState());
        assertEquals(2.0, last.data().value(), 0.0);
    }

    @Test
    void requestedDeadlineMissedIsRaised() throws InterruptedException {
        Rig r = rig();
        QosProfile q = QosProfile.DEFAULT.withDeadline(new Deadline(Duration.fromMillis(50)));
        DataReader<Reading> dr = r.sub.createDataReader(r.t, SUPPORT, q);
        DataWriter<Reading> dw = r.pub.createDataWriter(r.t, SUPPORT, q);
        dw.write(new Reading(1, 0, 1.0));
        RequestedDeadlineMissedStatus before = dr.getRequestedDeadlineMissedStatus();
        assertEquals(0, before.totalCount());
        Thread.sleep(140); // > 2 deadline periods with no new sample
        RequestedDeadlineMissedStatus after = dr.getRequestedDeadlineMissedStatus();
        assertTrue(after.totalCount() >= 1, "deadline should be missed at least once");
        assertTrue(after.totalCountChange() >= 1);
    }

    @Test
    void infiniteDeadlineNeverMisses() throws InterruptedException {
        Rig r = rig();
        DataReader<Reading> dr = r.sub.createDataReader(r.t, SUPPORT, QosProfile.DEFAULT);
        DataWriter<Reading> dw = r.pub.createDataWriter(r.t, SUPPORT, QosProfile.DEFAULT);
        dw.write(new Reading(1, 0, 1.0));
        Thread.sleep(60);
        assertEquals(0, dr.getRequestedDeadlineMissedStatus().totalCount());
    }

    @Test
    void livelinessChangedReflectsWriterPresence() {
        Rig r = rig();
        DataReader<Reading> dr = r.sub.createDataReader(r.t, SUPPORT, QosProfile.DEFAULT);
        DataWriter<Reading> dw = r.pub.createDataWriter(r.t, SUPPORT, QosProfile.DEFAULT);
        LivelinessChangedStatus alive = dr.getLivelinessChangedStatus();
        assertEquals(1, alive.aliveCount());
        assertEquals(0, alive.notAliveCount());
        dw.simulateLivelinessLost();
        LivelinessChangedStatus lost = dr.getLivelinessChangedStatus();
        assertEquals(0, lost.aliveCount());
        assertEquals(1, lost.notAliveCount());
        assertEquals(-1, lost.aliveCountChange());
        assertEquals(1, lost.notAliveCountChange());
    }

    @Test
    void rxoIncompatibleReliabilityDetected() {
        QosProfile reader = QosProfile.DEFAULT; // RELIABLE
        QosProfile writer = QosProfile.DEFAULT.withReliability(Reliability.BEST_EFFORT_DEFAULT);
        assertFalse(reader.isCompatibleWith(writer));
    }

    @Test
    void rxoDeadlineCompatibility() {
        // requested(reader) must be >= offered(writer)
        QosProfile reader = QosProfile.DEFAULT.withDeadline(new Deadline(Duration.fromMillis(100)));
        QosProfile fastWriter = QosProfile.DEFAULT.withDeadline(new Deadline(Duration.fromMillis(50)));
        QosProfile slowWriter = QosProfile.DEFAULT.withDeadline(new Deadline(Duration.fromMillis(200)));
        assertTrue(reader.isCompatibleWith(fastWriter));
        assertFalse(reader.isCompatibleWith(slowWriter));
    }

    @Test
    void ownershipKindMustMatch() {
        assertFalse(Ownership.EXCLUSIVE.isCompatibleWith(Ownership.SHARED));
        assertTrue(Ownership.EXCLUSIVE.isCompatibleWith(Ownership.EXCLUSIVE));
    }

    @Test
    void livelinessRxoKindAndLease() {
        Liveliness reqAuto = new Liveliness(Liveliness.Kind.AUTOMATIC, Duration.fromMillis(100));
        Liveliness offManualTopic = new Liveliness(Liveliness.Kind.MANUAL_BY_TOPIC, Duration.fromMillis(50));
        assertTrue(reqAuto.isCompatibleWith(offManualTopic));
        Liveliness reqManualTopic = new Liveliness(Liveliness.Kind.MANUAL_BY_TOPIC, Duration.fromMillis(100));
        Liveliness offAuto = new Liveliness(Liveliness.Kind.AUTOMATIC, Duration.fromMillis(50));
        assertFalse(reqManualTopic.isCompatibleWith(offAuto));
    }
}
