// SPDX-License-Identifier: Apache-2.0
package org.omg.dds;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.omg.dds.core.policy.Reliability;
import org.omg.dds.pub.DataWriterQos;
import org.omg.dds.sub.DataReaderQos;
import org.omg.dds.domain.DomainParticipant;
import org.omg.dds.domain.DomainParticipantFactory;
import org.omg.dds.pub.DataWriter;
import org.omg.dds.sub.DataReader;
import org.omg.dds.topic.Topic;
import org.zerodds.cdr.TopicTypeSupport;
import org.zerodds.internal.InProcessBus;
import org.zerodds.internal.Xcdr2Codec;

import java.nio.ByteBuffer;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * DDS 1.2 §7.1.3 — a writer and a reader communicate only on a matching topic
 * name *and* type name, and only when their RxO policies are compatible.
 */
class QosCompatibilityTest {

    private static int dom = 60;

    private static int nextDom() {
        return dom++;
    }

    record A(int v) {}
    record B(int v) {}

    static final TopicTypeSupport<A> SUPPORT_A = Qos.support("TypeA",
            (A a, ByteBuffer buf) -> Xcdr2Codec.writeInt(buf, a.v()),
            (ByteBuffer buf) -> new A(Xcdr2Codec.readInt(buf)));

    static final TopicTypeSupport<B> SUPPORT_B = Qos.support("TypeB",
            (B b, ByteBuffer buf) -> Xcdr2Codec.writeInt(buf, b.v()),
            (ByteBuffer buf) -> new B(Xcdr2Codec.readInt(buf)));

    @BeforeEach
    void setUp() throws Exception {
        InProcessBus.instance().reset();
    }

    @AfterEach
    void tearDown() throws Exception {
        InProcessBus.instance().reset();
    }

    @Test
    void differentTypeNamesDoNotConnect() throws Exception {
        DomainParticipant participant =
                Qos.factory().createParticipant(nextDom());
        Topic<A> ta = participant.createTopic("sameName", A.class);
        Topic<B> tb = participant.createTopic("sameName", B.class);
        DataWriter<A> writer = Qos.writer(participant.createPublisher(), ta, SUPPORT_A);
        DataReader<B> reader = Qos.reader(participant.createSubscriber(), tb, SUPPORT_B);

        writer.write(new A(1));
        assertEquals(0, Qos.take(reader).size(),
                "a reader must not receive samples of a different type name");

        participant.close();
    }

    @Test
    void incompatibleReliabilityDoesNotConnect() throws Exception {
        DomainParticipant participant =
                Qos.factory().createParticipant(nextDom());
        Topic<A> topic = participant.createTopic("reliability", A.class);
        DataReaderQos reliableReader = Qos.reader().withPolicy(
                Qos.policies().DataReaderReliability().withReliable());
        DataWriterQos bestEffortWriter = Qos.writer().withPolicy(
                Qos.policies().DataWriterReliability().withBestEffort());
        DataReader<A> reader = Qos.reader(participant.createSubscriber(), topic, SUPPORT_A, reliableReader);
        DataWriter<A> writer = Qos.writer(participant.createPublisher(), topic, SUPPORT_A, bestEffortWriter);

        writer.write(new A(1));
        List<org.omg.dds.sub.Sample<A>> samples = Qos.take(reader);
        assertEquals(0, samples.size(),
                "a RELIABLE reader must not receive a BEST_EFFORT writer's samples");
        assertTrue(reader.getRequestedIncompatibleQosStatus().getTotalCount() >= 1,
                "REQUESTED_INCOMPATIBLE_QOS is recorded");
        assertTrue(writer.getOfferedIncompatibleQosStatus().getTotalCount() >= 1,
                "OFFERED_INCOMPATIBLE_QOS is recorded");

        participant.close();
    }
}
