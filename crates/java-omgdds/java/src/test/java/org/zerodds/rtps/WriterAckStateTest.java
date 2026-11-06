// SPDX-License-Identifier: Apache-2.0
package org.zerodds.rtps;

import org.zerodds.testing.Waits;
import java.net.InetSocketAddress;

import org.junit.jupiter.api.Test;
import org.zerodds.cdr.HexBytes;

import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * The send history of a reliable writer is what wait_for_acknowledgments waits on
 * (DDS 1.4 §2.2.2.4.2.12, DDSI-RTPS 2.5 §8.4.7): the writer is acknowledged when every sample
 * has been acknowledged by every reader it was sent to.
 */
class WriterAckStateTest {

    private static final byte[] PREFIX = {1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12};
    private static final byte[] READER_A = {0, 0, 1, 4};
    private static final byte[] READER_B = {0, 0, 2, 4};
    private static final byte[] WRITER = {0, 0, 1, 3};
    private static final byte[] PAYLOAD = {1, 2, 3};

    private static ReliableWriterState.Target target(byte[] reader) {
        return new ReliableWriterState.Target(PREFIX, reader,
                InetSocketAddress.createUnresolved("localhost", 7411));
    }

    private static RtpsMessage.AckNack ack(byte[] reader, long base, int count) {
        RtpsMessage.AckNack ack = new RtpsMessage.AckNack();
        ack.readerId = reader;
        ack.writerId = WRITER;
        ack.base = base;
        ack.count = count;
        ack.numBits = 0;
        ack.bitmap = new long[0];
        return ack;
    }

    @Test
    void javapsm_2_a_writer_that_sent_nothing_has_nothing_to_wait_for() throws Exception {
        ReliableWriterState state = new ReliableWriterState(WRITER);
        assertTrue(state.allAcknowledged());
        assertTrue(state.awaitAcknowledged(0L));
    }

    @Test
    void javapsm_2_wait_for_acknowledgments_times_out_when_nobody_acknowledged() throws Exception {
        ReliableWriterState state = new ReliableWriterState(WRITER);
        state.add(1, PAYLOAD, target(READER_A));
        assertFalse(state.allAcknowledged());
        long start = System.nanoTime();
        assertFalse(state.awaitAcknowledged(60_000_000L));
        assertTrue(System.nanoTime() - start >= 60_000_000L, "the wait lasts the time given");
    }

    @Test
    void javapsm_2_an_acknack_beyond_the_sample_acknowledges_it() throws Exception {
        ReliableWriterState state = new ReliableWriterState(WRITER);
        state.add(1, PAYLOAD, target(READER_A));
        // base 1: the reader still misses sequence number 1
        assertTrue(state.acknowledge(PREFIX, ack(READER_A, 1, 1)));
        assertFalse(state.allAcknowledged());
        assertFalse(state.awaitAcknowledged(20_000_000L));
        // base 2: it has everything below 2
        assertTrue(state.acknowledge(PREFIX, ack(READER_A, 2, 2)));
        assertTrue(state.allAcknowledged());
        assertTrue(state.awaitAcknowledged(0L));
    }

    @Test
    void javapsm_2_a_waiter_is_released_by_the_acknack_of_another_thread() throws Exception {
        ReliableWriterState state = new ReliableWriterState(WRITER);
        state.add(1, PAYLOAD, target(READER_A));
        Thread waiter = Thread.currentThread();
        Thread acknowledger = new Thread(() -> {
            Waits.untilWaitingIn(waiter, "awaitAcknowledged");
            state.acknowledge(PREFIX, ack(READER_A, 2, 1));
        });
        acknowledger.start();
        assertTrue(state.awaitAcknowledged(30_000_000_000L));
        acknowledger.join();
        assertTrue(state.awaitAcknowledged(Long.MAX_VALUE));
    }

    @Test
    void javapsm_2_every_reader_that_received_the_sample_has_to_acknowledge_it() throws Exception {
        ReliableWriterState state = new ReliableWriterState(WRITER);
        state.add(1, PAYLOAD, target(READER_A));
        state.add(1, PAYLOAD, target(READER_B));
        state.acknowledge(PREFIX, ack(READER_A, 2, 1));
        assertFalse(state.allAcknowledged(), "the second reader has not acknowledged");
        state.acknowledge(PREFIX, ack(READER_B, 2, 1));
        assertTrue(state.allAcknowledged());
    }

    @Test
    void javapsm_2_a_reader_that_leaves_is_not_waited_for() throws Exception {
        ReliableWriterState state = new ReliableWriterState(WRITER);
        state.add(1, PAYLOAD, target(READER_A));
        assertFalse(state.allAcknowledged());
        state.removePeer(HexBytes.encode(PREFIX));
        assertTrue(state.allAcknowledged());
        assertTrue(state.awaitAcknowledged(0L));
    }
}
