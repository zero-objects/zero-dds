// SPDX-License-Identifier: Apache-2.0
package org.omg.dds;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.omg.dds.core.event.RequestedIncompatibleQosEvent;
import org.omg.dds.core.event.SampleRejectedEvent;
import org.omg.dds.core.policy.Reliability;
import org.omg.dds.core.status.DataAvailableStatus;
import org.omg.dds.core.status.RequestedIncompatibleQosStatus;
import org.omg.dds.core.status.SampleRejectedStatus;
import org.omg.dds.domain.DomainParticipant;
import org.omg.dds.pub.DataWriter;
import org.omg.dds.sub.DataReader;
import org.omg.dds.sub.DataReaderAdapter;
import org.omg.dds.topic.Topic;
import org.zerodds.internal.InProcessBus;

import java.util.ArrayList;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.concurrent.atomic.AtomicReference;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * DDS 1.4 §2.2.4.1 statuses of a DataReader: SAMPLE_REJECTED from RESOURCE_LIMITS,
 * REQUESTED_INCOMPATIBLE_QOS to the listener, and the reset of a status when it is read.
 */
class ReaderStatusTest {

    @BeforeEach
    void setUp() {
        InProcessBus.instance().reset();
    }

    @AfterEach
    void tearDown() {
        InProcessBus.instance().reset();
    }

    @Test
    void aSampleBeyondMaxSamplesIsRejectedAndReportedToTheListener() throws Exception {
        DomainParticipant participant = Qos.factory().createParticipant(950);
        Topic<byte[]> topic = participant.createTopic("Limits", byte[].class);
        AtomicReference<SampleRejectedStatus> seen = new AtomicReference<>();
        DataReader<byte[]> reader = participant.createSubscriber().createDataReader(topic,
                Qos.readerKeepAll().withPolicy(
                        Qos.pf().ResourceLimits().withMaxSamples(1)));
        reader.setListener(new DataReaderAdapter<byte[]>() {
            @Override
            public void onSampleRejected(SampleRejectedEvent<byte[]> event) {
                seen.set(event.getStatus());
            }
        });
        DataWriter<byte[]> writer = participant.createPublisher().createDataWriter(topic);
        writer.write(new byte[] {1});
        assertEquals(0, reader.getSampleRejectedStatus().getTotalCount());
        writer.write(new byte[] {2});
        assertEquals(SampleRejectedStatus.Kind.REJECTED_BY_SAMPLES_LIMIT,
                seen.get().getLastReason());
        assertEquals(1, seen.get().getTotalCount());
        // the sample that fit is the one the reader holds
        assertEquals(1, reader.read(new ArrayList<>()).size());
        // the listener read the status, so the change count started again
        assertEquals(0, reader.getSampleRejectedStatus().getTotalCountChange());
        assertEquals(1, reader.getSampleRejectedStatus().getTotalCount());
    }

    @Test
    void aWriterThatOffersLessThanRequestedIsReportedToTheListener() throws Exception {
        DomainParticipant participant = Qos.factory().createParticipant(951);
        Topic<byte[]> topic = participant.createTopic("Incompatible", byte[].class);
        AtomicInteger count = new AtomicInteger();
        AtomicReference<RequestedIncompatibleQosStatus> seen = new AtomicReference<>();
        DataReader<byte[]> reader = participant.createSubscriber().createDataReader(topic,
                Qos.reader().withPolicy(Qos.pf().Reliability().withReliable()));
        reader.setListener(new DataReaderAdapter<byte[]>() {
            @Override
            public void onRequestedIncompatibleQos(RequestedIncompatibleQosEvent<byte[]> event) {
                count.incrementAndGet();
                seen.set(event.getStatus());
            }
        });
        participant.createPublisher().createDataWriter(topic,
                Qos.writer().withPolicy(Qos.pf().Reliability().withBestEffort()));
        assertEquals(1, count.get());
        assertEquals(Reliability.class, seen.get().getLastPolicyClass());
        assertEquals(0, reader.getMatchedPublications().size());
    }

    @Test
    void aStatusIsResetWhenItIsReadAndDataAvailableWhenTheSamplesAreTaken() throws Exception {
        DomainParticipant participant = Qos.factory().createParticipant(952);
        Topic<byte[]> topic = participant.createTopic("Reset", byte[].class);
        DataReader<byte[]> reader = participant.createSubscriber().createDataReader(topic);
        DataWriter<byte[]> writer = participant.createPublisher().createDataWriter(topic);
        writer.write(new byte[] {1});
        assertTrue(reader.getStatusChanges().contains(DataAvailableStatus.class));
        // asking for the changes does not reset them
        assertTrue(reader.getStatusChanges().contains(DataAvailableStatus.class));
        assertTrue(reader.getStatusChanges().contains(
                org.omg.dds.core.status.SubscriptionMatchedStatus.class));
        reader.getSubscriptionMatchedStatus();
        assertFalse(reader.getStatusChanges().contains(
                org.omg.dds.core.status.SubscriptionMatchedStatus.class));
        reader.take(new ArrayList<>());
        assertFalse(reader.getStatusChanges().contains(DataAvailableStatus.class));
    }
}
