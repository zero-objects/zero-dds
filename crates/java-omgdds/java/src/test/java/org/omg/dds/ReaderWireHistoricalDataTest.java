// SPDX-License-Identifier: Apache-2.0
package org.omg.dds;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import org.omg.dds.pub.DataWriter;
import java.util.List;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.TimeoutException;
import java.util.function.BooleanSupplier;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.omg.dds.domain.DomainParticipant;
import org.omg.dds.sub.DataReader;
import org.omg.dds.topic.Topic;
import org.zerodds.internal.InProcessBus;
import org.zerodds.rtps.QosParameters;
import org.zerodds.rtps.Rtps;
import org.zerodds.rtps.RtpsParticipant;
import org.zerodds.testing.TestDomains;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * wait_for_historical_data of a DataReader whose writer is in another process (DDS 1.4
 * 2.2.2.5.3.32): it returns when the HEARTBEAT of the reliable, TRANSIENT_LOCAL writer has
 * announced its last sample and the reader has received up to it, and is a TimeoutException while
 * the writer has announced nothing. The other process is a second RTPS participant, as in
 * {@code WriterWireAckTest}. Each test leases its domain from the band of the Rust tests
 * ({@code org.zerodds.testing.TestDomains}) and gives it back with no participant left in it.
 */
class ReaderWireHistoricalDataTest {

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

    private static RtpsParticipant.WireWriter writer(RtpsParticipant remote, String topic,
            String type) {
        QosParameters qos = new QosParameters();
        qos.reliability = new QosParameters.Reliability();
        qos.reliability.kind = Rtps.RELIABILITY_RELIABLE;
        qos.durability = Rtps.DURABILITY_TRANSIENT_LOCAL;
        return remote.createWriter(topic, type,
                org.zerodds.rtps.SerializedSample.encapsulation(2,
                        org.zerodds.cdr.ByteArrayTypeSupport.INSTANCE.getExtensibility(),
                        org.zerodds.cdr.EndianMode.LITTLE_ENDIAN),
                new byte[0], new byte[0], false, true, qos);
    }

    private static byte[] body(int value) {
        return org.zerodds.cdr.ByteArrayTypeSupport.INSTANCE.encode(new byte[] {(byte) value});
    }

    private DataReader<byte[]> transientLocalReader(DomainParticipant participant, Topic<byte[]> topic) {
        return participant.createSubscriber().createDataReader(topic, Qos.readerKeepAll()
                .withPolicy(Qos.pf().Durability().withTransientLocal())
                .withPolicy(Qos.policies().DataReaderReliability().withReliable()));
    }

    @Test
    void javapsm_2_the_call_returns_once_the_announced_samples_have_arrived() throws Exception {
        DomainParticipant participant = Qos.factory().createParticipant(domain);
        Topic<byte[]> topic = participant.createTopic("WireHistorical", byte[].class);
        RtpsParticipant remote = RtpsParticipant.newParticipant(domain);
        try {
            RtpsParticipant.WireWriter writer = writer(remote, "WireHistorical", topic.getTypeName());
            // The writer of the other process keeps its samples for readers that match later
            // (TRANSIENT_LOCAL) and writes all three before the reader exists: the samples are
            // historical, and the HEARTBEAT after the match announces 1 to 3.
            List<RtpsParticipant.Kept> kept = new java.util.concurrent.CopyOnWriteArrayList<>();
            writer.keepForLateJoiners(() -> kept);
            for (int value = 1; value <= 3; value++) {
                long sequence = writer.write(body(value), null);
                kept.add(new RtpsParticipant.Kept(sequence, body(value), System.currentTimeMillis(),
                        Long.MAX_VALUE));
            }
            DataReader<byte[]> reader = transientLocalReader(participant, topic);
            waitFor(() -> reader.getMatchedPublications().size() == 1,
                    "the reader meets the writer of the other participant over SEDP");
            reader.waitForHistoricalData(20, TimeUnit.SECONDS);
            assertEquals(3, Qos.take(reader).size());
        } finally {
            remote.close();
            participant.close();
        }
    }

    @Test
    void javapsm_2_a_transient_local_writer_hands_what_it_keeps_to_a_reader_of_another_process_that_matches_later()
            throws Exception {
        DomainParticipant participant = Qos.factory().createParticipant(domain);
        Topic<byte[]> topic = participant.createTopic("WireLateJoiner", byte[].class);
        DataWriter<byte[]> writer = participant.createPublisher().createDataWriter(topic, Qos.writer()
                .withPolicy(Qos.pf().Durability().withTransientLocal())
                .withPolicy(Qos.pf().Reliability().withReliable())
                .withPolicy(Qos.pf().History().withKeepLast(2)));
        writer.write(new byte[] {1});
        writer.write(new byte[] {2});
        writer.write(new byte[] {3});
        RtpsParticipant remote = RtpsParticipant.newParticipant(domain);
        try {
            // DDS 1.4 §2.2.3.4: the writer keeps samples for readers that join later, as many as
            // its HISTORY says (KEEP_LAST 2): the reader of the other process gets 2 and 3
            List<byte[]> got = new java.util.concurrent.CopyOnWriteArrayList<>();
            QosParameters qos = new QosParameters();
            qos.reliability = new QosParameters.Reliability();
            qos.reliability.kind = Rtps.RELIABILITY_RELIABLE;
            qos.durability = Rtps.DURABILITY_TRANSIENT_LOCAL;
            remote.createTypedReader("WireLateJoiner", topic.getTypeName(), false, new int[] {2},
                    new byte[0], sample -> got.add(sample.body()), qos);
            waitFor(() -> got.size() >= 2, "the reader that matched later gets the kept samples");
            assertArrayEquals(body(2), got.get(0));
            assertArrayEquals(body(3), got.get(1));
            assertEquals(2, got.size());
        } finally {
            remote.close();
            participant.close();
        }
    }

    @Test
    void javapsm_2_the_call_is_a_timeout_exception_while_the_writer_announced_nothing() throws Exception {
        DomainParticipant participant = Qos.factory().createParticipant(domain);
        Topic<byte[]> topic = participant.createTopic("WireHistoricalNone", byte[].class);
        DataReader<byte[]> reader = transientLocalReader(participant, topic);
        RtpsParticipant remote = RtpsParticipant.newParticipant(domain);
        try {
            writer(remote, "WireHistoricalNone", topic.getTypeName());
            waitFor(() -> reader.getMatchedPublications().size() == 1,
                    "the reader meets the writer of the other participant over SEDP");
            assertThrows(TimeoutException.class,
                    () -> reader.waitForHistoricalData(300, TimeUnit.MILLISECONDS));
        } finally {
            remote.close();
            participant.close();
        }
    }
}
