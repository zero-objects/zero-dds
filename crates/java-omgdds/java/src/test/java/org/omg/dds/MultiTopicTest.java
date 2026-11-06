// SPDX-License-Identifier: Apache-2.0
package org.omg.dds;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.omg.dds.domain.DomainParticipant;
import org.omg.dds.pub.DataWriter;
import org.omg.dds.pub.Publisher;
import org.omg.dds.sub.DataReader;
import org.omg.dds.sub.Sample;
import org.omg.dds.sub.Subscriber;
import org.omg.dds.topic.MultiTopic;
import org.omg.dds.topic.Topic;
import org.omg.dds.topic.TopicDescription;
import org.zerodds.internal.InProcessBus;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertThrows;

/**
 * DDS 1.2 §2.2.2.3.4: {@code create_multi_topic} and what a DataReader on a
 * MultiTopic receives for the single-source form; the joins are in
 * {@link MultiTopicJoinTest}.
 */
class MultiTopicTest {

    private static int dom = 100;

    private static int nextDom() {
        return dom++;
    }

    /** A bean with the members the expressions of the test read. */
    public static final class Reading {
        private final String sensor;
        private final double value;

        public Reading(String sensor, double value) {
            this.sensor = sensor;
            this.value = value;
        }

        public String getSensor() {
            return sensor;
        }

        public double getValue() {
            return value;
        }
    }

    @BeforeEach
    void setUp() {
        InProcessBus.instance().reset();
    }

    @AfterEach
    void tearDown() {
        InProcessBus.instance().reset();
    }

    @Test
    void javapsm_2_the_multi_topic_carries_its_expression() {
        DomainParticipant participant = Qos.factory().createParticipant(nextDom());
        Topic<Reading> topic = participant.createTopic("Reading", Reading.class);

        MultiTopic<Reading> multiTopic = participant.createMultiTopic("Hot",
                Reading.class, "SELECT * FROM Reading WHERE value > 50",
                List.of());
        assertEquals("Hot", multiTopic.getName());
        assertEquals("SELECT * FROM Reading WHERE value > 50",
                multiTopic.getSubscriptionExpression());
        assertEquals(List.of(), multiTopic.getExpressionParameters());
        assertEquals(topic.getTypeName(), multiTopic.getTypeName());
        assertSame(participant, multiTopic.getParent());
        assertNotNull(multiTopic.getTypeSupport());
        assertEquals("Hot", multiTopic.<Object>cast().getName());
    }

    @Test
    void javapsm_2_the_expression_parameters_are_replaced() {
        DomainParticipant participant = Qos.factory().createParticipant(nextDom());
        participant.createTopic("Reading", Reading.class);
        MultiTopic<Reading> multiTopic = participant.createMultiTopic("Hot",
                Reading.class, "SELECT * FROM Reading WHERE value > %0",
                List.of("50"));
        assertEquals(List.of("50"), multiTopic.getExpressionParameters());

        multiTopic.setExpressionParameters(List.of("500"));
        assertEquals(List.of("500"), multiTopic.getExpressionParameters());
    }

    @Test
    void javapsm_2_a_reader_on_the_multi_topic_receives_the_matching_samples() throws Exception {
        DomainParticipant participant = Qos.factory().createParticipant(nextDom());
        Topic<Reading> topic = participant.createTopic("Reading", Reading.class);
        Publisher publisher = participant.createPublisher();
        Subscriber subscriber = participant.createSubscriber();
        DataWriter<Reading> writer = publisher.createDataWriter(topic);

        MultiTopic<Reading> multiTopic = participant.createMultiTopic("Hot",
                Reading.class, "SELECT * FROM Reading WHERE value > 50",
                List.of());
        DataReader<Reading> reader = subscriber.createDataReader(multiTopic);

        writer.write(new Reading("a", 10));
        writer.write(new Reading("b", 99));
        List<Sample<Reading>> got = Qos.take(reader);
        assertEquals(1, got.size());
        assertEquals(99.0, got.get(0).getData().getValue());
    }

    @Test
    void javapsm_2_the_reader_reports_the_multi_topic_as_its_description() {
        DomainParticipant participant = Qos.factory().createParticipant(nextDom());
        participant.createTopic("Reading", Reading.class);
        MultiTopic<Reading> multiTopic = participant.createMultiTopic("Hot",
                Reading.class, "SELECT * FROM Reading WHERE value > 50",
                List.of());
        DataReader<Reading> reader = participant.createSubscriber().createDataReader(multiTopic);
        assertSame(multiTopic, reader.getTopicDescription());
    }

    @Test
    void javapsm_2_the_multi_topic_is_a_topic_description_of_the_participant() {
        DomainParticipant participant = Qos.factory().createParticipant(nextDom());
        participant.createTopic("Reading", Reading.class);
        MultiTopic<Reading> multiTopic = participant.createMultiTopic("Hot",
                Reading.class, "SELECT * FROM Reading WHERE value > 50",
                List.of());
        TopicDescription<?> found =
                participant.lookupTopicDescription("Hot");
        assertSame(multiTopic, found);
    }
}
