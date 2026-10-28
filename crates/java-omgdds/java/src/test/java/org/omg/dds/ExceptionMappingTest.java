// SPDX-License-Identifier: Apache-2.0
package org.omg.dds;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.omg.dds.core.DDSException;
import org.omg.dds.domain.DomainParticipant;
import org.omg.dds.domain.DomainParticipantFactory;
import org.omg.dds.pub.DataWriter;
import org.zerodds.pub.DataWriterImpl;
import org.omg.dds.pub.Publisher;
import org.omg.dds.sub.DataReader;
import org.omg.dds.sub.Subscriber;
import org.omg.dds.topic.Topic;
import org.zerodds.core.AlreadyClosedException;
import org.zerodds.core.NotEnabledException;
import org.zerodds.internal.InProcessBus;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * DDS-Java-PSM 1.0 §7.3.2 (Error Handling and Exceptions) and Table 7.1: the
 * ReturnCode_t of the PIM is mapped onto Java exceptions. A failed operation
 * must raise, not return a value the application has to remember to check.
 */
class ExceptionMappingTest {

    private static final int DOMAIN = 41;

    private DomainParticipant participant;
    private Topic<byte[]> topic;
    private Publisher publisher;
    private Subscriber subscriber;
    private DataWriter<byte[]> writer;
    private DataReader<byte[]> reader;

    @BeforeEach
    void setUp() {
        InProcessBus.instance().reset();
        participant = Qos.factory().createParticipant(DOMAIN);
        topic = participant.createTopic("Errors", byte[].class);
        publisher = participant.createPublisher();
        subscriber = participant.createSubscriber();
        writer = publisher.createDataWriter(topic);
        reader = subscriber.createDataReader(topic);
    }

    @AfterEach
    void tearDown() {
        participant.close();
    }

    @Test
    void writeOnAClosedWriterThrowsAlreadyClosed() {
        writer.close();
        // RETCODE_ALREADY_DELETED (Table 7.1); without the exception the sample
        // is dropped silently and the application never learns about it.
        assertThrows(AlreadyClosedException.class, () -> writer.write(new byte[] {1}));
    }

    @Test
    void readAndTakeOnAClosedReaderThrowAlreadyClosed() {
        reader.close();
        assertThrows(AlreadyClosedException.class, reader::read);
        assertThrows(AlreadyClosedException.class, reader::take);
    }

    @Test
    void aNotEnabledWriterRefusesToWrite() {
        DataWriterImpl<byte[]> disabled = new DataWriterImpl<>(DOMAIN, null, topic,
                org.zerodds.cdr.ByteArrayTypeSupport.INSTANCE,
                Qos.writer(), java.util.Set.of());
        // RETCODE_NOT_ENABLED (Table 7.1)
        assertThrows(NotEnabledException.class, () -> disabled.write(new byte[] {1}));
        // it belongs to no Publisher, so closing the participant does not close it, and it
        // holds the Topic (DDS 1.4 2.2.2.2.1.7 delete_topic)
        disabled.close();
    }

    @Test
    void everyPsmExceptionIsAnUncheckedDdsException() {
        assertTrue(RuntimeException.class.isAssignableFrom(DDSException.class),
                "§7.3.2: all exceptions but TimeoutException are unchecked");
        assertTrue(DDSException.class.isAssignableFrom(AlreadyClosedException.class));
        assertTrue(DDSException.class.isAssignableFrom(NotEnabledException.class));
        assertFalse(Exception.class.equals(DDSException.class));
    }

    @Test
    void aClosedEntityRefusesToCreateContainedEntities() {
        publisher.close();
        assertThrows(AlreadyClosedException.class, () -> publisher.createDataWriter(topic));
        subscriber.close();
        assertThrows(AlreadyClosedException.class, () -> subscriber.createDataReader(topic));
    }

    @Test
    void aNullArgumentIsABadParameter() throws Exception {
        // RETCODE_BAD_PARAMETER -> java.lang.IllegalArgumentException (Table 7.1)
        assertThrows(IllegalArgumentException.class,
                () -> writer.getMatchedSubscriptionData(null));
        // The same writer accepts a real sample — the failure above is the
        // argument, not the entity.
        writer.write(new byte[] {1, 2});
        assertEquals(1, Qos.take(reader).size());
    }
}