// SPDX-License-Identifier: Apache-2.0
package org.omg.dds;

import org.omg.dds.sub.ViewState;
import org.omg.dds.sub.SampleState;
import org.omg.dds.sub.InstanceState;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.omg.dds.core.policy.History;
import org.omg.dds.domain.DomainParticipant;
import org.omg.dds.domain.DomainParticipantFactory;
import org.omg.dds.pub.DataWriter;
import org.omg.dds.sub.DataReader;
import org.omg.dds.sub.Sample;
import org.omg.dds.topic.Topic;
import org.zerodds.internal.InProcessBus;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import org.omg.dds.sub.DataReaderQos;
import org.omg.dds.pub.DataWriterQos;

/**
 * DDS 1.2 §7.1.2.5.1.2 (sample_state), §7.1.2.5.1.3 (instance_state snapshot)
 * and §7.1.2.5.1.8 (view_state).
 */
class SampleStateTest {

    private static int dom = 70;

    private static int nextDom() {
        return dom++;
    }

    @BeforeEach
    void setUp() throws Exception {
        InProcessBus.instance().reset();
    }

    @AfterEach
    void tearDown() throws Exception {
        InProcessBus.instance().reset();
    }

    @Test
    void readMarksSamplesRead() throws Exception {
        DomainParticipant participant =
                Qos.factory().createParticipant(nextDom());
        Topic<byte[]> topic = participant.createTopic("sampleState", byte[].class);
        DataWriter<byte[]> writer = participant.createPublisher().createDataWriter(topic);
        DataReader<byte[]> reader = participant.createSubscriber().createDataReader(topic);

        writer.write(new byte[] {1});
        List<Sample<byte[]>> first = Qos.read(reader);
        assertEquals(SampleState.NOT_READ, first.get(0).getSampleState());
        List<Sample<byte[]>> second = Qos.read(reader);
        assertEquals(SampleState.READ, second.get(0).getSampleState(),
                "a sample accessed by read is READ on the next read");

        participant.close();
    }

    @Test
    void viewStateIsNewUntilFirstAccess() throws Exception {
        DomainParticipant participant =
                Qos.factory().createParticipant(nextDom());
        Topic<byte[]> topic = participant.createTopic("viewState", byte[].class);
        DataWriter<byte[]> writer = participant.createPublisher().createDataWriter(topic);
        DataReader<byte[]> reader = participant.createSubscriber().createDataReader(topic);

        writer.write(new byte[] {1});
        List<Sample<byte[]>> first = Qos.read(reader);
        assertEquals(ViewState.NEW, first.get(0).getViewState());

        writer.write(new byte[] {2});
        List<Sample<byte[]>> second = Qos.read(reader);
        assertEquals(ViewState.NOT_NEW, second.get(0).getViewState(),
                "an instance already accessed is NOT_NEW");

        participant.close();
    }

    @Test
    void disposeSnapshotsInstanceStateForAllSamples() throws Exception {
        DomainParticipant participant =
                Qos.factory().createParticipant(nextDom());
        Topic<byte[]> topic = participant.createTopic("instanceState", byte[].class);
        DataReaderQos keepAll = Qos.reader().withPolicy(Qos.policies().History().withKeepAll());
        DataWriterQos keepAllWriter = Qos.writer().withPolicy(Qos.policies().History().withKeepAll());
        DataWriter<byte[]> writer = Qos.writer(participant.createPublisher(), topic, org.zerodds.cdr.TypeSupportResolver.resolve(byte[].class), keepAllWriter);
        DataReader<byte[]> reader = Qos.reader(participant.createSubscriber(), topic, org.zerodds.cdr.TypeSupportResolver.resolve(byte[].class), keepAll);

        writer.write(new byte[] {1});
        writer.write(new byte[] {2});
        Qos.dispose(writer, new byte[] {0});
        List<Sample<byte[]>> got = Qos.read(reader);
        assertEquals(3, got.size());
        for (Sample<byte[]> s : got) {
            assertEquals(InstanceState.NOT_ALIVE_DISPOSED, s.getInstanceState(),
                    "instance_state is a snapshot shared by all samples of the instance");
        }

        participant.close();
    }
}
