// SPDX-License-Identifier: Apache-2.0
package org.omg.dds;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.omg.dds.core.ModifiableTime;
import org.omg.dds.core.Time;
import org.omg.dds.core.policy.EntityFactory;
import org.omg.dds.domain.DomainParticipant;
import org.omg.dds.domain.DomainParticipantFactory;
import org.omg.dds.domain.DomainParticipantFactoryQos;
import org.omg.dds.domain.DomainParticipantQos;
import org.omg.dds.pub.Publisher;
import org.omg.dds.pub.PublisherQos;
import org.omg.dds.sub.Subscriber;
import org.omg.dds.topic.Topic;
import org.omg.dds.topic.TopicQos;
import org.omg.dds.topic.TopicDescription;
import org.zerodds.core.qos.DomainParticipantFactoryQosImpl;
import org.zerodds.domain.DomainParticipantImpl;
import org.zerodds.core.qos.DomainParticipantQosImpl;
import org.zerodds.internal.InProcessBus;
import org.zerodds.pub.PublisherImpl;
import org.zerodds.sub.SubscriberImpl;

import java.util.concurrent.TimeUnit;
import java.util.concurrent.TimeoutException;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * The DCPS operations of the participant of DDS 1.2 §2.2.1.2 that the OMG
 * API of DDS-Java-PSM 1.0 §7.4.2 adds to the entity: find_topic,
 * lookup_topicdescription, delete_contained_entities, contains_entity,
 * get_current_time, the default QoS operations and get_discovered_*.
 */
class DomainParticipantOperationsTest {

    private static int dom = 60;

    private static int nextDom() {
        return dom++;
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
    void findTopicReturnsTheTopicAndTimesOutOtherwise() throws Exception {
        DomainParticipant participant = Qos.factory().createParticipant(nextDom());
        Topic<String> topic = participant.createTopic("Chatter", String.class);

        assertSame(topic, participant.findTopic("Chatter",
                Qos.env().getSPI().zeroDuration()));
        assertSame(topic, participant.findTopic("Chatter", 0, TimeUnit.SECONDS));
        assertThrows(TimeoutException.class,
                () -> participant.findTopic("Nothing",
                        Qos.env().getSPI().zeroDuration()));
    }

    @Test
    void lookupTopicDescriptionFindsOnlyWhatWasCreated() {
        DomainParticipant participant = Qos.factory().createParticipant(nextDom());
        Topic<String> topic = participant.createTopic("Chatter", String.class);

        TopicDescription<String> found =
                participant.lookupTopicDescription("Chatter");
        assertSame(topic, found);
        assertSame(topic, found.<Topic<String>>cast());
        assertEquals("Chatter", found.getName());
        // §7.8.2: java.lang.String is the built-in type DDS::String (XTypes 1.3 Annex E)
        assertEquals("DDS::String", found.getTypeName());
        assertNull(participant.lookupTopicDescription("Nothing"));
    }

    /** DDS-XTypes 1.3 §7.6.4.1 and §7.6.4.2: several topics of one name, iterated by index. */
    @Test
    void lookupTopicDescriptionWithAnIndexIteratesTheTopicsOfOneName() {
        DomainParticipant participant = Qos.factory().createParticipant(nextDom());
        Topic<String> first = participant.createTopic("Chatter", String.class);
        Topic<byte[]> second = participant.createTopic("Chatter", byte[].class);
        DomainParticipantImpl impl = (DomainParticipantImpl) participant;

        assertSame(first, impl.lookupTopicDescription("Chatter", 0));
        assertSame(second, impl.lookupTopicDescription("Chatter", 1));
        assertEquals("DDS::String", impl.lookupTopicDescription("Chatter", 0).getTypeName());
        assertEquals("DDS::Bytes", impl.lookupTopicDescription("Chatter", 1).getTypeName());
        assertNull(impl.lookupTopicDescription("Chatter", 2), "past the last topic of the name");
        assertNull(impl.lookupTopicDescription("Chatter", -1));
        assertNull(impl.lookupTopicDescription("Nothing", 0));
    }

    /** DDS-XTypes 1.3 §7.6.4.1: the topics of one name have the QoS of one network topic. */
    @Test
    void aTopicOfAnExistingNameWithOtherPoliciesIsRefused() {
        DomainParticipant participant = Qos.factory().createParticipant(nextDom());
        Topic<String> first = participant.createTopic("Chatter", String.class);
        TopicQos other = Qos.topic().withPolicy(Qos.pf().TransportPriority().withValue(6));
        assertThrows(org.omg.dds.core.InconsistentPolicyException.class,
                () -> ((DomainParticipantImpl) participant).createTopic("Chatter", byte[].class, other));
        assertEquals(1, first.getInconsistentTopicStatus().getTotalCount());
        assertNull(((DomainParticipantImpl) participant).lookupTopicDescription("Chatter", 1));
        // the same policies are fine
        assertNotNull(participant.createTopic("Chatter", byte[].class));
    }

    @Test
    void containsEntityWalksTheContainment() {
        DomainParticipant participant = Qos.factory().createParticipant(nextDom());
        Publisher publisher = participant.createPublisher();
        Subscriber subscriber = participant.createSubscriber();
        Topic<String> topic = participant.createTopic("Chatter", String.class);
        var writer = ((PublisherImpl) publisher).createDataWriter(topic);
        var reader = ((SubscriberImpl) subscriber).createDataReader(topic);

        assertTrue(participant.containsEntity(publisher.getInstanceHandle()));
        assertTrue(participant.containsEntity(writer.getInstanceHandle()));
        assertTrue(participant.containsEntity(subscriber.getInstanceHandle()));
        assertTrue(participant.containsEntity(reader.getInstanceHandle()));
        assertTrue(participant.containsEntity(topic.getInstanceHandle()));
        assertFalse(participant.containsEntity(
                Qos.env().getSPI().nilHandle()));
    }

    @Test
    void defaultQosIsTheQosOfWhatIsCreatedNext() {
        DomainParticipant participant = Qos.factory().createParticipant(nextDom());
        PublisherQos defaults = participant.getDefaultPublisherQos();
        assertNotNull(defaults);

        PublisherQos mine = defaults.withPolicy(
                defaults.getPartition().withName(java.util.List.of("mine")));
        participant.setDefaultPublisherQos(mine);
        assertEquals(mine, participant.getDefaultPublisherQos());
        assertEquals(mine, participant.createPublisher().getQos());

        assertNotNull(participant.getDefaultSubscriberQos());
        assertNotNull(participant.getDefaultTopicQos());
    }

    @Test
    void entityFactoryDecidesWhetherWhatIsCreatedIsEnabled() {
        // DDS 1.2 §7.1.3.7: autoEnableCreatedEntities defaults to true.
        DomainParticipant participant = Qos.factory().createParticipant(nextDom());
        assertTrue(((PublisherImpl) participant.createPublisher()).isEnabled());

        DomainParticipant lazy = Qos.factory().createParticipant(
                nextDom(),
                DomainParticipantQosImpl.defaults(Qos.env())
                        .withPolicy(autoEnableOff()),
                null, null);
        assertFalse(((PublisherImpl) lazy.createPublisher()).isEnabled());
    }

    /** ENTITY_FACTORY with autoEnableCreatedEntities = false (DDS 1.2 §7.1.3.7). */
    private static EntityFactory autoEnableOff() {
        return Qos.policies().EntityFactory()
                .withAutoEnableCreatedEntities(false);
    }

    @Test
    void factoryQosAndDefaultParticipantQos() {
        DomainParticipantFactory factory = Qos.factory();
        assertNotNull(factory.getQos());
        assertNotNull(factory.getDefaultParticipantQos());

        // The normative DomainParticipantFactoryQos.withPolicy takes a
        // ForDataWriter, so the ENTITY_FACTORY goes in through the
        // implementation's own generic withPolicy.
        EntityFactory offFactory = autoEnableOff();
        DomainParticipantFactoryQos off = ((DomainParticipantFactoryQosImpl)
                DomainParticipantFactoryQosImpl.defaults(Qos.env()))
                .withPolicy(offFactory);
        factory.setQos(off);
        assertSame(off, factory.getQos());

        byte[] value = "mine".getBytes(java.nio.charset.StandardCharsets.UTF_8);
        DomainParticipantQos mine = DomainParticipantQosImpl
                .defaults(Qos.env()).withPolicy(
                        Qos.policies().UserData().withValue(value, 0, value.length));
        factory.setDefaultParticipantQos(mine);
        assertEquals(mine, factory.createParticipant(nextDom()).getQos());
        assertEquals(mine, factory.getDefaultParticipantQos());

        factory.setQos(DomainParticipantFactoryQosImpl.defaults(Qos.env()));
        factory.setDefaultParticipantQos(
                DomainParticipantQosImpl.defaults(Qos.env()));
    }

    @Test
    void lookupParticipantFindsOneOfTheDomain() {
        int domain = nextDom();
        DomainParticipant first = Qos.factory().createParticipant(domain);
        assertSame(first, Qos.factory().lookupParticipant(domain));
        assertNull(Qos.factory().lookupParticipant(nextDom()));
    }

    @Test
    void getCurrentTimeFillsTheContainerOrReturnsANewOne() {
        DomainParticipant participant = Qos.factory().createParticipant(nextDom());
        Time now = participant.getCurrentTime();
        assertTrue(now.getTime(TimeUnit.SECONDS) > 0);

        ModifiableTime container =
                Qos.env().getSPI().newTime(0, TimeUnit.SECONDS);
        assertSame(container, participant.getCurrentTime(container));
        assertTrue(container.getTime(TimeUnit.SECONDS) > 0);
    }

    @Test
    void getBuiltinSubscriberIsTheSameObjectEveryTime() {
        DomainParticipant participant = Qos.factory().createParticipant(nextDom());
        Subscriber builtin = participant.getBuiltinSubscriber();
        assertNotNull(builtin);
        assertSame(builtin, participant.getBuiltinSubscriber());
    }

    @Test
    void discoveryListsTheParticipantsOfTheDomainAndIgnoreHidesOne() {
        int domain = nextDom();
        DomainParticipant mine = Qos.factory().createParticipant(domain);
        assertTrue(mine.getDiscoveredParticipants()
                .contains(mine.getInstanceHandle()));

        mine.ignoreParticipant(mine.getInstanceHandle());
        assertFalse(mine.getDiscoveredParticipants()
                .contains(mine.getInstanceHandle()));
    }

    @Test
    void closeContainedEntitiesClosesThemWithoutClosingTheParticipant() {
        DomainParticipant participant = Qos.factory().createParticipant(nextDom());
        Publisher publisher = participant.createPublisher();
        Topic<String> topic = participant.createTopic("Chatter", String.class);

        participant.closeContainedEntities();
        assertTrue(Qos.isClosed(publisher));
        assertTrue(org.zerodds.topic.TopicImpl.isClosed(topic));
        assertFalse(Qos.isClosed(participant));
    }
}
