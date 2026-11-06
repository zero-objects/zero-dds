// SPDX-License-Identifier: Apache-2.0
package org.omg.dds;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.omg.dds.core.NotEnabledException;
import org.omg.dds.core.PreconditionNotMetException;
import org.omg.dds.core.event.PublicationMatchedEvent;
import org.omg.dds.core.event.SubscriptionMatchedEvent;
import org.omg.dds.core.policy.EntityFactory;
import org.omg.dds.core.status.PublicationMatchedStatus;
import org.omg.dds.core.status.Status;
import org.omg.dds.domain.DomainParticipant;
import org.omg.dds.domain.DomainParticipantAdapter;
import org.omg.dds.domain.DomainParticipantFactory;
import org.omg.dds.domain.DomainParticipantFactoryQos;
import org.omg.dds.domain.DomainParticipantListener;
import org.omg.dds.domain.DomainParticipantQos;
import org.omg.dds.pub.DataWriter;
import org.omg.dds.pub.Publisher;
import org.omg.dds.pub.PublisherAdapter;
import org.omg.dds.pub.PublisherQos;
import org.omg.dds.sub.DataReader;
import org.omg.dds.sub.Subscriber;
import org.omg.dds.sub.SubscriberAdapter;
import org.omg.dds.sub.SubscriberQos;
import org.omg.dds.topic.Topic;
import org.omg.dds.topic.TopicAdapter;
import org.zerodds.core.qos.DomainParticipantFactoryQosImpl;
import org.zerodds.core.qos.DomainParticipantQosImpl;
import org.zerodds.domain.DomainParticipantImpl;
import org.zerodds.internal.DurabilityService;
import org.zerodds.internal.InProcessBus;
import org.zerodds.pub.DataWriterImpl;
import org.zerodds.pub.PublisherImpl;
import org.zerodds.sub.DataReaderImpl;
import org.zerodds.sub.SubscriberImpl;

import java.util.ArrayList;
import java.util.Collections;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.concurrent.atomic.AtomicReference;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * The entity operations of DDS 1.4 §2.2.2.1 and the participant operations of §2.2.2.2.1
 * that concern creating, enabling, deleting and configuring: enable and ENTITY_FACTORY,
 * create_* with a listener and a status mask, get_domain_id, delete_participant, set_qos and the
 * default QoS operations. Domain ids 1200 to 1229.
 */
class ParticipantEntityTest {

    @BeforeEach
    void setUp() {
        InProcessBus.instance().reset();
        DurabilityService.instance().reset();
    }

    @AfterEach
    void tearDown() {
        InProcessBus.instance().reset();
        DurabilityService.instance().reset();
    }

    private static EntityFactory autoEnableOff() {
        return Qos.policies().EntityFactory().withAutoEnableCreatedEntities(false);
    }

    /** A participant whose ENTITY_FACTORY leaves what it creates disabled. */
    private static DomainParticipant lazyParticipant(int domain) {
        return Qos.factory().createParticipant(domain,
                DomainParticipantQosImpl.defaults(Qos.env()).withPolicy(autoEnableOff()),
                null, null);
    }

    // --- get_domain_id ------------------------------------------------------

    @Test
    void javapsm_2_get_domain_id_is_the_domain_the_participant_was_created_in() {
        assertEquals(1200, Qos.factory().createParticipant(1200).getDomainId());
        assertEquals(1201, Qos.factory().createParticipant(1201).getDomainId());
    }

    // --- enable and ENTITY_FACTORY (§2.2.2.1.1.7, §2.2.3.20) ---------------------

    @Test
    void javapsm_2_a_factory_that_does_not_auto_enable_creates_a_disabled_participant_whose_children_stay_disabled() {
        DomainParticipantFactory factory = Qos.factory();
        DomainParticipantFactoryQos original = factory.getQos();
        try {
            factory.setQos(((DomainParticipantFactoryQosImpl)
                    DomainParticipantFactoryQosImpl.defaults(Qos.env())).withPolicy(autoEnableOff()));
            DomainParticipantImpl participant = (DomainParticipantImpl) factory.createParticipant(1202);
            assertFalse(participant.isEnabled());
            Publisher publisher = participant.createPublisher();
            Subscriber subscriber = participant.createSubscriber();
            Topic<byte[]> topic = participant.createTopic("FactoryOff", byte[].class);
            assertFalse(((PublisherImpl) publisher).isEnabled());
            assertFalse(((SubscriberImpl) subscriber).isEnabled());
            assertFalse(((org.zerodds.topic.TopicImpl<byte[]>) topic).isEnabled());
            // enabling a child of a participant that is not enabled is refused
            assertThrows(PreconditionNotMetException.class, publisher::enable);
            assertThrows(PreconditionNotMetException.class, subscriber::enable);

            participant.enable();
            assertTrue(participant.isEnabled());
            // the participant's own ENTITY_FACTORY is true: what it created is enabled with it
            assertTrue(((PublisherImpl) publisher).isEnabled());
            assertTrue(((SubscriberImpl) subscriber).isEnabled());
            assertTrue(((org.zerodds.topic.TopicImpl<byte[]>) topic).isEnabled());
            // and enable is idempotent
            participant.enable();
        } finally {
            factory.setQos(original);
        }
    }

    @Test
    void javapsm_2_a_writer_of_a_disabled_publisher_is_disabled_and_enabling_the_publisher_enables_it() throws Exception {
        DomainParticipant participant = lazyParticipant(1203);
        Topic<byte[]> topic = participant.createTopic("LazyWriter", byte[].class);
        Publisher publisher = participant.createPublisher();
        assertFalse(((PublisherImpl) publisher).isEnabled());
        DataWriter<byte[]> writer = publisher.createDataWriter(topic);
        assertFalse(((DataWriterImpl<byte[]>) writer).isEnabled());
        assertThrows(NotEnabledException.class, () -> writer.write(new byte[] {1}));
        // the factory of the writer is not enabled
        assertThrows(PreconditionNotMetException.class, writer::enable);

        publisher.enable();
        assertTrue(((PublisherImpl) publisher).isEnabled());
        // the Publisher's own ENTITY_FACTORY is true: the writer it created is enabled with it
        assertTrue(((DataWriterImpl<byte[]>) writer).isEnabled());
        writer.write(new byte[] {2});
    }

    @Test
    void javapsm_2_a_reader_of_a_disabled_subscriber_is_disabled_and_enabling_the_subscriber_enables_it() {
        DomainParticipant participant = lazyParticipant(1205);
        Topic<byte[]> topic = participant.createTopic("LazyReader", byte[].class);
        Subscriber subscriber = participant.createSubscriber();
        assertFalse(((SubscriberImpl) subscriber).isEnabled());
        DataReader<byte[]> reader = subscriber.createDataReader(topic);
        assertFalse(((DataReaderImpl<byte[]>) reader).isEnabled());
        assertThrows(PreconditionNotMetException.class, reader::enable);

        subscriber.enable();
        assertTrue(((DataReaderImpl<byte[]>) reader).isEnabled());
    }

    // --- create_* with a listener and a status mask (§2.2.4.3.1) -----------------

    @Test
    void javapsm_2_create_participant_with_a_listener_and_a_mask_reaches_it_from_the_writers_of_the_participant() {
        AtomicInteger publicationMatched = new AtomicInteger();
        AtomicInteger subscriptionMatched = new AtomicInteger();
        DomainParticipantListener listener = new DomainParticipantAdapter() {
            @Override
            public void onPublicationMatched(PublicationMatchedEvent<?> status) {
                publicationMatched.incrementAndGet();
            }

            @Override
            public void onSubscriptionMatched(SubscriptionMatchedEvent<?> status) {
                subscriptionMatched.incrementAndGet();
            }
        };
        DomainParticipant participant = Qos.factory().createParticipant(1207,
                Qos.factory().getDefaultParticipantQos(), listener,
                Collections.<Class<? extends Status>>singleton(PublicationMatchedStatus.class));
        assertSame(listener, participant.getListener());
        Topic<byte[]> topic = participant.createTopic("ParticipantMask", byte[].class);
        participant.createSubscriber().createDataReader(topic);
        participant.createPublisher().createDataWriter(topic);
        assertEquals(1, publicationMatched.get());
        // SUBSCRIPTION_MATCHED is not in the mask of the participant's listener
        assertEquals(0, subscriptionMatched.get());
    }

    @Test
    void javapsm_2_create_publisher_with_a_listener_sees_the_match_of_a_writer_created_later() {
        DomainParticipant participant = Qos.factory().createParticipant(1208);
        Topic<byte[]> topic = participant.createTopic("PublisherListener", byte[].class);
        participant.createSubscriber().createDataReader(topic);
        AtomicReference<DataWriter<?>> source = new AtomicReference<>();
        Publisher publisher = participant.createPublisher(Qos.publisher(), new PublisherAdapter() {
            @Override
            public void onPublicationMatched(PublicationMatchedEvent<?> status) {
                source.set(status.getSource());
            }
        }, null);
        DataWriter<byte[]> writer = publisher.createDataWriter(topic);
        assertSame(writer, source.get());
    }

    @Test
    void javapsm_2_create_subscriber_with_a_listener_sees_the_match_of_a_reader_created_later() {
        DomainParticipant participant = Qos.factory().createParticipant(1209);
        Topic<byte[]> topic = participant.createTopic("SubscriberListener", byte[].class);
        participant.createPublisher().createDataWriter(topic);
        AtomicReference<DataReader<?>> source = new AtomicReference<>();
        Subscriber subscriber = participant.createSubscriber(Qos.subscriber(), new SubscriberAdapter() {
            @Override
            public void onSubscriptionMatched(SubscriptionMatchedEvent<?> status) {
                source.set(status.getSource());
            }
        }, null);
        DataReader<byte[]> reader = subscriber.createDataReader(topic);
        assertSame(reader, source.get());
    }

    @Test
    void javapsm_2_create_topic_with_a_listener_and_a_mask_keeps_them() {
        DomainParticipant participant = Qos.factory().createParticipant(1210);
        TopicAdapter<byte[]> listener = new TopicAdapter<>();
        Topic<byte[]> topic = participant.createTopic("TopicListenerKept", byte[].class,
                participant.getDefaultTopicQos(), listener, null);
        assertSame(listener, topic.getListener());
    }

    // --- delete_participant: close (§2.2.2.2.1, factory delete_participant) ----------

    @Test
    void javapsm_2_the_transient_data_of_a_domain_survives_the_close_of_a_participant_while_another_one_is_open() throws Exception {
        int domain = 1211;
        DomainParticipant first = Qos.factory().createParticipant(domain);
        DomainParticipant second = Qos.factory().createParticipant(domain);
        Topic<byte[]> topic = first.createTopic("DomainData", byte[].class);
        DataWriter<byte[]> writer = first.createPublisher().createDataWriter(topic,
                Qos.writer().withPolicy(Qos.pf().Durability().withTransient())
                        // closing unregisters, and an unregister disposes with autodispose true
                        // (DDS 1.4 2.2.3.21); the data of a disposed instance does not outlive it
                        .withPolicy(Qos.pf().WriterDataLifecycle()
                                .withAutDisposeUnregisteredInstances(false)));
        writer.write(new byte[] {7});
        writer.close(); // TRANSIENT data outlives the writer

        second.close();
        // the first participant is open: the TRANSIENT data of the domain is still there
        DataReader<byte[]> late = first.createSubscriber().createDataReader(topic,
                Qos.reader().withPolicy(Qos.pf().Durability().withTransient()));
        assertEquals(1, late.take(new ArrayList<>()).size());
        assertSame(first, Qos.factory().lookupParticipant(domain));

        first.close();
        assertNull(Qos.factory().lookupParticipant(domain));
    }

    // --- set_qos and the default QoS (§2.2.2.1.1.1, §2.2.2.2.1.20 to §2.2.2.2.1.25) -------

    @Test
    void javapsm_2_a_default_qos_whose_policies_are_not_consistent_is_refused_and_keeps_the_old_one() {
        DomainParticipant participant = Qos.factory().createParticipant(1213);
        // HISTORY depth 10 is larger than RESOURCE_LIMITS max_samples_per_instance 5
        // (DDS 1.4 §2.2.3.18)
        org.omg.dds.topic.TopicQos before = participant.getDefaultTopicQos();
        org.omg.dds.topic.TopicQos inconsistent = Qos.topic()
                .withPolicy(Qos.pf().History().withKeepLast(10))
                .withPolicy(Qos.pf().ResourceLimits().withMaxSamplesPerInstance(5));
        assertThrows(org.omg.dds.core.InconsistentPolicyException.class,
                () -> participant.setDefaultTopicQos(inconsistent));
        assertSame(before, participant.getDefaultTopicQos());
        // and a topic cannot be created with it
        assertThrows(org.omg.dds.core.InconsistentPolicyException.class,
                () -> ((DomainParticipantImpl) participant).createTopic(
                        "Inconsistent", byte[].class, inconsistent));
        // max_samples must not be smaller than max_samples_per_instance (§2.2.3.19)
        org.omg.dds.topic.TopicQos small = Qos.topic().withPolicy(
                Qos.pf().ResourceLimits().withMaxSamples(3).withMaxSamplesPerInstance(5));
        assertThrows(org.omg.dds.core.InconsistentPolicyException.class,
                () -> participant.setDefaultTopicQos(small));
    }

}
