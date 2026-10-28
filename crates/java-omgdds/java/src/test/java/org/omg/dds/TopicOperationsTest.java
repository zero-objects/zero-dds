// SPDX-License-Identifier: Apache-2.0
package org.omg.dds;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.omg.dds.core.AlreadyClosedException;
import org.omg.dds.core.PreconditionNotMetException;
import org.omg.dds.core.StatusCondition;
import org.omg.dds.core.event.InconsistentTopicEvent;
import org.omg.dds.core.status.InconsistentTopicStatus;
import org.omg.dds.domain.DomainParticipant;
import org.omg.dds.domain.DomainParticipantAdapter;
import org.omg.dds.pub.DataWriter;
import org.omg.dds.sub.DataReader;
import org.omg.dds.sub.Sample;
import org.omg.dds.sub.Subscriber;
import org.omg.dds.topic.ContentFilteredTopic;
import org.omg.dds.topic.MultiTopic;
import org.omg.dds.topic.PublicationBuiltinTopicData;
import org.omg.dds.topic.Topic;
import org.omg.dds.topic.TopicAdapter;
import org.omg.dds.type.TypeSupport;
import org.zerodds.internal.InProcessBus;
import org.zerodds.topic.BuiltinTopics;
import org.zerodds.topic.PublicationBuiltinTopicDataImpl;

import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.TimeoutException;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.concurrent.atomic.AtomicReference;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * Topic, ContentFilteredTopic, MultiTopic and TypeSupport (DDS 1.4 §2.2.2.3 and DDS-Java-PSM 1.0
 * §7.5): INCONSISTENT_TOPIC and the topic listener, closing with users, the expression
 * parameters, filtering over a supertype, and the TypeSupport a topic is created from.
 * Domain ids 1290 to 1307.
 */
class TopicOperationsTest {

    /** A bean of the topics of these tests. */
    public static class Foo {
        private int level;

        public int getLevel() {
            return level;
        }

        public void setLevel(int level) {
            this.level = level;
        }
    }

    /** A subtype of {@link Foo}: a ContentFilteredTopic of Foo can read its samples. */
    public static class Bar extends Foo {
        private String label;

        public String getLabel() {
            return label;
        }

        public void setLabel(String label) {
            this.label = label;
        }
    }

    private static Foo foo(int level) {
        Foo foo = new Foo();
        foo.setLevel(level);
        return foo;
    }

    private static Bar bar(int level, String label) {
        Bar bar = new Bar();
        bar.setLevel(level);
        bar.setLabel(label);
        return bar;
    }

    @BeforeEach
    void setUp() {
        InProcessBus.instance().reset();
    }

    @AfterEach
    void tearDown() {
        InProcessBus.instance().reset();
    }

    // --- INCONSISTENT_TOPIC (§2.2.4.1) and the TopicListener (§2.2.4.3) ----------------

    @Test
    void aTopicOfTheSameNameAndAnotherTypeInTheDomainIsInconsistentAndTellsTheTopicListener() {
        int domain = 1290;
        DomainParticipant first = Qos.factory().createParticipant(domain);
        DomainParticipant second = Qos.factory().createParticipant(domain);
        AtomicInteger calls = new AtomicInteger();
        AtomicReference<InconsistentTopicEvent<String>> event = new AtomicReference<>();
        Topic<String> topic = first.createTopic("Inconsistent", String.class,
                first.getDefaultTopicQos(), new TopicAdapter<String>() {
                    @Override
                    public void onInconsistentTopic(InconsistentTopicEvent<String> status) {
                        calls.incrementAndGet();
                        event.set(status);
                    }
                }, null);
        assertEquals(0, topic.getInconsistentTopicStatus().getTotalCount());

        Topic<byte[]> other = second.createTopic("Inconsistent", byte[].class);
        assertEquals(1, calls.get());
        assertSame(topic, event.get().getSource());
        assertEquals(1, event.get().getStatus().getTotalCount());
        assertEquals(1, event.get().getStatus().getTotalCountChange());

        // the topic of the second participant is inconsistent with the first one, too
        assertTrue(other.getStatusChanges().contains(InconsistentTopicStatus.class));
        InconsistentTopicStatus status = other.getInconsistentTopicStatus();
        assertEquals(1, status.getTotalCount());
        assertEquals(1, status.getTotalCountChange());
        // reading the status resets the change and the changed flag
        assertFalse(other.getStatusChanges().contains(InconsistentTopicStatus.class));
        assertEquals(0, other.getInconsistentTopicStatus().getTotalCountChange());
        assertEquals(1, other.getInconsistentTopicStatus().getTotalCount());
    }

    @Test
    void aTopicWithoutListenerPassesInconsistentTopicToTheParticipantListener() {
        int domain = 1291;
        AtomicReference<InconsistentTopicEvent<?>> seen = new AtomicReference<>();
        DomainParticipant first = Qos.factory().createParticipant(domain,
                Qos.factory().getDefaultParticipantQos(), new DomainParticipantAdapter() {
                    @Override
                    public void onInconsistentTopic(InconsistentTopicEvent<?> status) {
                        seen.set(status);
                    }
                }, null);
        Topic<String> topic = first.createTopic("InconsistentUp", String.class);
        Qos.factory().createParticipant(domain).createTopic("InconsistentUp", byte[].class);
        assertNotNull(seen.get());
        assertSame(topic, seen.get().getSource());
        assertEquals(1, seen.get().getStatus().getTotalCount());
    }

    @Test
    void anInconsistentTopicTriggersItsStatusCondition() {
        int domain = 1292;
        DomainParticipant first = Qos.factory().createParticipant(domain);
        Topic<String> topic = first.createTopic("InconsistentCondition", String.class);
        StatusCondition<Topic<String>> condition = topic.getStatusCondition();
        assertFalse(condition.getTriggerValue());
        Qos.factory().createParticipant(domain).createTopic("InconsistentCondition", byte[].class);
        assertTrue(condition.getTriggerValue());
        topic.getInconsistentTopicStatus();
        assertFalse(condition.getTriggerValue());
    }

    @Test
    void topicsOfTheSameTypeOrInAnotherDomainAreConsistent() {
        DomainParticipant first = Qos.factory().createParticipant(1293);
        Topic<String> topic = first.createTopic("Consistent", String.class);
        Qos.factory().createParticipant(1293).createTopic("Consistent", String.class);
        Qos.factory().createParticipant(1294).createTopic("Consistent", byte[].class);
        first.createTopic("OtherName", byte[].class);
        assertEquals(0, topic.getInconsistentTopicStatus().getTotalCount());
        assertTrue(topic.getStatusChanges().isEmpty());
    }

    // --- TopicDescription: name, type name, participant (§2.2.2.3.1) ------------------------

    @Test
    void aTopicAndAContentFilteredTopicKnowTheirNameTypeAndParticipant() {
        DomainParticipant participant = Qos.factory().createParticipant(1295);
        Topic<Foo> topic = participant.createTopic("Described", Foo.class);
        ContentFilteredTopic<Foo> filtered = participant.createContentFilteredTopic(
                "DescribedView", topic, "level > 1", new ArrayList<>());
        assertEquals("Described", topic.getName());
        assertSame(participant, topic.getParent());
        assertEquals(topic.getTypeName(), filtered.getTypeName());
        assertEquals("DescribedView", filtered.getName());
        assertSame(participant, filtered.getParent());
        assertSame(topic, filtered.getRelatedTopic());
        assertSame(filtered, participant.lookupTopicDescription("DescribedView"));
        assertSame(topic, participant.lookupTopicDescription("Described"));
    }

    // --- ContentFilteredTopic (§2.2.2.3.3) ----------------------------------------------

    @Test
    void setExpressionParametersFiltersWithTheNewParametersFromThenOn() throws Exception {
        DomainParticipant participant = Qos.factory().createParticipant(1296);
        Topic<Foo> topic = participant.createTopic("Parameters", Foo.class);
        DataWriter<Foo> writer = participant.createPublisher().createDataWriter(topic);
        ContentFilteredTopic<Foo> filtered = participant.createContentFilteredTopic(
                "ParametersView", topic, "level > %0", List.of("10"));
        assertEquals(List.of("10"), filtered.getExpressionParameters());
        DataReader<Foo> reader = participant.createSubscriber().createDataReader(filtered,
                Qos.readerKeepAll());

        writer.write(foo(5));
        writer.write(foo(15));
        List<Sample<Foo>> first = Qos.take(reader);
        assertEquals(1, first.size());
        assertEquals(15, first.get(0).getData().getLevel());

        filtered.setExpressionParameters(List.of("3"));
        assertEquals(List.of("3"), filtered.getExpressionParameters());
        writer.write(foo(5));
        writer.write(foo(2));
        List<Sample<Foo>> second = Qos.take(reader);
        assertEquals(1, second.size());
        assertEquals(5, second.get(0).getData().getLevel());

        // a parameter list the expression cannot take is refused and the old one stays
        assertThrows(IllegalArgumentException.class,
                () -> filtered.setExpressionParameters(new ArrayList<>()));
        assertEquals(List.of("3"), filtered.getExpressionParameters());
        writer.write(foo(4));
        assertEquals(1, Qos.take(reader).size());
    }

    @Test
    void aContentFilteredTopicOfASupertypeFiltersTheSamplesOfTheSubtypeByTheSupertypeFields()
            throws Exception {
        DomainParticipant participant = Qos.factory().createParticipant(1297);
        Topic<Bar> topic = participant.createTopic("Supertype", Bar.class);
        DataWriter<Bar> writer = participant.createPublisher().createDataWriter(topic);
        ContentFilteredTopic<Foo> filtered = participant.<Foo>createContentFilteredTopic(
                "SupertypeView", topic, "level > 5", new ArrayList<>());
        DataReader<Foo> reader = participant.createSubscriber().createDataReader(filtered,
                Qos.readerKeepAll());

        writer.write(bar(1, "low"));
        writer.write(bar(9, "high"));
        List<Sample<Foo>> samples = Qos.take(reader);
        assertEquals(1, samples.size());
        assertTrue(samples.get(0).getData() instanceof Bar);
        assertEquals(9, samples.get(0).getData().getLevel());
        assertEquals("high", ((Bar) samples.get(0).getData()).getLabel());
    }

    @Test
    void closingAContentFilteredTopicNeedsItsReadersGoneAndRemovesItFromTheParticipant() {
        DomainParticipant participant = Qos.factory().createParticipant(1298);
        Topic<Foo> topic = participant.createTopic("Closing", Foo.class);
        ContentFilteredTopic<Foo> filtered = participant.createContentFilteredTopic(
                "ClosingView", topic, "level > 1", new ArrayList<>());
        Subscriber subscriber = participant.createSubscriber();
        DataReader<Foo> reader = subscriber.createDataReader(filtered);

        // the related topic cannot go while the filtered topic exists, the filtered topic not
        // while a reader reads it (DDS 1.4 §2.2.2.3.2, §2.2.2.3.3)
        assertThrows(PreconditionNotMetException.class, topic::close);
        assertThrows(PreconditionNotMetException.class, filtered::close);
        assertSame(filtered, participant.lookupTopicDescription("ClosingView"));

        reader.close();
        filtered.close();
        filtered.close(); // idempotent
        assertNull(participant.lookupTopicDescription("ClosingView"));
        assertThrows(AlreadyClosedException.class, () -> subscriber.createDataReader(filtered));
        topic.close();
        assertNull(participant.lookupTopicDescription("Closing"));
    }

    @Test
    void aTopicDescriptionIsCloseableAndLeavesTheParticipantWhenTheResourceEnds() {
        DomainParticipant participant = Qos.factory().createParticipant(1299);
        Topic<Foo> topic = participant.createTopic("Resource", Foo.class);
        try (ContentFilteredTopic<Foo> filtered = participant.createContentFilteredTopic(
                "ResourceView", topic, "level > 1", new ArrayList<>())) {
            assertSame(filtered, participant.lookupTopicDescription("ResourceView"));
        }
        assertNull(participant.lookupTopicDescription("ResourceView"));
        try (MultiTopic<Foo> multi = participant.createMultiTopic("ResourceMulti", Foo.class,
                "SELECT * FROM Resource WHERE level > 1", new ArrayList<>())) {
            assertSame(multi, participant.lookupTopicDescription("ResourceMulti"));
        }
        assertNull(participant.lookupTopicDescription("ResourceMulti"));
        topic.close();
    }

    // --- MultiTopic (§2.2.2.3.4) ---------------------------------------------------------

    @Test
    void closingAMultiTopicNeedsItsReadersGoneAndFreesTheTopicsOfItsFromClause() {
        DomainParticipant participant = Qos.factory().createParticipant(1300);
        Topic<Foo> source = participant.createTopic("Gauges", Foo.class);
        MultiTopic<Foo> multi = participant.createMultiTopic("HotGauges", Foo.class,
                "SELECT * FROM Gauges WHERE level > 5", new ArrayList<>());
        Subscriber subscriber = participant.createSubscriber();
        DataReader<Foo> reader = subscriber.createDataReader(multi);
        assertSame(multi, participant.lookupTopicDescription("HotGauges"));

        assertThrows(PreconditionNotMetException.class, multi::close);
        assertThrows(PreconditionNotMetException.class, source::close);
        reader.close();
        // the MultiTopic still holds the topic of its FROM clause
        assertThrows(PreconditionNotMetException.class, source::close);
        multi.close();
        multi.close(); // idempotent
        assertNull(participant.lookupTopicDescription("HotGauges"));
        assertThrows(AlreadyClosedException.class, () -> subscriber.createDataReader(multi));
        source.close();
        assertNull(participant.lookupTopicDescription("Gauges"));
    }

    // --- delete_topic (§2.2.2.2.1.7) and delete_contained_entities ---------------------------

    @Test
    void aClosedTopicIsGoneFromLookupFindAndDiscovery() {
        int domain = 1301;
        DomainParticipant participant = Qos.factory().createParticipant(domain);
        DomainParticipant other = Qos.factory().createParticipant(domain);
        Topic<Foo> topic = participant.createTopic("GoneTopic", Foo.class);
        org.omg.dds.core.InstanceHandle handle = topic.getInstanceHandle();
        assertTrue(other.getDiscoveredTopics().contains(handle));

        topic.close();
        assertNull(participant.lookupTopicDescription("GoneTopic"));
        assertThrows(TimeoutException.class, () -> participant.findTopic("GoneTopic", 0, TimeUnit.SECONDS));
        assertFalse(participant.containsEntity(handle));
        assertFalse(other.getDiscoveredTopics().contains(handle));
        assertThrows(PreconditionNotMetException.class, () -> other.getDiscoveredTopicData(handle));
        // the name can be used again
        assertNotNull(participant.createTopic("GoneTopic", Foo.class));
    }

    @Test
    void deleteContainedEntitiesRemovesEveryTopicDescriptionFromTheLookups() {
        DomainParticipant participant = Qos.factory().createParticipant(1302);
        Topic<Foo> topic = participant.createTopic("Contained", Foo.class);
        participant.createContentFilteredTopic("ContainedView", topic, "level > 1", new ArrayList<>());
        participant.createMultiTopic("ContainedMulti", Foo.class,
                "SELECT * FROM Contained WHERE level > 1", new ArrayList<>());
        participant.createPublisher().createDataWriter(topic);
        participant.createSubscriber().createDataReader(topic);

        participant.closeContainedEntities();
        assertNull(participant.lookupTopicDescription("Contained"));
        assertNull(participant.lookupTopicDescription("ContainedView"));
        assertNull(participant.lookupTopicDescription("ContainedMulti"));
        assertTrue(participant.getDiscoveredTopics().isEmpty());
        assertFalse(participant.containsEntity(topic.getInstanceHandle()));
    }

    @Test
    void closingAParticipantWithFilteredTopicsMultiTopicsAndReadersDoesNotFail() {
        DomainParticipant participant = Qos.factory().createParticipant(1303);
        Topic<Foo> topic = participant.createTopic("CloseAll", Foo.class);
        Subscriber subscriber = participant.createSubscriber();
        subscriber.createDataReader(participant.createContentFilteredTopic(
                "CloseAllView", topic, "level > 1", new ArrayList<>()));
        subscriber.createDataReader(participant.createMultiTopic("CloseAllMulti", Foo.class,
                "SELECT * FROM CloseAll WHERE level > 1", new ArrayList<>()));
        participant.createPublisher().createDataWriter(topic);
        participant.getBuiltinSubscriber();
        participant.close();
        assertTrue(Qos.isClosed(participant));
    }

    // --- find_topic (§2.2.2.2.1.11) -------------------------------------------------------------

    @Test
    void findTopicWaitsForATopicAnotherThreadCreatesAndTimesOutOtherwise() throws Exception {
        DomainParticipant participant = Qos.factory().createParticipant(1304);
        Thread creator = new Thread(() -> {
            try {
                Thread.sleep(200);
            } catch (InterruptedException e) {
                return;
            }
            participant.createTopic("Late", byte[].class);
        });
        creator.start();
        Topic<byte[]> found = participant.findTopic("Late", 30, TimeUnit.SECONDS);
        creator.join();
        assertNotNull(found);
        assertEquals("Late", found.getName());

        long start = System.nanoTime();
        assertThrows(TimeoutException.class,
                () -> participant.findTopic("Never", 300, TimeUnit.MILLISECONDS));
        long elapsedMillis = (System.nanoTime() - start) / 1_000_000L;
        assertTrue(elapsedMillis >= 290, "the wait lasts the timeout: " + elapsedMillis);
    }

    // --- TypeSupport (DDS-Java-PSM 1.0 §7.5.1) ----------------------------------------------------

    @Test
    void aTopicMadeFromAClassHasATypeSupportOfThatClass() {
        DomainParticipant participant = Qos.factory().createParticipant(1307);
        Topic<Bar> topic = participant.createTopic("ClassTypeSupport", Bar.class);
        TypeSupport<Bar> support = topic.getTypeSupport();
        assertTrue(support instanceof TypeSupport);
        assertEquals(Bar.class, support.getType());
        assertEquals(topic.getTypeName(), support.getTypeName());
        assertTrue(support.newData() instanceof Bar);
        assertSame(participant.lookupTopicDescription("ClassTypeSupport").getTypeSupport(), support);
    }

    @Test
    void newTypeSupportWithoutANameRegistersUnderTheNameOfTheClass() throws Exception {
        DomainParticipant participant = Qos.factory().createParticipant(1305);
        TypeSupport<String> support = TypeSupport.newTypeSupport(String.class, Qos.env());
        assertEquals(String.class, support.getType());
        assertEquals(String.class.getName(), support.getTypeName());
        assertEquals("", support.newData());

        Topic<String> topic = participant.createTopic("FromTypeSupport", support);
        assertSame(support, topic.getTypeSupport());
        assertTrue(topic.getTypeSupport() instanceof TypeSupport);
        assertEquals(String.class.getName(), topic.getTypeName());
        DataWriter<String> writer = participant.createPublisher().createDataWriter(topic);
        DataReader<String> reader = participant.createSubscriber().createDataReader(topic);
        writer.write("through the TypeSupport");
        List<Sample<String>> samples = Qos.take(reader);
        assertEquals(1, samples.size());
        assertEquals("through the TypeSupport", samples.get(0).getData());
    }

    @Test
    void aTypeSupportRegisteredUnderANameGivesTheTopicItsWritersAndItsAnnouncementThatName()
            throws Exception {
        int domain = 1306;
        DomainParticipant observer = Qos.factory().createParticipant(domain);
        DomainParticipant first = Qos.factory().createParticipant(domain);
        DomainParticipant second = Qos.factory().createParticipant(domain);
        TypeSupport<String> named = TypeSupport.newTypeSupport(String.class, "my::Named", Qos.env());
        assertEquals("my::Named", named.getTypeName());

        Topic<String> writing = first.createTopic("TsNamed", named);
        Topic<String> reading = second.createTopic("TsNamed",
                TypeSupport.newTypeSupport(String.class, "my::Named", Qos.env()));
        assertEquals("my::Named", writing.getTypeName());
        DataWriter<String> writer = first.createPublisher().createDataWriter(writing);
        DataReader<String> reader = second.createSubscriber().createDataReader(reading);
        // writer and reader match on the registered name
        assertEquals(1, writer.getPublicationMatchedStatus().getCurrentCount());
        writer.write("named");
        assertEquals("named", Qos.take(reader).get(0).getData());

        // the announcement of the writer carries the registered name
        DataReader<PublicationBuiltinTopicDataImpl> publications =
                observer.getBuiltinSubscriber().lookupDataReader(BuiltinTopics.PUBLICATION);
        String announced = null;
        for (Sample<PublicationBuiltinTopicDataImpl> sample : Qos.take(publications)) {
            PublicationBuiltinTopicData data = sample.getData();
            if (data != null && "TsNamed".equals(data.getTopicName())) {
                announced = data.getTypeName();
            }
        }
        assertEquals("my::Named", announced);

        // a topic of the same name made from the class has the name of the class and is
        // another type: no match
        DomainParticipant third = Qos.factory().createParticipant(domain);
        DataReader<String> unmatched = third.createSubscriber().createDataReader(
                third.createTopic("TsNamed", String.class));
        writer.write("again");
        assertEquals(0, unmatched.read(new ArrayList<>()).size());
        assertEquals(0, unmatched.getSubscriptionMatchedStatus().getCurrentCount());
    }
}
