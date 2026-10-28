// SPDX-License-Identifier: Apache-2.0
package org.omg.dds;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.omg.dds.core.DomainEntity;
import org.omg.dds.core.Entity;
import org.omg.dds.core.EntityQos;
import org.omg.dds.core.event.PublicationMatchedEvent;
import org.omg.dds.core.event.StatusChangedEvent;
import org.omg.dds.core.status.DataAvailableStatus;
import org.omg.dds.core.status.LivelinessChangedStatus;
import org.omg.dds.core.status.LivelinessLostStatus;
import org.omg.dds.core.status.OfferedDeadlineMissedStatus;
import org.omg.dds.core.status.OfferedIncompatibleQosStatus;
import org.omg.dds.core.status.PublicationMatchedStatus;
import org.omg.dds.core.status.RequestedDeadlineMissedStatus;
import org.omg.dds.core.status.RequestedIncompatibleQosStatus;
import org.omg.dds.core.status.SampleLostStatus;
import org.omg.dds.core.status.SampleRejectedStatus;
import org.omg.dds.core.status.Status;
import org.omg.dds.core.status.SubscriptionMatchedStatus;
import org.omg.dds.domain.DomainParticipant;
import org.omg.dds.pub.DataWriter;
import org.omg.dds.pub.DataWriterAdapter;
import org.omg.dds.pub.DataWriterQos;
import org.omg.dds.pub.Publisher;
import org.omg.dds.sub.DataReader;
import org.omg.dds.sub.Subscriber;
import org.omg.dds.topic.Topic;
import org.zerodds.internal.InProcessBus;

import java.io.Closeable;
import java.io.Serializable;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.EventObject;
import java.util.List;
import java.util.Set;
import java.util.concurrent.CopyOnWriteArrayList;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * The entity base interfaces and the status classes of DDS-Java-PSM 1.0 §7.3.6 and §7.3.7.1:
 * what every entity extends, who its parent is, and the hierarchy of the statuses.
 * Domain ids 1400 to 1499.
 */
class QosEntityBasicsTest {

    @BeforeEach
    void setUp() {
        InProcessBus.instance().reset();
    }

    @AfterEach
    void tearDown() {
        InProcessBus.instance().reset();
    }

    // --- §7.3.6 ----------------------------------------------------------------------------

    @Test
    void entityExtendsCloseable() throws Exception {
        assertTrue(Closeable.class.isAssignableFrom(Entity.class));
        try (QosFixtures.Rig rig = new QosFixtures.Rig()) {
            // every entity can be held and closed as a java.io.Closeable
            List<Closeable> entities = new ArrayList<>();
            entities.add(rig.publisher);
            entities.add(rig.subscriber);
            entities.add(rig.topic);
            entities.add(rig.writer(Qos.writer()));
            entities.add(rig.reader(Qos.reader()));
            for (Closeable entity : entities) {
                entity.close();
                assertTrue(Qos.isClosed((Entity<?, ?>) entity), entity.toString());
            }
        }
    }

    @Test
    void everyEntityExtendsEntityAndEveryOneButTheParticipantExtendsDomainEntity() {
        assertTrue(Entity.class.isAssignableFrom(DomainParticipant.class));
        assertFalse(DomainEntity.class.isAssignableFrom(DomainParticipant.class));
        for (Class<?> type : Arrays.<Class<?>>asList(Publisher.class, Subscriber.class,
                Topic.class, DataWriter.class, DataReader.class)) {
            assertTrue(DomainEntity.class.isAssignableFrom(type), type.getSimpleName());
            assertTrue(Entity.class.isAssignableFrom(type), type.getSimpleName());
        }
    }

    @Test
    void theParentOfAnEntityIsTheEntityThatCreatedIt() throws Exception {
        try (QosFixtures.Rig rig = new QosFixtures.Rig()) {
            DataWriter<QosFixtures.Item> writer = rig.writer(Qos.writer());
            DataReader<QosFixtures.Item> reader = rig.reader(Qos.reader());
            assertSame(rig.participant, rig.publisher.getParent());
            assertSame(rig.participant, rig.subscriber.getParent());
            assertSame(rig.participant, rig.topic.getParent());
            assertSame(rig.publisher, writer.getParent());
            assertSame(rig.subscriber, reader.getParent());
        }
    }

    @Test
    void getQosAndGetListenerKeepTheirTypeOnTheSubinterfaces() throws Exception {
        try (QosFixtures.Rig rig = new QosFixtures.Rig()) {
            DataWriter<QosFixtures.Item> writer = rig.writer(Qos.writer());
            // The type parameters of Entity<LISTENER, QOS> make these compile without a cast.
            DataWriterQos qos = writer.getQos();
            org.omg.dds.pub.DataWriterListener<QosFixtures.Item> listener = writer.getListener();
            EntityQos<?> asBase = writer.getQos();
            assertSame(qos, asBase);
            assertEquals(null, listener);
        }
    }

    // --- §7.3.7.1 ----------------------------------------------------------------------------

    @Test
    void theThirteenStatusKindsAreClassesOfTheStatusBase() {
        Set<Class<? extends Status>> kinds = Status.allStatuses(Qos.env());
        assertEquals(13, kinds.size());
        for (Class<? extends Status> kind : kinds) {
            assertTrue(Status.class.isAssignableFrom(kind), kind.getSimpleName());
            assertTrue(java.lang.reflect.Modifier.isAbstract(kind.getModifiers()), kind.getSimpleName());
            assertTrue(Serializable.class.isAssignableFrom(kind), kind.getSimpleName());
        }
        assertTrue(kinds.containsAll(Arrays.<Class<? extends Status>>asList(
                DataAvailableStatus.class, LivelinessChangedStatus.class, LivelinessLostStatus.class,
                OfferedDeadlineMissedStatus.class, OfferedIncompatibleQosStatus.class,
                PublicationMatchedStatus.class, RequestedDeadlineMissedStatus.class,
                RequestedIncompatibleQosStatus.class, SampleLostStatus.class,
                SampleRejectedStatus.class, SubscriptionMatchedStatus.class)));
    }

    @Test
    void theEventThatCarriesAStatusIsAnEventObjectWithTheEntityAsSource() throws Exception {
        assertTrue(EventObject.class.isAssignableFrom(StatusChangedEvent.class));
        List<StatusChangedEvent<?>> events = new CopyOnWriteArrayList<>();
        try (QosFixtures.Rig rig = new QosFixtures.Rig()) {
            DataWriter<QosFixtures.Item> writer = rig.writer(Qos.writer());
            writer.setListener(new DataWriterAdapter<QosFixtures.Item>() {
                @Override
                public void onPublicationMatched(PublicationMatchedEvent<QosFixtures.Item> status) {
                    events.add(status);
                }
            });
            rig.reader(Qos.reader());

            assertEquals(1, events.size());
            StatusChangedEvent<?> event = events.get(0);
            assertSame(writer, ((EventObject) event).getSource());
            assertSame(writer, event.getSource());
            assertTrue(event instanceof PublicationMatchedEvent);
            assertEquals(1, ((PublicationMatchedEvent<?>) event).getStatus().getCurrentCount());
        }
    }
}
