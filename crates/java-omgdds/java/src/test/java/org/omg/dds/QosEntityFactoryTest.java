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
    void aPublisherWithAutoEnableFalseCreatesDisabledWriters() throws Exception {
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
    void aPublisherWithAutoEnableTrueCreatesEnabledWriters() throws Exception {
        try (QosFixtures.Rig rig = new QosFixtures.Rig(publisherQos(true), Qos.subscriber())) {
            DataWriter<QosFixtures.Item> writer = rig.writer(Qos.writer());
            writeOne(writer); // no NotEnabledException
        }
    }

    @Test
    void theChangeOfTheFactoryPolicyAffectsOnlyTheEntitiesCreatedAfterIt() throws Exception {
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
    void enablingThePublisherEnablesTheWritersItCreatedWhileDisabled() throws Exception {
        DomainParticipant participant = lazyParticipant();
        try {
            Topic<QosFixtures.Item> topic = participant.createTopic("Items", QosFixtures.Item.class);
            Publisher publisher = participant.createPublisher(publisherQos(true));
            assertFalse(((PublisherImpl) publisher).isEnabled(), "created by a participant that does not enable");
            DataWriter<QosFixtures.Item> writer = Qos.writer(publisher, topic, QosFixtures.SUPPORT, Qos.writer());
            assertThrows(NotEnabledException.class, () -> writeOne(writer));

            publisher.enable();
            writeOne(writer);
        } finally {
            participant.close();
        }
    }

    @Test
    void enablingThePublisherLeavesTheWritersDisabledWhenItsPolicyIsFalse() throws Exception {
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

    @Test
    void aWriterCannotBeEnabledBeforeItsPublisher() throws Exception {
        DomainParticipant participant = lazyParticipant();
        try {
            Topic<QosFixtures.Item> topic = participant.createTopic("Items", QosFixtures.Item.class);
            Publisher publisher = participant.createPublisher(publisherQos(true));
            DataWriter<QosFixtures.Item> writer = Qos.writer(publisher, topic, QosFixtures.SUPPORT, Qos.writer());
            assertThrows(PreconditionNotMetException.class, writer::enable);
            publisher.enable();
            writer.enable(); // idempotent now
        } finally {
            participant.close();
        }
    }

    // --- Subscriber and DataReader --------------------------------------------------------

    @Test
    void aSubscriberWithAutoEnableFalseCreatesDisabledReaders() throws Exception {
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

    @Test
    void enablingTheSubscriberEnablesTheReadersItCreatedWhileDisabled() throws Exception {
        DomainParticipant participant = lazyParticipant();
        try {
            Topic<QosFixtures.Item> topic = participant.createTopic("Items", QosFixtures.Item.class);
            Subscriber subscriber = participant.createSubscriber(subscriberQos(true));
            DataReader<QosFixtures.Item> reader = Qos.reader(subscriber, topic, QosFixtures.SUPPORT, Qos.reader());
            assertThrows(NotEnabledException.class, () -> Qos.read(reader));
            assertThrows(PreconditionNotMetException.class, reader::enable);

            subscriber.enable();
            assertEquals(0, Qos.read(reader).size());
        } finally {
            participant.close();
        }
    }

    // --- the participant and the factory ------------------------------------------------------

    @Test
    void theParticipantFactoryDecidesWhetherTheParticipantsItCreatesAreEnabled() {
        DomainParticipantFactory factory = Qos.factory();
        DomainParticipantFactoryQos original = factory.getQos();
        assertTrue(original.getEntityFactory().isAutoEnableCreatedEntities());
        DomainParticipant enabled = factory.createParticipant(QosFixtures.nextDomain());
        DomainParticipant disabled = null;
        try {
            assertTrue(((DomainParticipantImpl) enabled).isEnabled());

            DomainParticipantFactoryQos off = ((DomainParticipantFactoryQosImpl)
                    DomainParticipantFactoryQosImpl.defaults(Qos.env())).withPolicy(autoEnable(false));
            assertFalse(off.getEntityFactory().isAutoEnableCreatedEntities());
            factory.setQos(off);
            assertFalse(factory.getQos().getEntityFactory().isAutoEnableCreatedEntities());
            disabled = factory.createParticipant(QosFixtures.nextDomain());
            assertFalse(((DomainParticipantImpl) disabled).isEnabled());
            disabled.enable();
            assertTrue(((DomainParticipantImpl) disabled).isEnabled());
        } finally {
            factory.setQos(original);
            enabled.close();
            if (disabled != null) {
                disabled.close();
            }
        }
        assertTrue(factory.getQos().getEntityFactory().isAutoEnableCreatedEntities());
    }

    @Test
    void theParticipantPolicyDecidesWhetherItsPublishersSubscribersAndTopicsAreEnabled() {
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
