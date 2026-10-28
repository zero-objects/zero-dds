// SPDX-License-Identifier: Apache-2.0
package org.omg.dds;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.omg.dds.core.AlreadyClosedException;
import org.omg.dds.core.NotEnabledException;
import org.omg.dds.domain.DomainParticipant;
import org.omg.dds.pub.DataWriter;
import org.omg.dds.pub.Publisher;
import org.omg.dds.topic.Topic;
import org.zerodds.internal.InProcessBus;

import java.util.concurrent.TimeUnit;

import static org.junit.jupiter.api.Assertions.assertThrows;

/**
 * wait_for_acknowledgments of the DataWriter (DDS 1.4 §2.2.2.4.2.12) for the readers of the
 * process; the wait on readers of other processes, over ACKNACK, is tested with the state that
 * tracks them in {@code org.zerodds.rtps.WriterAckStateTest}. Domain ids 1370 to 1379.
 */
class WriterWaitAckTest {

    @BeforeEach
    void setUp() {
        InProcessBus.instance().reset();
    }

    @AfterEach
    void tearDown() {
        InProcessBus.instance().reset();
    }

    @Test
    void aWriterWhoseReadersAreInTheProcessIsAcknowledgedWhenWriteReturns() throws Exception {
        DomainParticipant participant = Qos.factory().createParticipant(1370);
        Topic<byte[]> topic = participant.createTopic("AckInProcess", byte[].class);
        DataWriter<byte[]> writer = participant.createPublisher().createDataWriter(topic,
                Qos.writer().withPolicy(Qos.pf().Reliability().withReliable()));
        participant.createSubscriber().createDataReader(topic,
                Qos.reader().withPolicy(Qos.pf().Reliability().withReliable()));
        writer.write(new byte[] {1});
        writer.waitForAcknowledgments(0, TimeUnit.SECONDS);
        writer.waitForAcknowledgments(5, TimeUnit.SECONDS);
        writer.waitForAcknowledgments(Qos.env().getSPI().zeroDuration());
        writer.waitForAcknowledgments(Qos.env().getSPI().infiniteDuration());
    }

    @Test
    void aWriterWithoutReadersHasNothingOutstanding() throws Exception {
        DomainParticipant participant = Qos.factory().createParticipant(1371);
        DataWriter<byte[]> writer = participant.createPublisher().createDataWriter(
                participant.createTopic("AckNoReaders", byte[].class));
        writer.write(new byte[] {1});
        writer.waitForAcknowledgments(1, TimeUnit.SECONDS);
    }

    @Test
    void anArgumentThatIsNotADurationIsRefused() {
        DomainParticipant participant = Qos.factory().createParticipant(1372);
        DataWriter<byte[]> writer = participant.createPublisher().createDataWriter(
                participant.createTopic("AckArguments", byte[].class));
        assertThrows(IllegalArgumentException.class,
                () -> writer.waitForAcknowledgments(-1, TimeUnit.SECONDS));
        assertThrows(IllegalArgumentException.class,
                () -> writer.waitForAcknowledgments(null));
    }

    @Test
    void aClosedWriterAndAWriterThatIsNotEnabledRefuseTheWait() {
        DomainParticipant participant = Qos.factory().createParticipant(1373);
        Topic<byte[]> topic = participant.createTopic("AckState", byte[].class);
        Publisher lazy = participant.createPublisher(
                Qos.publisher().withPolicy(Qos.policies().EntityFactory()
                        .withAutoEnableCreatedEntities(false)));
        DataWriter<byte[]> disabled = lazy.createDataWriter(topic);
        assertThrows(NotEnabledException.class,
                () -> disabled.waitForAcknowledgments(1, TimeUnit.SECONDS));

        DataWriter<byte[]> closed = participant.createPublisher().createDataWriter(topic);
        closed.close();
        assertThrows(AlreadyClosedException.class,
                () -> closed.waitForAcknowledgments(1, TimeUnit.SECONDS));
    }
}
