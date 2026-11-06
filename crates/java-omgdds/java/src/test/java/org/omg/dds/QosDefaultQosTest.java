// SPDX-License-Identifier: Apache-2.0
package org.omg.dds;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.omg.dds.core.InconsistentPolicyException;
import org.omg.dds.domain.DomainParticipant;
import org.omg.dds.domain.DomainParticipantFactory;
import org.omg.dds.domain.DomainParticipantQos;
import org.omg.dds.pub.DataWriter;
import org.omg.dds.pub.DataWriterQos;
import org.omg.dds.pub.PublisherQos;
import org.omg.dds.sub.DataReader;
import org.omg.dds.sub.DataReaderQos;
import org.omg.dds.sub.SubscriberQos;
import org.omg.dds.topic.Topic;
import org.omg.dds.topic.TopicQos;
import org.zerodds.core.qos.DomainParticipantQosImpl;
import org.zerodds.internal.InProcessBus;

import java.util.Arrays;
import java.util.concurrent.TimeUnit;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertThrows;

/**
 * The factories hold a default QoS for what they create (DDS 1.4 §2.2.2.2.1.7 and its
 * counterparts); DDS-Java-PSM 1.0 §7.3.5.4 has each of them set those defaults from QoS objects.
 * Domain ids 1400 to 1499.
 */
class QosDefaultQosTest {

    @BeforeEach
    void setUp() {
        InProcessBus.instance().reset();
    }

    @AfterEach
    void tearDown() {
        InProcessBus.instance().reset();
    }

    @Test
    void javapsm_7_3_5_4_the_default_topic_qos_of_the_participant_is_the_qos_of_the_topics_it_creates_next() throws Exception {
        try (QosFixtures.Rig rig = new QosFixtures.Rig()) {
            TopicQos mine = Qos.topic().withPolicy(Qos.pf().TransportPriority().withValue(6));
            rig.participant.setDefaultTopicQos(mine);
            assertSame(mine, rig.participant.getDefaultTopicQos());
            Topic<QosFixtures.Item> topic = rig.participant.createTopic("Another", QosFixtures.Item.class);
            assertEquals(mine, topic.getQos());
        }
    }

    @Test
    void javapsm_7_3_5_4_the_default_subscriber_qos_of_the_participant_is_the_qos_of_the_subscribers_it_creates_next() throws Exception {
        try (QosFixtures.Rig rig = new QosFixtures.Rig()) {
            SubscriberQos mine = Qos.subscriber().withPolicy(Qos.pf().Partition().withName(Arrays.asList("P")));
            rig.participant.setDefaultSubscriberQos(mine);
            assertSame(mine, rig.participant.getDefaultSubscriberQos());
            assertEquals(mine, rig.participant.createSubscriber().getQos());
        }
    }

    @Test
    void javapsm_7_3_5_4_the_default_publisher_qos_of_the_participant_is_the_qos_of_the_publishers_it_creates_next() throws Exception {
        try (QosFixtures.Rig rig = new QosFixtures.Rig()) {
            PublisherQos mine = Qos.publisher().withPolicy(Qos.pf().Partition().withName(Arrays.asList("Q")));
            rig.participant.setDefaultPublisherQos(mine);
            assertSame(mine, rig.participant.getDefaultPublisherQos());
            assertEquals(mine, rig.participant.createPublisher().getQos());
        }
    }

    @Test
    void javapsm_7_3_5_4_the_default_data_writer_qos_of_the_publisher_is_the_qos_of_the_writers_it_creates_next() throws Exception {
        try (QosFixtures.Rig rig = new QosFixtures.Rig()) {
            DataWriterQos mine = Qos.writer().withPolicy(Qos.pf().OwnershipStrength().withValue(12));
            rig.publisher.setDefaultDataWriterQos(mine);
            assertSame(mine, rig.publisher.getDefaultDataWriterQos());
            DataWriter<QosFixtures.Item> writer = Qos.writer(rig.publisher, rig.topic, QosFixtures.SUPPORT);
            assertEquals(mine, writer.getQos());
            assertEquals(12, writer.getQos().getOwnershipStrength().getValue());
            // a writer created with a QoS of its own does not take the default
            DataWriter<QosFixtures.Item> own = rig.writer(Qos.writer());
            assertEquals(0, own.getQos().getOwnershipStrength().getValue());
        }
    }

    @Test
    void javapsm_7_3_5_4_the_default_data_reader_qos_of_the_subscriber_is_the_qos_of_the_readers_it_creates_next() throws Exception {
        try (QosFixtures.Rig rig = new QosFixtures.Rig()) {
            DataReaderQos mine = Qos.reader().withPolicy(
                    Qos.pf().TimeBasedFilter().withMinimumSeparation(30, TimeUnit.MILLISECONDS));
            rig.subscriber.setDefaultDataReaderQos(mine);
            assertSame(mine, rig.subscriber.getDefaultDataReaderQos());
            DataReader<QosFixtures.Item> reader = Qos.reader(rig.subscriber, rig.topic, QosFixtures.SUPPORT);
            assertEquals(mine, reader.getQos());
        }
    }

    @Test
    void javapsm_7_3_5_4_a_default_qos_that_contradicts_itself_is_refused_and_the_default_stays() throws Exception {
        try (QosFixtures.Rig rig = new QosFixtures.Rig()) {
            DataWriterQos before = rig.publisher.getDefaultDataWriterQos();
            DataWriterQos inconsistent = Qos.writer()
                    .withPolicy(Qos.pf().History().withKeepLast(8))
                    .withPolicy(Qos.pf().ResourceLimits().withMaxSamplesPerInstance(2));
            assertThrows(InconsistentPolicyException.class,
                    () -> rig.publisher.setDefaultDataWriterQos(inconsistent));
            assertSame(before, rig.publisher.getDefaultDataWriterQos());

            DataReaderQos readerBefore = rig.subscriber.getDefaultDataReaderQos();
            DataReaderQos readerInconsistent = Qos.reader()
                    .withPolicy(Qos.pf().ResourceLimits().withMaxSamples(1).withMaxSamplesPerInstance(2));
            assertThrows(InconsistentPolicyException.class,
                    () -> rig.subscriber.setDefaultDataReaderQos(readerInconsistent));
            assertSame(readerBefore, rig.subscriber.getDefaultDataReaderQos());
        }
    }
}
