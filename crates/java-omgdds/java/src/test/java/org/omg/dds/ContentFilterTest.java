// SPDX-License-Identifier: Apache-2.0
package org.omg.dds;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.omg.dds.core.Condition;
import org.omg.dds.core.WaitSet;
import org.omg.dds.domain.DomainParticipant;
import org.omg.dds.pub.DataWriter;
import org.omg.dds.pub.Publisher;
import org.omg.dds.sub.DataReader;
import org.omg.dds.sub.QueryCondition;
import org.omg.dds.sub.Sample;
import org.omg.dds.sub.Subscriber;
import org.omg.dds.topic.ContentFilteredTopic;
import org.omg.dds.topic.Topic;
import org.zerodds.internal.InProcessBus;
import org.zerodds.sub.ContentFilter;

import java.util.List;
import java.util.function.Predicate;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * The content filter expressions of DDS 1.2 §2.2.2.5.3.8 / DDSI-RTPS 2.5
 * §9.4.1.1, as {@code createContentFilteredTopic}, {@code
 * createQueryCondition} and the read/take selector use them.
 */
class ContentFilterTest {

    private static int dom = 90;

    private static int nextDom() {
        return dom++;
    }

    /** A plain bean with a nested member, to resolve both spellings. */
    public static final class Reading {
        private final String sensor;
        private final double value;
        private final Inner inner;

        public Reading(String sensor, double value, Inner inner) {
            this.sensor = sensor;
            this.value = value;
            this.inner = inner;
        }

        public String getSensor() {
            return sensor;
        }

        public double getValue() {
            return value;
        }

        public Inner getInner() {
            return inner;
        }

        /** A record: its accessors are named like the members. */
        public record Inner(String unit, Integer channel) {
        }
    }

    private static final Reading HOT =
            new Reading("a", 99.0, new Reading.Inner("C", 3));
    private static final Reading COLD =
            new Reading("b", -1.0, new Reading.Inner("C", null));
    /** COLD as it is written: a member that is not optional has a value (a null cannot be serialized). */
    private static final Reading COLD_SENT =
            new Reading("b", -1.0, new Reading.Inner("C", 0));

    @BeforeEach
    void setUp() {
        InProcessBus.instance().reset();
    }

    @AfterEach
    void tearDown() {
        InProcessBus.instance().reset();
    }

    private static boolean matches(String expression, List<String> parameters,
            Object sample) {
        return ContentFilter.compile(expression, parameters).test(sample);
    }

    @Test
    void comparisonOperators() {
        assertTrue(matches("value > 50", null, HOT));
        assertFalse(matches("value > 50", null, COLD));
        assertTrue(matches("value >= 99", null, HOT));
        assertTrue(matches("value <= 99", null, HOT));
        assertTrue(matches("value = 99", null, HOT));
        assertTrue(matches("value != 99", null, COLD));
        assertTrue(matches("value <> 99", null, COLD));
        assertTrue(matches("sensor = 'a'", null, HOT));
        assertTrue(matches("sensor != 'a'", null, COLD));
        // A string on one side and a number on the other compares as text.
        assertTrue(matches("sensor < 'c'", null, HOT));
    }

    @Test
    void nestedMembersAndRecordAccessors() {
        assertTrue(matches("inner.channel = 3", null, HOT));
        assertTrue(matches("inner.unit = 'C'", null, HOT));
        assertFalse(matches("inner.channel IS NULL", null, HOT));
        assertTrue(matches("inner.channel IS NULL", null, COLD));
        assertTrue(matches("inner.channel IS NOT NULL", null, HOT));
    }

    @Test
    void booleanOperators() {
        assertTrue(matches("value > 0 AND sensor = 'a'", null, HOT));
        assertFalse(matches("value > 0 AND sensor = 'b'", null, HOT));
        assertTrue(matches("value > 0 OR sensor = 'b'", null, COLD));
        assertTrue(matches("NOT (value > 0)", null, COLD));
        assertFalse(matches("NOT (value > 0)", null, HOT));
        assertTrue(matches("value < 0 OR (value > 50 AND sensor = 'a')", null, HOT));
    }

    @Test
    void betweenInAndLike() {
        assertTrue(matches("value BETWEEN 0 AND 100", null, HOT));
        assertFalse(matches("value BETWEEN 0 AND 10", null, HOT));
        assertTrue(matches("sensor IN ('a', 'c')", null, HOT));
        assertFalse(matches("sensor IN ('b', 'c')", null, HOT));
        assertTrue(matches("sensor LIKE 'a%'", null, HOT));
        assertFalse(matches("sensor LIKE 'a_'", null, HOT));
        assertTrue(matches("sensor LIKE 'a'", null, HOT));
        // LIKE: '%' is any string and '_' exactly one character
        assertTrue(matches("sensor LIKE '_'", null, HOT));
        assertFalse(matches("sensor LIKE '__'", null, HOT));
        assertTrue(matches("sensor LIKE '%' ESCAPE '\\'", null, HOT));
    }

    @Test
    void expressionParameters() {
        assertTrue(matches("value > %0", List.of("50"), HOT));
        assertFalse(matches("value > %0", List.of("500"), HOT));
        assertTrue(matches("sensor = %0", List.of("a"), HOT));
        assertTrue(matches("value BETWEEN %0 AND %1", List.of("0", "100"), HOT));
        assertThrows(IllegalArgumentException.class,
                () -> matches("value > %1", List.of("50"), HOT));
    }

    @Test
    void anEmptyExpressionMatchesEverySample() {
        assertTrue(matches(null, null, HOT));
        assertTrue(matches("", null, HOT));
        assertTrue(matches("   ", null, HOT));
    }

    /** DDS-DCPS 1.4 Annex B.2: FIELDNAME RelOp FIELDNAME and Parameter RelOp FIELDNAME. */
    @Test
    void aFieldCanBeComparedWithAnotherFieldOrWithALiteralOnTheLeft() {
        // HOT: value 99.0, inner.channel 3.
        assertTrue(matches("value > inner.channel", null, HOT));
        assertFalse(matches("value < inner.channel", null, HOT));
        assertTrue(matches("sensor = sensor", null, HOT));
        assertTrue(matches("10 < value", null, HOT));
        assertFalse(matches("100 < value", null, HOT));
        assertTrue(matches("%0 <= value", List.of("99"), HOT));
        assertFalse(matches("%0 > value", List.of("99"), HOT));
        assertTrue(matches("'a' = sensor", null, HOT));
    }

    @Test
    void aMalformedExpressionIsRejected() {
        assertThrows(IllegalArgumentException.class,
                () -> matches("value >", null, HOT));
        assertThrows(IllegalArgumentException.class,
                () -> matches("value > 1 AND", null, HOT));
        assertThrows(IllegalArgumentException.class,
                () -> matches("value > 1)", null, HOT));
        assertThrows(IllegalArgumentException.class,
                () -> matches("nosuchmember > 1", null, HOT));
        assertThrows(IllegalArgumentException.class,
                () -> matches("value = 'unterminated", null, HOT));
    }

    @Test
    void theContentFilteredTopicOfTheParticipantFiltersTheSamples() throws Exception {
        DomainParticipant participant = Qos.factory().createParticipant(nextDom());
        Topic<Reading> topic = participant.createTopic("Reading", Reading.class);
        Publisher publisher = participant.createPublisher();
        Subscriber subscriber = participant.createSubscriber();
        DataWriter<Reading> writer = publisher.createDataWriter(topic);

        ContentFilteredTopic<Reading> hot =
                participant.createContentFilteredTopic("Hot", topic,
                        "value > 50", List.of());
        assertEquals("Hot", hot.getName());
        assertEquals("value > 50", hot.getFilterExpression());
        assertEquals(topic, hot.getRelatedTopic());

        DataReader<Reading> reader = subscriber.createDataReader(hot);
        writer.write(COLD_SENT);
        writer.write(HOT);
        List<Sample<Reading>> got = Qos.take(reader);
        assertEquals(1, got.size());
        assertEquals(99.0, got.get(0).getData().getValue());
    }

    @Test
    void theQueryConditionOfTheReaderTriggersOnAMatchingSample() throws Exception {
        DomainParticipant participant = Qos.factory().createParticipant(nextDom());
        Topic<Reading> topic = participant.createTopic("Reading", Reading.class);
        Publisher publisher = participant.createPublisher();
        Subscriber subscriber = participant.createSubscriber();
        DataReader<Reading> reader = subscriber.createDataReader(topic);
        DataWriter<Reading> writer = publisher.createDataWriter(topic);

        QueryCondition<Reading> condition =
                reader.createQueryCondition("value > 50", List.of());
        assertFalse(condition.getTriggerValue());
        WaitSet waitSet = WaitSet.newWaitSet(reader.getEnvironment());
        waitSet.attachCondition(condition);
        try {
            writer.write(COLD_SENT);
            assertFalse(condition.getTriggerValue(),
                    "the sample does not satisfy the expression");
            writer.write(HOT);
            assertTrue(condition.getTriggerValue());
        } finally {
            waitSet.detachCondition(condition);
            condition.close();
        }
    }

    @Test
    void theSelectorOfReadAndTakeCarriesTheExpression() throws Exception {
        DomainParticipant participant = Qos.factory().createParticipant(nextDom());
        Topic<Reading> topic = participant.createTopic("Reading", Reading.class);
        Publisher publisher = participant.createPublisher();
        Subscriber subscriber = participant.createSubscriber();
        DataReader<Reading> reader = subscriber.createDataReader(topic);
        DataWriter<Reading> writer = publisher.createDataWriter(topic);
        writer.write(COLD_SENT);
        writer.write(HOT);

        var selector = reader.<Reading>select().Content("value > 50", List.of());
        List<Sample<Reading>> got = new java.util.ArrayList<>();
        selector.read(got);
        assertEquals(1, got.size());
        assertEquals(99.0, got.get(0).getData().getValue());
    }

    @Test
    void aConditionKnowsTheFilterThatTriggersIt() {
        DomainParticipant participant = Qos.factory().createParticipant(nextDom());
        Topic<Reading> topic = participant.createTopic("Reading", Reading.class);
        DataReader<Reading> reader = participant.createSubscriber()
                .createDataReader(topic);
        QueryCondition<Reading> condition =
                reader.createQueryCondition("value > %0", List.of("50"));
        assertEquals("value > %0", condition.getQueryExpression());
        assertEquals(List.of("50"), condition.getQueryParameters());
        assertTrue(condition instanceof Condition);
    }

    @Test
    void aPredicateFromTheFilterIsReusable() {
        Predicate<Reading> filter =
                ContentFilter.compile("sensor = 'a'", List.of());
        assertTrue(filter.test(HOT));
        assertFalse(filter.test(COLD));
        assertFalse(filter.test(null),
                "a sample without data has no members");
    }
}
