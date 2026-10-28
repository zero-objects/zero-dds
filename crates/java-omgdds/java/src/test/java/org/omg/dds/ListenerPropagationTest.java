// SPDX-License-Identifier: Apache-2.0
package org.omg.dds;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.omg.dds.core.event.DataAvailableEvent;
import org.omg.dds.core.event.DataOnReadersEvent;
import org.omg.dds.core.event.PublicationMatchedEvent;
import org.omg.dds.core.event.SubscriptionMatchedEvent;
import org.omg.dds.core.status.PublicationMatchedStatus;
import org.omg.dds.core.status.Status;
import org.omg.dds.core.status.SubscriptionMatchedStatus;
import org.omg.dds.domain.DomainParticipant;
import org.omg.dds.domain.DomainParticipantAdapter;
import org.omg.dds.pub.DataWriter;
import org.omg.dds.pub.DataWriterAdapter;
import org.omg.dds.pub.Publisher;
import org.omg.dds.pub.PublisherAdapter;
import org.omg.dds.sub.DataReader;
import org.omg.dds.sub.DataReaderAdapter;
import org.omg.dds.sub.Subscriber;
import org.omg.dds.sub.SubscriberAdapter;
import org.omg.dds.topic.Topic;
import org.zerodds.internal.InProcessBus;

import java.util.Collections;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.concurrent.atomic.AtomicReference;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertSame;

/**
 * DDS 1.4 §2.2.4.3.1: a status change goes to the listener of the entity if it takes that
 * status, else to the listener of the containing entity, up to the DomainParticipant.
 */
class ListenerPropagationTest {

    @BeforeEach
    void setUp() {
        InProcessBus.instance().reset();
    }

    @AfterEach
    void tearDown() {
        InProcessBus.instance().reset();
    }

    @Test
    void aWriterWithoutListenerPassesTheMatchToThePublisherAndThenTheParticipant() {
        DomainParticipant participant = Qos.factory().createParticipant(960);
        Topic<byte[]> topic = participant.createTopic("UpWriter", byte[].class);
        AtomicInteger atParticipant = new AtomicInteger();
        participant.setListener(new DomainParticipantAdapter() {
            @Override
            public void onPublicationMatched(PublicationMatchedEvent<?> status) {
                atParticipant.incrementAndGet();
            }
        });
        participant.createSubscriber().createDataReader(topic);
        Publisher publisher = participant.createPublisher();
        AtomicReference<DataWriter<?>> atPublisher = new AtomicReference<>();
        publisher.setListener(new PublisherAdapter() {
            @Override
            public void onPublicationMatched(PublicationMatchedEvent<?> status) {
                atPublisher.set(status.getSource());
            }
        });
        DataWriter<byte[]> first = publisher.createDataWriter(topic);
        assertSame(first, atPublisher.get());
        assertEquals(0, atParticipant.get(), "the Publisher took it");
        // a Publisher that takes no matches passes them on
        publisher.setListener(publisher.getListener(), Collections.<Class<? extends Status>>emptySet());
        participant.createPublisher().createDataWriter(topic);
        assertEquals(1, atParticipant.get());
    }

    @Test
    void aWriterListenerSetAtCreationSeesTheMatchOfEnable() {
        DomainParticipant participant = Qos.factory().createParticipant(961);
        Topic<byte[]> topic = participant.createTopic("AtCreation", byte[].class);
        participant.createSubscriber().createDataReader(topic);
        AtomicReference<PublicationMatchedStatus> seen = new AtomicReference<>();
        participant.createPublisher().createDataWriter(topic, Qos.writer(),
                new DataWriterAdapter<byte[]>() {
                    @Override
                    public void onPublicationMatched(PublicationMatchedEvent<byte[]> status) {
                        seen.set(status.getStatus());
                    }
                }, null);
        assertEquals(1, seen.get().getCurrentCount());
    }

    @Test
    void aReaderListenerSetAtCreationSeesTheMatchOfEnableAndTheSubscriberGetsDataOnReaders() {
        DomainParticipant participant = Qos.factory().createParticipant(962);
        Topic<byte[]> topic = participant.createTopic("ReaderUp", byte[].class);
        DataWriter<byte[]> writer = participant.createPublisher().createDataWriter(topic);
        Subscriber subscriber = participant.createSubscriber();
        AtomicInteger onReaders = new AtomicInteger();
        AtomicInteger onAvailable = new AtomicInteger();
        subscriber.setListener(new SubscriberAdapter() {
            @Override
            public void onDataOnReaders(DataOnReadersEvent status) {
                onReaders.incrementAndGet();
            }

            @Override
            public void onSubscriptionMatched(SubscriptionMatchedEvent<?> status) {
                // not reached: the reader has its own listener for it
                onAvailable.addAndGet(100);
            }
        });
        AtomicReference<SubscriptionMatchedStatus> seen = new AtomicReference<>();
        DataReader<byte[]> reader = subscriber.createDataReader(topic, Qos.reader(),
                new DataReaderAdapter<byte[]>() {
                    @Override
                    public void onSubscriptionMatched(SubscriptionMatchedEvent<byte[]> status) {
                        seen.set(status.getStatus());
                    }

                    @Override
                    public void onDataAvailable(DataAvailableEvent<byte[]> status) {
                        onAvailable.incrementAndGet();
                    }
                }, null);
        assertEquals(1, seen.get().getCurrentCount());
        try {
            writer.write(new byte[] {1});
        } catch (java.util.concurrent.TimeoutException e) {
            throw new AssertionError(e);
        }
        // DATA_ON_READERS goes to the Subscriber and takes the place of the reader's DATA_AVAILABLE
        assertEquals(1, onReaders.get());
        assertEquals(0, onAvailable.get());
        assertEquals(1, reader.read(new java.util.ArrayList<>()).size());
    }
}
