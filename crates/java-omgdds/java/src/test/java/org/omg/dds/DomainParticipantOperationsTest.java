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
    void javapsm_2_find_topic_returns_the_topic_and_times_out_otherwise() throws Exception {
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
    void javapsm_2_lookup_topic_description_finds_only_what_was_created() {
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
    void xtypes_7_6_4_2_lookup_topic_description_with_an_index_iterates_the_topics_of_one_name() {
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
    void xtypes_7_6_4_1_a_topic_of_an_existing_name_with_other_policies_is_refused() {
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
    void javapsm_2_contains_entity_walks_the_containment() {
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

    /** ENTITY_FACTORY with autoEnableCreatedEntities = false (DDS 1.2 §7.1.3.7). */
    private static EntityFactory autoEnableOff() {
        return Qos.policies().EntityFactory()
                .withAutoEnableCreatedEntities(false);
    }

    @Test
    void javapsm_2_the_factory_qos_and_the_default_participant_qos_are_set_and_read() {
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
    void javapsm_2_lookup_participant_finds_one_of_the_domain() {
        int domain = nextDom();
        DomainParticipant first = Qos.factory().createParticipant(domain);
        assertSame(first, Qos.factory().lookupParticipant(domain));
        assertNull(Qos.factory().lookupParticipant(nextDom()));
    }

    @Test
    void javapsm_2_get_current_time_fills_the_container_or_returns_a_new_one() {
        DomainParticipant participant = Qos.factory().createParticipant(nextDom());
        Time now = participant.getCurrentTime();
        assertTrue(now.getTime(TimeUnit.SECONDS) > 0);

        ModifiableTime container =
                Qos.env().getSPI().newTime(0, TimeUnit.SECONDS);
        assertSame(container, participant.getCurrentTime(container));
        assertTrue(container.getTime(TimeUnit.SECONDS) > 0);
    }

    @Test
    void javapsm_2_get_builtin_subscriber_is_the_same_object_every_time() {
        DomainParticipant participant = Qos.factory().createParticipant(nextDom());
        Subscriber builtin = participant.getBuiltinSubscriber();
        assertNotNull(builtin);
        assertSame(builtin, participant.getBuiltinSubscriber());
    }

    @Test
    void javapsm_2_discovery_lists_the_participants_of_the_domain_and_ignore_hides_one() {
        int domain = nextDom();
        DomainParticipant mine = Qos.factory().createParticipant(domain);
        assertTrue(mine.getDiscoveredParticipants()
                .contains(mine.getInstanceHandle()));

        mine.ignoreParticipant(mine.getInstanceHandle());
        assertFalse(mine.getDiscoveredParticipants()
                .contains(mine.getInstanceHandle()));
    }

    @Test
    void javapsm_2_close_contained_entities_closes_them_without_closing_the_participant() {
        DomainParticipant participant = Qos.factory().createParticipant(nextDom());
        Publisher publisher = participant.createPublisher();
        Topic<String> topic = participant.createTopic("Chatter", String.class);

        participant.closeContainedEntities();
        assertTrue(Qos.isClosed(publisher));
        assertTrue(org.zerodds.topic.TopicImpl.isClosed(topic));
        assertFalse(Qos.isClosed(participant));
    }
}
