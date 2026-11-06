// SPDX-License-Identifier: Apache-2.0
package org.omg.dds;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.omg.dds.core.NotEnabledException;
import org.omg.dds.core.PreconditionNotMetException;
import org.omg.dds.core.policy.EntityFactory;
import org.omg.dds.domain.DomainParticipant;
import org.omg.dds.domain.DomainParticipantFactory;
import org.omg.dds.domain.DomainParticipantFactoryQos;
import org.omg.dds.domain.DomainParticipantQos;
import org.omg.dds.pub.DataWriter;
import org.omg.dds.pub.Publisher;
import org.omg.dds.pub.PublisherQos;
import org.omg.dds.sub.DataReader;
import org.omg.dds.sub.Subscriber;
import org.omg.dds.sub.SubscriberQos;
import org.omg.dds.topic.Topic;
import org.zerodds.core.qos.DomainParticipantFactoryQosImpl;
import org.zerodds.core.qos.DomainParticipantQosImpl;
import org.zerodds.domain.DomainParticipantImpl;
import org.zerodds.internal.InProcessBus;
import org.zerodds.pub.PublisherImpl;
import org.zerodds.sub.SubscriberImpl;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * ENTITY_FACTORY (DDS 1.4 §2.2.3.20): whether what a factory creates is enabled, and
 * §2.2.2.1.1.7: an entity created by a disabled factory is disabled, enabling a factory enables
 * what it created, and an entity cannot be enabled before its factory. Domain ids 1400 to 1499.
 */
class QosEntityFactoryTest {

    @BeforeEach
    void setUp() {
        InProcessBus.instance().reset();
    }

    @AfterEach
    void tearDown() {
        InProcessBus.instance().reset();
    }

    private static EntityFactory autoEnable(boolean on) {
        return Qos.pf().EntityFactory().withAutoEnableCreatedEntities(on);
    }

    private static PublisherQos publisherQos(boolean autoEnable) {
        return Qos.publisher().withPolicy(autoEnable(autoEnable));
    }

    private static SubscriberQos subscriberQos(boolean autoEnable) {
        return Qos.subscriber().withPolicy(autoEnable(autoEnable));
    }

    /** A participant whose own ENTITY_FACTORY leaves the publishers and subscribers it creates disabled. */
    private static DomainParticipant lazyParticipant() {
        DomainParticipantQos lazy = DomainParticipantQosImpl.defaults(Qos.env()).withPolicy(autoEnable(false));
        return Qos.factory().createParticipant(QosFixtures.nextDomain(), lazy, null, null);
    }

    private static void writeOne(DataWriter<QosFixtures.Item> writer) throws Exception {
        writer.write(new QosFixtures.Item(1, 1));
    }

    // --- Publisher and DataWriter -----------------------------------------------------

    @Test
    void javapsm_7_3_5_1_a_publisher_with_auto_enable_false_creates_disabled_writers() throws Exception {
        try (QosFixtures.Rig rig = new QosFixtures.Rig(publisherQos(false), Qos.subscriber())) {
            assertTrue(((PublisherImpl) rig.publisher).isEnabled());
            DataReader<QosFixtures.Item> reader = rig.reader(Qos.reader());
            DataWriter<QosFixtures.Item> writer = rig.writer(Qos.writer());

            assertThrows(NotEnabledException.class, () -> writeOne(writer));
            assertEquals(0, reader.getSubscriptionMatchedStatus().getCurrentCount(),
                    "a writer that is not enabled is not a match");

            writer.enable();
            assertEquals(1, reader.getSubscriptionMatchedStatus().getCurrentCount());
            writeOne(writer);
            assertEquals(1, Qos.take(reader).size());
        }
    }

    @Test
    void javapsm_7_3_5_1_the_change_of_the_factory_policy_affects_only_the_entities_created_after_it() throws Exception {
        try (QosFixtures.Rig rig = new QosFixtures.Rig()) {
            DataWriter<QosFixtures.Item> before = rig.writer(Qos.writer());
            rig.publisher.setQos(rig.publisher.getQos().withPolicy(autoEnable(false)));
            DataWriter<QosFixtures.Item> disabled = rig.writer(Qos.writer());
            rig.publisher.setQos(rig.publisher.getQos().withPolicy(autoEnable(true)));
            DataWriter<QosFixtures.Item> after = rig.writer(Qos.writer());

            writeOne(before);
            assertThrows(NotEnabledException.class, () -> writeOne(disabled));
            writeOne(after);
        }
    }

    @Test
    void javapsm_7_3_5_1_enabling_the_publisher_leaves_the_writers_disabled_when_its_policy_is_false() throws Exception {
        DomainParticipant participant = lazyParticipant();
        try {
            Topic<QosFixtures.Item> topic = participant.createTopic("Items", QosFixtures.Item.class);
            Publisher publisher = participant.createPublisher(publisherQos(false));
            DataWriter<QosFixtures.Item> writer = Qos.writer(publisher, topic, QosFixtures.SUPPORT, Qos.writer());

            publisher.enable();
            assertThrows(NotEnabledException.class, () -> writeOne(writer));
            writer.enable();
            writeOne(writer);
        } finally {
            participant.close();
        }
    }

    // --- Subscriber and DataReader --------------------------------------------------------

    @Test
    void javapsm_7_3_5_1_a_subscriber_with_auto_enable_false_creates_disabled_readers() throws Exception {
        try (QosFixtures.Rig rig = new QosFixtures.Rig(Qos.publisher(), subscriberQos(false))) {
            assertTrue(((SubscriberImpl) rig.subscriber).isEnabled());
            DataReader<QosFixtures.Item> reader = rig.reader(Qos.reader());
            DataWriter<QosFixtures.Item> writer = rig.writer(Qos.writer());

            assertThrows(NotEnabledException.class, () -> Qos.read(reader));
            assertEquals(0, writer.getPublicationMatchedStatus().getCurrentCount());

            reader.enable();
            assertEquals(1, writer.getPublicationMatchedStatus().getCurrentCount());
            writeOne(writer);
            assertEquals(1, Qos.take(reader).size());
        }
    }

    // --- the participant and the factory ------------------------------------------------------

    @Test
    void javapsm_7_3_5_1_the_participant_policy_decides_whether_its_publishers_subscribers_and_topics_are_enabled() {
        DomainParticipant lazy = lazyParticipant();
        try {
            assertFalse(((PublisherImpl) lazy.createPublisher()).isEnabled());
            assertFalse(((SubscriberImpl) lazy.createSubscriber()).isEnabled());
            assertFalse(((org.zerodds.topic.TopicImpl<?>) lazy.createTopic("T", QosFixtures.Item.class)).isEnabled());
        } finally {
            lazy.close();
        }
    }
}
