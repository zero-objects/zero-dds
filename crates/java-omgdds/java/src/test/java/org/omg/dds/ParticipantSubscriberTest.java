// SPDX-License-Identifier: Apache-2.0
package org.omg.dds;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.omg.dds.core.AlreadyClosedException;
import org.omg.dds.core.PreconditionNotMetException;
import org.omg.dds.core.event.PublicationMatchedEvent;
import org.omg.dds.core.status.LivelinessLostStatus;
import org.omg.dds.core.status.Status;
import org.omg.dds.domain.DomainParticipant;
import org.omg.dds.pub.DataWriter;
import org.omg.dds.pub.DataWriterAdapter;
import org.omg.dds.pub.Publisher;
import org.omg.dds.pub.PublisherAdapter;
import org.omg.dds.sub.DataReader;
import org.omg.dds.sub.Subscriber;
import org.omg.dds.topic.ContentFilteredTopic;
import org.omg.dds.topic.Topic;
import org.zerodds.DataStates;
import org.zerodds.internal.InProcessBus;

import java.util.ArrayList;
import java.util.Collection;
import java.util.Collections;
import java.util.concurrent.atomic.AtomicBoolean;
import java.util.concurrent.atomic.AtomicInteger;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * The Subscriber operations of DDS 1.4 §2.2.2.5.2 that belong to the entity containment:
 * lookup_datareader, delete_contained_entities, begin_access and end_access, get_datareaders
 * with a data state, and the status mask of a listener. Domain ids 1340 to 1349.
 */
class ParticipantSubscriberTest {

    @BeforeEach
    void setUp() {
        InProcessBus.instance().reset();
    }

    @AfterEach
    void tearDown() {
        InProcessBus.instance().reset();
    }

    // --- lookup_datareader (§2.2.2.5.2.7) --------------------------------------------------

    @Test
    void javapsm_2_lookup_data_reader_finds_by_name_by_topic_and_by_filtered_topic() {
        DomainParticipant participant = Qos.factory().createParticipant(1340);
        Topic<byte[]> topic = participant.createTopic("LookupReader", byte[].class);
        Topic<byte[]> other = participant.createTopic("LookupReaderOther", byte[].class);
        Subscriber subscriber = participant.createSubscriber();
        DataReader<byte[]> reader = subscriber.createDataReader(topic);
        ContentFilteredTopic<byte[]> view = participant.createContentFilteredTopic(
                "LookupReaderView", other, "", new ArrayList<>());
        DataReader<byte[]> viewReader = subscriber.createDataReader(view);

        assertSame(reader, subscriber.<byte[]>lookupDataReader("LookupReader"));
        assertSame(reader, subscriber.lookupDataReader(topic));
        assertSame(viewReader, subscriber.<byte[]>lookupDataReader("LookupReaderView"));
        assertSame(viewReader, subscriber.lookupDataReader(view));
        // the reader reads the filtered topic, not the topic under it
        assertNull(subscriber.lookupDataReader("LookupReaderOther"));
        assertNull(subscriber.lookupDataReader("Nothing"));

        reader.close();
        assertNull(subscriber.lookupDataReader("LookupReader"));
        assertNull(subscriber.lookupDataReader(topic));
    }

    // --- delete_subscriber, delete_contained_entities -------------------------------------------

    @Test
    void javapsm_2_closing_a_subscriber_closes_its_readers_and_leaves_the_participant() {
        DomainParticipant participant = Qos.factory().createParticipant(1341);
        Topic<byte[]> topic = participant.createTopic("CloseSubscriber", byte[].class);
        Subscriber subscriber = participant.createSubscriber();
        DataReader<byte[]> reader = subscriber.createDataReader(topic);
        assertTrue(participant.containsEntity(subscriber.getInstanceHandle()));
        assertTrue(participant.containsEntity(reader.getInstanceHandle()));

        subscriber.close();
        assertTrue(Qos.isClosed(reader));
        assertThrows(AlreadyClosedException.class, () -> reader.read(new ArrayList<>()));
        assertFalse(participant.containsEntity(subscriber.getInstanceHandle()));
        assertFalse(participant.containsEntity(reader.getInstanceHandle()));
        assertThrows(AlreadyClosedException.class, () -> subscriber.createDataReader(topic));
    }

    // --- begin_access, end_access (§2.2.2.5.2.8, §2.2.2.5.2.9) ---------------------------------------

    @Test
    void javapsm_2_end_access_needs_a_begin_access_and_the_calls_nest() {
        DomainParticipant participant = Qos.factory().createParticipant(1342);
        Subscriber subscriber = participant.createSubscriber();
        assertThrows(PreconditionNotMetException.class, subscriber::endAccess);
        subscriber.beginAccess();
        subscriber.beginAccess();
        subscriber.endAccess();
        subscriber.endAccess();
        assertThrows(PreconditionNotMetException.class, subscriber::endAccess);
        subscriber.close();
        assertThrows(AlreadyClosedException.class, subscriber::beginAccess);
    }

    @Test
    void javapsm_2_with_the_default_access_scope_no_begin_access_is_needed() {
        DomainParticipant participant = Qos.factory().createParticipant(1344);
        Subscriber subscriber = participant.createSubscriber();
        DataReader<byte[]> reader = subscriber.createDataReader(
                participant.createTopic("InstanceScope", byte[].class));
        assertEquals(0, reader.read(new ArrayList<>()).size());
    }

    // --- get_datareaders (§2.2.2.5.2.6) -----------------------------------------------------------------

    @Test
    void javapsm_2_get_data_readers_with_a_data_state_lists_the_readers_that_hold_such_samples() throws Exception {
        DomainParticipant participant = Qos.factory().createParticipant(1346);
        Topic<byte[]> first = participant.createTopic("DataStateOne", byte[].class);
        Topic<byte[]> second = participant.createTopic("DataStateTwo", byte[].class);
        Subscriber subscriber = participant.createSubscriber();
        DataReader<byte[]> readerOne = subscriber.createDataReader(first);
        DataReader<byte[]> readerTwo = subscriber.createDataReader(second);
        DataWriter<byte[]> writer = participant.createPublisher().createDataWriter(first);

        Collection<DataReader<?>> all = subscriber.getDataReaders(new ArrayList<DataReader<?>>());
        assertEquals(2, all.size());
        assertTrue(subscriber.getDataReaders(new ArrayList<DataReader<?>>(),
                DataStates.notRead(Qos.env())).isEmpty());

        writer.write(new byte[] {1});
        Collection<DataReader<?>> withData = subscriber.getDataReaders(
                new ArrayList<DataReader<?>>(), DataStates.notRead(Qos.env()));
        assertEquals(1, withData.size());
        assertSame(readerOne, withData.iterator().next());

        Qos.take(readerOne);
        assertTrue(subscriber.getDataReaders(new ArrayList<DataReader<?>>(),
                DataStates.notRead(Qos.env())).isEmpty());
        assertFalse(withData.contains(readerTwo));
    }

    // --- the status mask of a listener (§2.2.4.3.1) ----------------------------------------------------------

    @Test
    void javapsm_2_a_writer_listener_that_does_not_take_a_status_leaves_it_to_the_publisher() {
        DomainParticipant participant = Qos.factory().createParticipant(1347);
        Topic<byte[]> topic = participant.createTopic("MaskedWriter", byte[].class);
        AtomicInteger atWriter = new AtomicInteger();
        AtomicInteger atPublisher = new AtomicInteger();
        Publisher publisher = participant.createPublisher(Qos.publisher(), new PublisherAdapter() {
            @Override
            public void onPublicationMatched(PublicationMatchedEvent<?> status) {
                atPublisher.incrementAndGet();
            }
        }, null);
        // the writer's mask names LIVELINESS_LOST only
        publisher.createDataWriter(topic, Qos.writer(), new DataWriterAdapter<byte[]>() {
            @Override
            public void onPublicationMatched(PublicationMatchedEvent<byte[]> status) {
                atWriter.incrementAndGet();
            }
        }, Collections.<Class<? extends Status>>singleton(LivelinessLostStatus.class));
        participant.createSubscriber().createDataReader(topic);
        assertEquals(0, atWriter.get());
        assertEquals(1, atPublisher.get());
    }

    @Test
    void javapsm_2_an_empty_mask_takes_no_status_and_a_null_mask_takes_all() {
        DomainParticipant participant = Qos.factory().createParticipant(1348);
        Topic<byte[]> topic = participant.createTopic("EmptyMask", byte[].class);
        AtomicInteger empty = new AtomicInteger();
        AtomicInteger all = new AtomicInteger();
        participant.createPublisher().createDataWriter(topic, Qos.writer(),
                new DataWriterAdapter<byte[]>() {
                    @Override
                    public void onPublicationMatched(PublicationMatchedEvent<byte[]> status) {
                        empty.incrementAndGet();
                    }
                }, Collections.<Class<? extends Status>>emptySet());
        participant.createPublisher().createDataWriter(topic, Qos.writer(),
                new DataWriterAdapter<byte[]>() {
                    @Override
                    public void onPublicationMatched(PublicationMatchedEvent<byte[]> status) {
                        all.incrementAndGet();
                    }
                }, null);
        participant.createSubscriber().createDataReader(topic);
        assertEquals(0, empty.get());
        assertEquals(1, all.get());
    }
}
