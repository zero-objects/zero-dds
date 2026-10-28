// SPDX-License-Identifier: Apache-2.0
package org.omg.dds;

import java.util.ArrayList;
import java.util.List;
import java.util.function.BooleanSupplier;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.omg.dds.core.InstanceHandle;
import org.omg.dds.core.policy.TypeConsistencyEnforcement;
import org.omg.dds.domain.DomainParticipant;
import org.omg.dds.pub.DataWriter;
import org.omg.dds.sub.DataReader;
import org.omg.dds.sub.Sample;
import org.omg.dds.sub.Subscriber;
import org.omg.dds.topic.PublicationBuiltinTopicData;
import org.omg.dds.topic.Topic;
import org.zerodds.cdr.ByteArrayTypeSupport;
import org.zerodds.cdr.EndianMode;
import org.zerodds.cdr.ReflectionTypeSupport;
import org.zerodds.internal.InProcessBus;
import org.zerodds.rtps.QosParameters;
import org.zerodds.rtps.Rtps;
import org.zerodds.rtps.RtpsParticipant;
import org.zerodds.testing.TestDomains;
import org.zerodds.rtps.SerializedSample;
import org.zerodds.typeobject.TypeObjects;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * Behaviour of the entities towards peers in another process, which is a second RTPS
 * participant in the domain as in {@code WriterWireAckTest}: a changed QoS reaches the peer over
 * SEDP (DDS 1.4 2.2.2.1.1.1), a matched remote writer is described by get_matched_publication_data
 * (2.2.2.5.3.33), a reader with ASSIGNABLE_TYPE_CONSISTENCY takes a remote writer of another type
 * name (XTypes 1.3 7.2.4), and a coherent set of a remote writer reaches a coherent-access reader
 * as a whole (DDSI-RTPS 2.5 8.7.6). Each test leases its domain from the band of the Rust tests
 * ({@code org.zerodds.testing.TestDomains}) and gives it back with no participant left in it.
 */
class WireEntityBehaviourTest {

    private String previous;
    /** The domain of the test, leased from the band of crates/test-domains. */
    private TestDomains.Lease lease;
    private int domain;

    @BeforeEach
    void setUp() {
        lease = TestDomains.lease();
        domain = lease.id();
        previous = System.getProperty("zerodds.rtps.enable");
        System.setProperty("zerodds.rtps.enable", "true");
        InProcessBus.instance().reset();
    }

    @AfterEach
    void tearDown() {
        if (previous == null) {
            System.clearProperty("zerodds.rtps.enable");
        } else {
            System.setProperty("zerodds.rtps.enable", previous);
        }
        InProcessBus.instance().reset();
        // The participant the PSM made for the domain leaves it before the lease goes.
        RtpsParticipant.closeDomain(domain);
        lease.close();
    }

    private static void waitFor(BooleanSupplier condition, String what) throws InterruptedException {
        long end = System.currentTimeMillis() + 20_000;
        while (!condition.getAsBoolean() && System.currentTimeMillis() < end) {
            Thread.sleep(50);
        }
        assertTrue(condition.getAsBoolean(), what);
    }

    private static byte[] encap() {
        return SerializedSample.encapsulation(2, ByteArrayTypeSupport.INSTANCE.getExtensibility(),
                EndianMode.LITTLE_ENDIAN);
    }

    private static byte[] body(int value) {
        return ByteArrayTypeSupport.INSTANCE.encode(new byte[] {(byte) value});
    }

    @Test
    void aChangedQosOfAWriterReachesTheReaderOfAnotherProcessOverSedp() throws Exception {
        DomainParticipant participant = Qos.factory().createParticipant(domain);
        Topic<byte[]> topic = participant.createTopic("WireQosUpdate", byte[].class);
        DataWriter<byte[]> writer = participant.createPublisher().createDataWriter(topic);
        RtpsParticipant remote = RtpsParticipant.newParticipant(domain);
        try {
            RtpsParticipant.WireReader reader = remote.createReader("WireQosUpdate",
                    topic.getTypeName(), false, body -> { });
            waitFor(() -> reader.matchedEndpoints().size() == 1,
                    "the reader meets the writer over SEDP");
            writer.setQos(writer.getQos().withPolicy(Qos.pf().UserData().withValue(new byte[] {5, 6}, 0, 2)));
            waitFor(() -> {
                List<org.zerodds.rtps.DiscoveryData.Endpoint> seen = reader.matchedEndpoints();
                return seen.size() == 1 && seen.get(0).qos.userData != null
                        && java.util.Arrays.equals(seen.get(0).qos.userData, new byte[] {5, 6});
            }, "the changed USER_DATA is announced again");
        } finally {
            remote.close();
            participant.close();
        }
    }

    @Test
    void aRemoteWriterIsDescribedByTheMatchedPublicationData() throws Exception {
        DomainParticipant participant = Qos.factory().createParticipant(domain);
        Topic<byte[]> topic = participant.createTopic("WireMatchedData", byte[].class);
        DataReader<byte[]> reader = participant.createSubscriber().createDataReader(topic);
        RtpsParticipant remote = RtpsParticipant.newParticipant(domain);
        try {
            QosParameters qos = new QosParameters();
            qos.reliability = new QosParameters.Reliability();
            qos.reliability.kind = Rtps.RELIABILITY_RELIABLE;
            qos.ownershipStrength = 17;
            remote.createWriter("WireMatchedData", topic.getTypeName(), encap(), new byte[0],
                    new byte[0], false, true, qos);
            waitFor(() -> reader.getMatchedPublications().size() == 1,
                    "the reader meets the writer of the other participant");
            InstanceHandle handle = reader.getMatchedPublications().iterator().next();
            PublicationBuiltinTopicData data = reader.getMatchedPublicationData(handle);
            assertEquals("WireMatchedData", data.getTopicName());
            assertEquals(topic.getTypeName(), data.getTypeName());
            assertEquals(17, data.getOwnershipStrength().getValue());
        } finally {
            remote.close();
            participant.close();
        }
    }

    /** The view of the reader: the first members. */
    public static class Reading {
        public int id;
        public String name;
    }

    /** The view of the writer: the same members and one appended (XTypes 1.3 7.2.4.2). */
    public static class Writing {
        public int id;
        public String name;
        public double extra;
    }

    @Test
    void anAssignableReaderTakesARemoteWriterOfAnotherTypeNameByItsTypeObject() throws Exception {
        DomainParticipant participant = Qos.factory().createParticipant(domain);
        Topic<Reading> topic = participant.createTopic("WireCoercion", Reading.class);
        DataReader<Reading> reader = Qos.reader(participant.createSubscriber(), topic,
                ReflectionTypeSupport.of(Reading.class), Qos.readerKeepAll().withPolicy(
                        Qos.policies().TypeConsistency().withAssignableTypeConsistency()));
        RtpsParticipant remote = RtpsParticipant.newParticipant(domain);
        try {
            ReflectionTypeSupport<Writing> support = ReflectionTypeSupport.of(Writing.class);
            QosParameters qos = new QosParameters();
            qos.reliability = new QosParameters.Reliability();
            qos.reliability.kind = Rtps.RELIABILITY_RELIABLE;
            qos.typeObject = TypeObjects.announce(Qos.env(), support);
            assertTrue(qos.typeObject != null);
            RtpsParticipant.WireWriter writer = remote.createWriter("WireCoercion",
                    support.getTypeName(), SerializedSample.encapsulation(2,
                            support.getExtensibility(), EndianMode.LITTLE_ENDIAN),
                    new byte[0], new byte[0], false, true, qos,
                    // the peer is any implementation that sends to a reader allowing coercion
                    rr -> rr.qos != null && rr.qos.typeConsistency != null
                            && rr.qos.typeConsistency.kind != 0);
            waitFor(() -> reader.getMatchedPublications().size() == 1,
                    "the reader takes the writer of the other type name");
            Writing sent = new Writing();
            sent.id = 4;
            sent.name = "four";
            sent.extra = 2.5;
            List<Sample<Reading>> taken = new ArrayList<>();
            List<String> log = new java.util.concurrent.CopyOnWriteArrayList<>();
            java.util.function.Consumer<String> saved = RtpsParticipant.LOG;
            RtpsParticipant.LOG = log::add;
            writer.write(support.encode(sent));
            try {
                waitFor(() -> {
                    taken.addAll(Qos.take(reader));
                    return !taken.isEmpty();
                }, "the sample of the other type arrives");
            } catch (AssertionError failed) {
                throw new AssertionError(failed.getMessage() + " " + log, failed);
            } finally {
                RtpsParticipant.LOG = saved;
            }
            assertEquals(4, taken.get(0).getData().id);
            assertEquals("four", taken.get(0).getData().name);
        } finally {
            remote.close();
            participant.close();
        }
    }

    @Test
    void anExactReaderDoesNotTakeThatRemoteWriter() throws Exception {
        DomainParticipant participant = Qos.factory().createParticipant(domain);
        Topic<Reading> topic = participant.createTopic("WireNoCoercion", Reading.class);
        DataReader<Reading> reader = Qos.reader(participant.createSubscriber(), topic,
                ReflectionTypeSupport.of(Reading.class), Qos.readerKeepAll().withPolicy(
                        Qos.policies().TypeConsistency().withKind(
                                TypeConsistencyEnforcement.Kind.EXACT_TYPE_TYPE_CONSISTENCY)));
        RtpsParticipant remote = RtpsParticipant.newParticipant(domain);
        try {
            ReflectionTypeSupport<Writing> support = ReflectionTypeSupport.of(Writing.class);
            QosParameters qos = new QosParameters();
            qos.typeObject = TypeObjects.announce(Qos.env(), support);
            remote.createWriter("WireNoCoercion", support.getTypeName(), SerializedSample.encapsulation(
                    2, support.getExtensibility(), EndianMode.LITTLE_ENDIAN), new byte[0], new byte[0],
                    false, true, qos);
            Thread.sleep(2000);
            assertEquals(0, reader.getMatchedPublications().size());
        } finally {
            remote.close();
            participant.close();
        }
    }

    @Test
    void aCoherentSetOfARemoteWriterReachesACoherentAccessReaderAsAWhole() throws Exception {
        DomainParticipant participant = Qos.factory().createParticipant(domain);
        Topic<byte[]> topic = participant.createTopic("WireCoherent", byte[].class);
        Subscriber subscriber = participant.createSubscriber(Qos.subscriber().withPolicy(
                Qos.policies().Presentation().withTopic().withCoherentAccess(true)));
        DataReader<byte[]> reader = subscriber.createDataReader(topic, Qos.readerKeepAll());
        RtpsParticipant remote = RtpsParticipant.newParticipant(domain);
        try {
            QosParameters qos = new QosParameters();
            qos.reliability = new QosParameters.Reliability();
            qos.reliability.kind = Rtps.RELIABILITY_RELIABLE;
            RtpsParticipant.WireWriter writer = remote.createWriter("WireCoherent",
                    topic.getTypeName(), encap(), new byte[0], new byte[0], false, true, qos);
            waitFor(() -> reader.getMatchedPublications().size() == 1,
                    "the reader meets the writer of the other participant");
            writer.write(body(1), null, 1L);
            writer.write(body(2), null, 1L);
            Thread.sleep(1000);
            assertEquals(0, reader.read(new ArrayList<>()).size(),
                    "the set is not complete: the reader holds it back");
            writer.endCoherentSet(1L);
            waitFor(() -> reader.read(new ArrayList<>()).size() == 2,
                    "the end of the set releases both samples together");
            assertArrayEquals(new byte[] {1}, Qos.take(reader).get(0).getData());
        } finally {
            remote.close();
            participant.close();
        }
    }

    @Test
    void aWriterTakesARemoteReaderOfAnotherTypeNameThatAllowsCoercionAndHasAnAssignableType()
            throws Exception {
        DomainParticipant participant = Qos.factory().createParticipant(domain);
        Topic<Writing> topic = participant.createTopic("WireWriterCoercion", Writing.class);
        DataWriter<Writing> writer = participant.createPublisher().createDataWriter(topic);
        RtpsParticipant remote = RtpsParticipant.newParticipant(domain);
        try {
            ReflectionTypeSupport<Reading> reading = ReflectionTypeSupport.of(Reading.class);
            QosParameters allowing = new QosParameters();
            allowing.typeConsistency = new QosParameters.TypeConsistency();
            allowing.typeConsistency.kind = 1;
            allowing.typeObject = TypeObjects.announce(Qos.env(), reading);
            remote.createTypedReader("WireWriterCoercion", reading.getTypeName(), false,
                    new int[] {2}, new byte[0], sample -> { }, allowing);
            waitFor(() -> writer.getMatchedSubscriptions().size() == 1,
                    "the writer takes the reader of the other type name");

            QosParameters exact = new QosParameters();
            exact.typeConsistency = new QosParameters.TypeConsistency();
            exact.typeConsistency.kind = 0;
            exact.typeObject = allowing.typeObject;
            remote.createTypedReader("WireWriterCoercion", reading.getTypeName(), false,
                    new int[] {2}, new byte[0], sample -> { }, exact);
            Thread.sleep(2000);
            assertEquals(1, writer.getMatchedSubscriptions().size(),
                    "a reader that does not allow coercion is not taken");
        } finally {
            remote.close();
            participant.close();
        }
    }
}
