// SPDX-License-Identifier: Apache-2.0
package org.omg.dds;

import java.util.ArrayList;
import java.util.Collection;
import java.util.List;
import java.util.function.BooleanSupplier;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.omg.dds.domain.DomainParticipant;
import org.omg.dds.pub.DataWriter;
import org.omg.dds.pub.Publisher;
import org.omg.dds.sub.DataReader;
import org.omg.dds.sub.Sample;
import org.omg.dds.sub.Subscriber;
import org.omg.dds.topic.Topic;
import org.zerodds.cdr.ByteArrayTypeSupport;
import org.zerodds.cdr.EndianMode;
import org.zerodds.internal.InProcessBus;
import org.zerodds.rtps.QosParameters;
import org.zerodds.rtps.Rtps;
import org.zerodds.rtps.RtpsParticipant;
import org.zerodds.testing.TestDomains;
import org.zerodds.rtps.SerializedSample;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * PRESENTATION access_scope GROUP with ordered_access (DDS 1.4 2.2.2.5.2.10, 2.2.2.5.3.8, 2.2.3.6):
 * the readers of a Subscriber give their samples in the order they were written, over several
 * readers: get_datareaders is a list with a reader for each sample, and a read returns one sample.
 * Over RTPS the order is the group sequence number of the Publisher of the writers (DDSI-RTPS 2.5
 * 8.7.5), a group coherent set arrives as a whole (8.7.6). Each test leases its domain from the band of the Rust tests
 * ({@code org.zerodds.testing.TestDomains}) and gives it back with no participant left in it.
 */
class GroupAccessTest {

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

    private static void waitFor(BooleanSupplier c, String what) throws InterruptedException {
        long end = System.currentTimeMillis() + 20_000;
        while (!c.getAsBoolean() && System.currentTimeMillis() < end) {
            Thread.sleep(50);
        }
        assertTrue(c.getAsBoolean(), what);
    }

    private static byte[] body(int v) {
        return ByteArrayTypeSupport.INSTANCE.encode(new byte[] {(byte) v});
    }

    private static byte[] encap() {
        return SerializedSample.encapsulation(2, ByteArrayTypeSupport.INSTANCE.getExtensibility(),
                EndianMode.LITTLE_ENDIAN);
    }

    /** The samples the two readers hold, counted inside begin_access/end_access (access scope GROUP). */
    private static int count(Subscriber subscriber, DataReader<byte[]> one, DataReader<byte[]> two) {
        subscriber.beginAccess();
        try {
            return one.read(new ArrayList<>()).size() + two.read(new ArrayList<>()).size();
        } finally {
            subscriber.endAccess();
        }
    }

    /** The number of samples that begin_access/get_datareaders lists, one entry per sample (access scope GROUP). */
    private static int listed(Subscriber subscriber) {
        subscriber.beginAccess();
        try {
            return subscriber.getDataReaders(new ArrayList<DataReader<?>>()).size();
        } finally {
            subscriber.endAccess();
        }
    }

    private static Subscriber groupSubscriber(DomainParticipant p, boolean coherent) {
        return p.createSubscriber(Qos.subscriber().withPolicy(Qos.policies().Presentation().withGroup()
                .withOrderedAccess(true).withCoherentAccess(coherent)));
    }

    private static Publisher groupPublisher(DomainParticipant p) {
        return p.createPublisher(Qos.publisher().withPolicy(Qos.policies().Presentation().withGroup()
                .withOrderedAccess(true).withCoherentAccess(true)));
    }

    @Test
    void getDataReadersIsAListInTheOrderOfWritingAndAReadGivesOneSample() throws Exception {
        DomainParticipant participant = Qos.factory().createParticipant(domain);
        Subscriber subscriber = groupSubscriber(participant, false);
        Publisher publisher = groupPublisher(participant);
        Topic<byte[]> one = participant.createTopic("GroupOne", byte[].class);
        Topic<byte[]> two = participant.createTopic("GroupTwo", byte[].class);
        DataReader<byte[]> readerOne = subscriber.createDataReader(one, Qos.readerKeepAll());
        DataReader<byte[]> readerTwo = subscriber.createDataReader(two, Qos.readerKeepAll());
        DataWriter<byte[]> writerOne = publisher.createDataWriter(one);
        DataWriter<byte[]> writerTwo = publisher.createDataWriter(two);
        writerOne.write(new byte[] {1});
        writerTwo.write(new byte[] {2});
        writerOne.write(new byte[] {3});

        subscriber.beginAccess();
        try {
            Collection<DataReader<?>> list = subscriber.getDataReaders(new ArrayList<DataReader<?>>());
            List<DataReader<?>> asList = new ArrayList<>(list);
            assertEquals(List.of(readerOne, readerTwo, readerOne), asList,
                    "a reader for each sample, in the order of writing");
            List<Integer> read = new ArrayList<>();
            for (DataReader<?> r : asList) {
                @SuppressWarnings("unchecked")
                List<Sample<byte[]>> one1 = ((DataReader<byte[]>) r).take(new ArrayList<>());
                assertEquals(1, one1.size(), "a take returns at most one sample");
                read.add((int) one1.get(0).getData()[0]);
            }
            assertEquals(List.of(1, 2, 3), read);
        } finally {
            subscriber.endAccess();
        }
    }

    @Test
    void samplesOfTwoRemoteWritersOfOnePublisherArriveInTheOrderOfTheGroupSequenceNumber() throws Exception {
        DomainParticipant participant = Qos.factory().createParticipant(domain);
        Subscriber subscriber = groupSubscriber(participant, true);
        Topic<byte[]> one = participant.createTopic("GroupWireOne", byte[].class);
        Topic<byte[]> two = participant.createTopic("GroupWireTwo", byte[].class);
        DataReader<byte[]> readerOne = subscriber.createDataReader(one, Qos.readerKeepAll());
        DataReader<byte[]> readerTwo = subscriber.createDataReader(two, Qos.readerKeepAll());
        RtpsParticipant remote = RtpsParticipant.newParticipant(domain);
        try {
            QosParameters qos = new QosParameters();
            qos.reliability = new QosParameters.Reliability();
            qos.reliability.kind = Rtps.RELIABILITY_RELIABLE;
            RtpsParticipant.WireWriter a = remote.createWriter("GroupWireOne", one.getTypeName(), encap(),
                    new byte[0], new byte[0], false, true, qos);
            RtpsParticipant.WireWriter b = remote.createWriter("GroupWireTwo", two.getTypeName(), encap(),
                    new byte[0], new byte[0], false, true, qos);
            waitFor(() -> readerOne.getMatchedPublications().size() == 1
                    && readerTwo.getMatchedPublications().size() == 1, "both readers meet their writers");
            byte[] digest = new byte[] {1, 2, 3, 4};
            // the second writer's sample has the lower group sequence number but is sent later
            a.write(body(1), null, 0L, new RtpsParticipant.GroupStamp(10, 0, digest));
            a.write(body(3), null, 0L, new RtpsParticipant.GroupStamp(12, 0, digest));
            Thread.sleep(300);
            b.write(body(2), null, 0L, new RtpsParticipant.GroupStamp(11, 0, digest));
            waitFor(() -> {
                subscriber.beginAccess();
                try {
                    return subscriber.getDataReaders(new ArrayList<DataReader<?>>()).size() == 3;
                } finally {
                    subscriber.endAccess();
                }
            }, "three samples");
            subscriber.beginAccess();
            try {
                List<DataReader<?>> list = new ArrayList<>(
                        subscriber.getDataReaders(new ArrayList<DataReader<?>>()));
                assertEquals(List.of(readerOne, readerTwo, readerOne), list);
            } finally {
                subscriber.endAccess();
            }
        } finally {
            remote.close();
            participant.close();
        }
    }

    @Test
    void aGroupCoherentSetOfTwoRemoteWritersArrivesAsAWholeAfterTheEndMarkers() throws Exception {
        DomainParticipant participant = Qos.factory().createParticipant(domain);
        Subscriber subscriber = groupSubscriber(participant, true);
        Topic<byte[]> one = participant.createTopic("GroupSetOne", byte[].class);
        Topic<byte[]> two = participant.createTopic("GroupSetTwo", byte[].class);
        DataReader<byte[]> readerOne = subscriber.createDataReader(one, Qos.readerKeepAll());
        DataReader<byte[]> readerTwo = subscriber.createDataReader(two, Qos.readerKeepAll());
        RtpsParticipant remote = RtpsParticipant.newParticipant(domain);
        try {
            QosParameters qos = new QosParameters();
            qos.reliability = new QosParameters.Reliability();
            qos.reliability.kind = Rtps.RELIABILITY_RELIABLE;
            RtpsParticipant.WireWriter a = remote.createWriter("GroupSetOne", one.getTypeName(), encap(),
                    new byte[0], new byte[0], false, true, qos);
            RtpsParticipant.WireWriter b = remote.createWriter("GroupSetTwo", two.getTypeName(), encap(),
                    new byte[0], new byte[0], false, true, qos);
            waitFor(() -> readerOne.getMatchedPublications().size() == 1
                    && readerTwo.getMatchedPublications().size() == 1, "both readers meet their writers");
            byte[] digest = new byte[] {1, 2, 3, 4};
            a.write(body(1), null, 0L, new RtpsParticipant.GroupStamp(21, 21, digest));
            b.write(body(2), null, 0L, new RtpsParticipant.GroupStamp(22, 21, digest));
            Thread.sleep(800);
            assertEquals(0, count(subscriber, readerOne, readerTwo), "the set is held until its end");
            a.endGroupCoherentSet(new RtpsParticipant.GroupStamp(23, 21, digest));
            b.endGroupCoherentSet(new RtpsParticipant.GroupStamp(23, 21, digest));
            waitFor(() -> count(subscriber, readerOne, readerTwo) == 2, "the set arrives as a whole");
        } finally {
            remote.close();
            participant.close();
        }
    }

    @Test
    void aGroupPublisherWithTwoWritersSendsItsSamplesAndSetsToAPeer() throws Exception {
        DomainParticipant participant = Qos.factory().createParticipant(domain);
        Publisher publisher = groupPublisher(participant);
        Topic<byte[]> one = participant.createTopic("GroupSendOne", byte[].class);
        Topic<byte[]> two = participant.createTopic("GroupSendTwo", byte[].class);
        DataWriter<byte[]> writerOne = publisher.createDataWriter(one);
        DataWriter<byte[]> writerTwo = publisher.createDataWriter(two);
        RtpsParticipant remote = RtpsParticipant.newParticipant(domain);
        try {
            List<Integer> gotOne = new java.util.concurrent.CopyOnWriteArrayList<>();
            List<Integer> gotTwo = new java.util.concurrent.CopyOnWriteArrayList<>();
            remote.createReader("GroupSendOne", one.getTypeName(), false,
                    bytes -> gotOne.add((int) ByteArrayTypeSupport.INSTANCE.decode(bytes)[0]));
            remote.createReader("GroupSendTwo", two.getTypeName(), false,
                    bytes -> gotTwo.add((int) ByteArrayTypeSupport.INSTANCE.decode(bytes)[0]));
            waitFor(() -> writerOne.getMatchedSubscriptions().size() == 1
                    && writerTwo.getMatchedSubscriptions().size() == 1, "the writers meet the readers");
            publisher.beginCoherentChanges();
            writerOne.write(new byte[] {1});
            writerTwo.write(new byte[] {2});
            publisher.endCoherentChanges();
            waitFor(() -> gotOne.equals(List.of(1)) && gotTwo.equals(List.of(2)),
                    "the samples of the group set arrive; the End Coherent Set DATA carries no data");
        } finally {
            remote.close();
            participant.close();
        }
    }

    @Test
    void aNumberOfAnUnmatchedWriterIsGivenUpByTheGroupHeartbeatsOfTheMatchedWriters() throws Exception {
        DomainParticipant participant = Qos.factory().createParticipant(domain);
        Subscriber subscriber = groupSubscriber(participant, true);
        Topic<byte[]> one = participant.createTopic("GroupHbOne", byte[].class);
        Topic<byte[]> two = participant.createTopic("GroupHbTwo", byte[].class);
        DataReader<byte[]> readerOne = subscriber.createDataReader(one, Qos.readerKeepAll());
        DataReader<byte[]> readerTwo = subscriber.createDataReader(two, Qos.readerKeepAll());
        RtpsParticipant remote = RtpsParticipant.newParticipant(domain);
        try {
            QosParameters qos = new QosParameters();
            qos.reliability = new QosParameters.Reliability();
            qos.reliability.kind = Rtps.RELIABILITY_RELIABLE;
            RtpsParticipant.WireWriter a = remote.createWriter("GroupHbOne", one.getTypeName(), encap(),
                    new byte[0], new byte[0], false, true, qos);
            RtpsParticipant.WireWriter b = remote.createWriter("GroupHbTwo", two.getTypeName(), encap(),
                    new byte[0], new byte[0], false, true, qos);
            waitFor(() -> readerOne.getMatchedPublications().size() == 1
                    && readerTwo.getMatchedPublications().size() == 1, "both readers meet their writers");
            byte[] digest = new byte[] {1, 2, 3, 4};
            // the Publisher gave out the group sequence numbers 10 to 12; 11 belongs to a writer that
            // no reader here is matched to
            a.setGroupInfo(() -> new RtpsParticipant.GroupStamp(12, 0, digest));
            b.setGroupInfo(() -> new RtpsParticipant.GroupStamp(12, 0, digest));
            a.write(body(1), null, 0L, new RtpsParticipant.GroupStamp(10, 0, digest));
            a.write(body(3), null, 0L, new RtpsParticipant.GroupStamp(12, 0, digest));
            waitFor(() -> listed(subscriber) == 2,
                    "11 is given up by the HEARTBEATs with currentGSN 12 and the empty range of the second writer");
        } finally {
            remote.close();
            participant.close();
        }
    }

    @Test
    void aNumberStaysMissingWhileAMatchedWriterSendsNoGroupHeartbeat() throws Exception {
        DomainParticipant participant = Qos.factory().createParticipant(domain);
        Subscriber subscriber = groupSubscriber(participant, true);
        Topic<byte[]> one = participant.createTopic("GroupNoHbOne", byte[].class);
        Topic<byte[]> two = participant.createTopic("GroupNoHbTwo", byte[].class);
        DataReader<byte[]> readerOne = subscriber.createDataReader(one, Qos.readerKeepAll());
        DataReader<byte[]> readerTwo = subscriber.createDataReader(two, Qos.readerKeepAll());
        RtpsParticipant remote = RtpsParticipant.newParticipant(domain);
        try {
            QosParameters qos = new QosParameters();
            qos.reliability = new QosParameters.Reliability();
            qos.reliability.kind = Rtps.RELIABILITY_RELIABLE;
            RtpsParticipant.WireWriter a = remote.createWriter("GroupNoHbOne", one.getTypeName(), encap(),
                    new byte[0], new byte[0], false, true, qos);
            RtpsParticipant.WireWriter b = remote.createWriter("GroupNoHbTwo", two.getTypeName(), encap(),
                    new byte[0], new byte[0], false, true, qos);
            waitFor(() -> readerOne.getMatchedPublications().size() == 1
                    && readerTwo.getMatchedPublications().size() == 1, "both readers meet their writers");
            byte[] digest = new byte[] {1, 2, 3, 4};
            // the Publisher gave out the group sequence numbers 10 to 12; 11 belongs to a writer that
            // no reader here is matched to
            a.setGroupInfo(() -> new RtpsParticipant.GroupStamp(12, 0, digest));
            // the second writer has no group info: it sends no HEARTBEAT with the GroupInfoFlag
            a.write(body(1), null, 0L, new RtpsParticipant.GroupStamp(10, 0, digest));
            a.write(body(3), null, 0L, new RtpsParticipant.GroupStamp(12, 0, digest));
            Thread.sleep(2500); // two periods of the HEARTBEAT
            assertEquals(1, listed(subscriber),
                    "there is no timer by default: 12 waits for 11");
        } finally {
            remote.close();
            participant.close();
        }
    }
}
