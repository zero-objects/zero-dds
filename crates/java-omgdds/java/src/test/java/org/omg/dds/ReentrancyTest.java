// SPDX-License-Identifier: Apache-2.0
package org.omg.dds;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.omg.dds.core.event.DataAvailableEvent;
import org.omg.dds.domain.DomainParticipant;
import org.omg.dds.domain.DomainParticipantFactory;
import org.omg.dds.pub.DataWriter;
import org.zerodds.pub.DataWriterImpl;
import org.omg.dds.sub.DataReader;
import org.omg.dds.sub.DataReaderAdapter;
import org.omg.dds.topic.Topic;
import org.zerodds.cdr.EndianMode;
import org.zerodds.cdr.ExtensibilityKind;
import org.zerodds.cdr.TopicTypeSupport;
import org.zerodds.internal.InProcessBus;
import org.zerodds.internal.Xcdr2Codec;

import java.nio.ByteBuffer;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.CyclicBarrier;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicBoolean;
import java.util.concurrent.atomic.AtomicReference;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * DDS-Java-PSM 1.0 §7.2.4 (Concurrency and Reentrancy): "All DataReader and
 * DataWriter operations shall be reentrant." A callback that runs while the
 * implementation still holds an entity's monitor turns any concurrent call on
 * that entity into a wait, so the two tests below block a listener callback on a
 * latch and require another thread to complete an operation on the very entity
 * the delivering thread is inside of.
 */
class ReentrancyTest {

    record Reading(int sensorId, double value) {}

    static final TopicTypeSupport<Reading> SUPPORT = new TopicTypeSupport<>() {
        @Override
        public String getTypeName() {
            return "ReentrancyReading";
        }

        @Override
        public void serialize(Reading r, ByteBuffer buf) {
            Xcdr2Codec.writeInt(buf, r.sensorId());
            Xcdr2Codec.writeDouble(buf, r.value());
        }

        @Override
        public Reading deserialize(ByteBuffer buf) {
            return new Reading(Xcdr2Codec.readInt(buf), Xcdr2Codec.readDouble(buf));
        }

        @Override
        public boolean isKeyed() {
            return false;
        }

        @Override
        public ExtensibilityKind getExtensibility() {
            return ExtensibilityKind.APPENDABLE;
        }

        @Override
        public byte[] encode(Reading r) {
            ByteBuffer buf = ByteBuffer.allocate(256);
            serialize(r, buf);
            byte[] out = new byte[buf.position()];
            buf.flip();
            buf.get(out);
            return out;
        }

        @Override
        public byte[] encode(Reading r, EndianMode endian) {
            return encode(r);
        }

        @Override
        public Reading decode(byte[] bytes) {
            return decode(bytes, 0, bytes.length);
        }

        @Override
        public Reading decode(byte[] bytes, int offset, int length) {
            return deserialize(ByteBuffer.wrap(bytes, offset, length));
        }

    };

    private static final int DOMAIN = 61;

    private DomainParticipant participant;
    private Topic<Reading> topic;

    @BeforeEach
    void setUp() {
        InProcessBus.instance().reset();
        participant = Qos.factory().createParticipant(DOMAIN);
        topic = participant.createTopic("Reentrancy", Reading.class);
    }

    @AfterEach
    void tearDown() {
        participant.close();
    }

    /** {@code write} declares TimeoutException (§7.6.2); the thread has nowhere to put it. */
    private static <T> void write(DataWriter<T> writer, T sample) {
        try {
            writer.write(sample);
        } catch (java.util.concurrent.TimeoutException e) {
            throw new IllegalStateException(e);
        }
    }

    /** A callback parked until {@code release} — the callback is the held monitor. */
    private static final class BlockingReaderListener extends DataReaderAdapter<Reading> {
        final CountDownLatch entered = new CountDownLatch(1);
        final CountDownLatch release = new CountDownLatch(1);
        private final AtomicBoolean blockNext = new AtomicBoolean(true);

        @Override
        public void onDataAvailable(DataAvailableEvent<Reading> status) {
            if (!blockNext.compareAndSet(true, false)) {
                return; // only the first sample parks, the others run straight through
            }
            entered.countDown();
            try {
                release.await(5, TimeUnit.SECONDS);
            } catch (InterruptedException e) {
                Thread.currentThread().interrupt();
            }
        }
    }

    @Test
    void aCallbackDoesNotHoldTheReaderMonitor() throws Exception {
        DataWriter<Reading> writer = Qos.writer(participant.createPublisher(), topic, SUPPORT);
        DataReader<Reading> reader = Qos.reader(participant.createSubscriber(), topic, SUPPORT);
        BlockingReaderListener listener = new BlockingReaderListener();
        reader.setListener(listener);

        Thread writerThread = new Thread(() -> write(writer, new Reading(1, 1.0)));
        writerThread.start();
        assertTrue(listener.entered.await(5, TimeUnit.SECONDS), "onDataAvailable never ran");

        CountDownLatch closed = new CountDownLatch(1);
        Thread closer = new Thread(() -> {
            reader.close();
            closed.countDown();
        });
        closer.start();
        try {
            assertTrue(closed.await(2, TimeUnit.SECONDS),
                    "close() on the DataReader blocked while its listener callback ran");
        } finally {
            listener.release.countDown();
            writerThread.join(5000);
            closer.join(5000);
        }
        assertTrue(Qos.isClosed(reader));
    }

    @Test
    void aCallbackDoesNotHoldTheWriterMonitor() throws Exception {
        DataWriter<Reading> writer = Qos.writer(participant.createPublisher(), topic, SUPPORT);
        DataReader<Reading> reader = Qos.reader(participant.createSubscriber(), topic, SUPPORT, Qos.readerKeepAll());
        BlockingReaderListener listener = new BlockingReaderListener();
        reader.setListener(listener);

        Thread writerThread = new Thread(() -> write(writer, new Reading(1, 1.0)));
        writerThread.start();
        assertTrue(listener.entered.await(5, TimeUnit.SECONDS), "onDataAvailable never ran");

        AtomicReference<Throwable> failure = new AtomicReference<>();
        CountDownLatch written = new CountDownLatch(1);
        Thread other = new Thread(() -> {
            try {
                writer.write(new Reading(2, 2.0));
            } catch (java.util.concurrent.TimeoutException | RuntimeException e) {
                failure.set(e);
            }
            written.countDown();
        });
        other.start();
        try {
            assertTrue(written.await(2, TimeUnit.SECONDS),
                    "write() on the DataWriter blocked while its reader's callback ran");
        } finally {
            listener.release.countDown();
            writerThread.join(5000);
            other.join(5000);
        }
        assertNull(failure.get(), "the concurrent write failed: " + failure.get());
        assertEquals(2, Qos.take(reader).size());
    }

    @Test
    void concurrentEnableRegistersTheEndpointOnce() throws Exception {
        DataWriterImpl<Reading> writer = new DataWriterImpl<>(DOMAIN, null, topic, SUPPORT,
                Qos.writer(), java.util.Set.of());
        int threads = 8;
        CyclicBarrier gate = new CyclicBarrier(threads);
        Thread[] callers = new Thread[threads];
        for (int i = 0; i < threads; i++) {
            callers[i] = new Thread(() -> {
                try {
                    gate.await(5, TimeUnit.SECONDS);
                } catch (Exception e) {
                    Thread.currentThread().interrupt();
                }
                writer.enable();
            });
            callers[i].start();
        }
        for (Thread t : callers) {
            t.join(5000);
        }
        assertEquals(1, InProcessBus.instance()
                .writers(DOMAIN, topic.getName(), SUPPORT.getTypeName()).size(),
                "enable() registered the writer more than once");
        writer.close();
        assertEquals(0, InProcessBus.instance()
                .writers(DOMAIN, topic.getName(), SUPPORT.getTypeName()).size());
    }
}