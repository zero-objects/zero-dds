// SPDX-License-Identifier: Apache-2.0
package org.omg.dds;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.omg.dds.core.ImmutablePolicyException;
import org.omg.dds.core.InconsistentPolicyException;
import org.omg.dds.core.PreconditionNotMetException;
import org.omg.dds.domain.DomainParticipant;
import org.omg.dds.pub.DataWriter;
import org.omg.dds.pub.DataWriterQos;
import org.omg.dds.pub.Publisher;
import org.omg.dds.sub.DataReader;
import org.omg.dds.sub.DataReaderQos;
import org.omg.dds.sub.Subscriber;
import org.omg.dds.topic.ContentFilteredTopic;
import org.omg.dds.topic.Topic;
import org.omg.dds.topic.TopicDescription;
import org.omg.dds.topic.TopicQos;
import org.zerodds.cdr.ByteArrayTypeSupport;
import org.zerodds.domain.DomainParticipantImpl;
import org.zerodds.internal.InProcessBus;
import org.zerodds.pub.DataWriterImpl;
import org.zerodds.sub.SubscriberImpl;

import java.lang.reflect.Proxy;
import java.util.Collections;
import java.util.concurrent.TimeUnit;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertThrows;

/**
 * DDS-Java-PSM 1.0 Table 7.1 and DDS 1.4 §2.2.3: RETCODE_IMMUTABLE_POLICY,
 * RETCODE_INCONSISTENT_POLICY, RETCODE_UNSUPPORTED and RETCODE_PRECONDITION_NOT_MET
 * as the exceptions of the entities.
 */
class CoreQosRulesTest {

    private static final int DOMAIN = 1505;

    private DomainParticipant participant;
    private Topic<byte[]> topic;
    private Publisher publisher;
    private Subscriber subscriber;
    private DataWriter<byte[]> writer;
    private DataReader<byte[]> reader;

    @BeforeEach
    void setUp() {
        InProcessBus.instance().reset();
        participant = Qos.factory().createParticipant(DOMAIN);
        topic = participant.createTopic("CoreQosRules", byte[].class);
        publisher = participant.createPublisher();
        subscriber = participant.createSubscriber();
        writer = publisher.createDataWriter(topic);
        reader = subscriber.createDataReader(topic);
    }

    @AfterEach
    void tearDown() {
        participant.close();
        InProcessBus.instance().reset();
    }

    // ---- RETCODE_IMMUTABLE_POLICY -------------------------------------------------

    // ---- RETCODE_INCONSISTENT_POLICY ---------------------------------------------

    private static DataWriterQos writerQos(int depth, int maxSamples, int maxPerInstance) {
        return Qos.writer()
                .withPolicy(Qos.pf().History().withKeepLast(depth))
                .withPolicy(Qos.pf().ResourceLimits().withMaxSamples(maxSamples)
                        .withMaxSamplesPerInstance(maxPerInstance));
    }

    @Test
    void javapsm_7_3_2_a_history_depth_above_max_samples_per_instance_is_inconsistent() {
        DataWriterQos before = writer.getQos();
        DataWriterQos inconsistent = writerQos(10, 100, 5);
        assertThrows(InconsistentPolicyException.class, () -> publisher.createDataWriter(topic, inconsistent));
        assertThrows(InconsistentPolicyException.class, () -> writer.setQos(inconsistent));
        // the refused QoS did not replace the QoS of the writer
        assertEquals(before, writer.getQos());
        // the depth of 5 against a limit of 5 is consistent
        assertNotNull(publisher.createDataWriter(topic, writerQos(5, 100, 5)));
    }

    @Test
    void javapsm_7_3_2_max_samples_below_max_samples_per_instance_is_inconsistent() {
        DataWriterQos inconsistent = writerQos(1, 4, 5);
        assertThrows(InconsistentPolicyException.class, () -> publisher.createDataWriter(topic, inconsistent));
        DataReaderQos readerQos = Qos.reader().withPolicy(Qos.pf().ResourceLimits()
                .withMaxSamples(4).withMaxSamplesPerInstance(5));
        assertThrows(InconsistentPolicyException.class, () -> subscriber.createDataReader(topic, readerQos));
        assertNotNull(publisher.createDataWriter(topic, writerQos(1, 5, 5)));
    }

    // ---- RETCODE_UNSUPPORTED -------------------------------------------------------

    @SuppressWarnings("unchecked")
    private static <T> T foreign(Class<T> type) {
        return (T) Proxy.newProxyInstance(type.getClassLoader(), new Class<?>[] {type},
                (proxy, method, args) -> null);
    }

    @Test
    void javapsm_7_3_2_a_topic_of_another_implementation_is_unsupported() {
        Topic<byte[]> other = foreign(Topic.class);
        assertThrows(UnsupportedOperationException.class, () -> publisher.createDataWriter(other));
    }

    @Test
    void javapsm_7_3_2_a_topic_description_of_another_implementation_is_unsupported() {
        TopicDescription<byte[]> other = foreign(TopicDescription.class);
        assertThrows(UnsupportedOperationException.class, () -> subscriber.createDataReader(other));
        ContentFilteredTopic<byte[]> filtered = foreign(ContentFilteredTopic.class);
        assertThrows(UnsupportedOperationException.class,
                () -> ((SubscriberImpl) subscriber).createDataReader(filtered, ByteArrayTypeSupport.INSTANCE));
    }

    // ---- RETCODE_PRECONDITION_NOT_MET of the publisher and the subscriber ---------

}
