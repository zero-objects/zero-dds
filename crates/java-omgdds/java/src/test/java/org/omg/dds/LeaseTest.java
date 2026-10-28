// SPDX-License-Identifier: Apache-2.0
package org.omg.dds;

import org.zerodds.core.Values;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.omg.dds.core.Duration;
import org.omg.dds.core.policy.Deadline;
import org.omg.dds.core.policy.Liveliness;
import org.omg.dds.pub.DataWriterQos;
import org.omg.dds.sub.DataReaderQos;
import org.omg.dds.core.event.LivelinessChangedEvent;
import org.omg.dds.core.event.LivelinessLostEvent;
import org.omg.dds.core.event.OfferedDeadlineMissedEvent;
import org.omg.dds.core.event.RequestedDeadlineMissedEvent;
import org.omg.dds.domain.DomainParticipant;
import org.omg.dds.domain.DomainParticipantFactory;
import org.omg.dds.pub.DataWriter;
import org.omg.dds.pub.DataWriterAdapter;
import org.omg.dds.pub.Publisher;
import org.omg.dds.sub.DataReader;
import org.omg.dds.sub.DataReaderAdapter;
import org.omg.dds.sub.Subscriber;
import org.omg.dds.topic.Topic;
import org.zerodds.cdr.EndianMode;
import org.zerodds.cdr.ExtensibilityKind;
import org.zerodds.cdr.TopicTypeSupport;
import org.zerodds.internal.InProcessBus;

import java.nio.ByteBuffer;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.TimeUnit;

import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * DDS 1.2 §7.1.4.2 — listeners alert the application of deadline misses and
 * liveliness changes asynchronously, via a lease, without the application
 * polling the status. Each test asserts the callback arrives while the status
 * getter is never called.
 */
class LeaseTest {

    record Reading(int sensorId, double value) {}

    static final TopicTypeSupport<Reading> SUPPORT = new TopicTypeSupport<>() {
        public String getTypeName() { return "Reading"; }
        public void serialize(Reading r, ByteBuffer buf) {
            buf.putInt(r.sensorId()); buf.putDouble(r.value());
        }
        public Reading deserialize(ByteBuffer buf) {
            return new Reading(buf.getInt(), buf.getDouble());
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

    @BeforeEach
    void setUp() { InProcessBus.instance().reset(); }
    @AfterEach
    void tearDown() { InProcessBus.instance().reset(); }

    private static void await(CountDownLatch latch) throws InterruptedException {
        assertTrue(latch.await(5, TimeUnit.SECONDS), "listener did not fire within timeout");
    }

    @Test
    void requestedDeadlineMissedFiresWithoutPolling() throws Exception {
        DomainParticipant p =
                Qos.factory().createParticipant(510);
        Topic<Reading> t = p.createTopic("DeadlineLease", Reading.class);
        DataReaderQos q = Qos.reader().withPolicy(Qos.policies().Deadline().withPeriod(30, TimeUnit.MILLISECONDS));
        DataWriterQos wq = Qos.writer().withPolicy(
                Qos.policies().Deadline().withPeriod(30, TimeUnit.MILLISECONDS));
        try (Publisher pub = p.createPublisher();
             Subscriber sub = p.createSubscriber();
             DataReader<Reading> dr = Qos.reader(sub, t, SUPPORT, q);
             DataWriter<Reading> dw = Qos.writer(pub, t, SUPPORT, wq)) {

            CountDownLatch fired = new CountDownLatch(1);
            dr.setListener(new DataReaderAdapter<>() {
                @Override
                public void onRequestedDeadlineMissed(
                        RequestedDeadlineMissedEvent<Reading> status) {
                    if (status.getStatus().getTotalCount() >= 1) fired.countDown();
                }
            });
            dw.write(new Reading(1, 1.0)); // one sample, then silence
            await(fired); // the lease raises the miss without getRequestedDeadlineMissedStatus()
        }
        p.close();
    }

    @Test
    void offeredDeadlineMissedFiresWithoutPolling() throws Exception {
        DomainParticipant p =
                Qos.factory().createParticipant(511);
        Topic<Reading> t = p.createTopic("OfferedDeadlineLease", Reading.class);
        DataReaderQos q = Qos.reader().withPolicy(Qos.policies().Deadline().withPeriod(30, TimeUnit.MILLISECONDS));
        DataWriterQos wq = Qos.writer().withPolicy(
                Qos.policies().Deadline().withPeriod(30, TimeUnit.MILLISECONDS));
        try (Publisher pub = p.createPublisher();
             Subscriber sub = p.createSubscriber();
             DataReader<Reading> dr = Qos.reader(sub, t, SUPPORT, q);
             DataWriter<Reading> dw = Qos.writer(pub, t, SUPPORT, wq)) {

            CountDownLatch fired = new CountDownLatch(1);
            dw.setListener(new DataWriterAdapter<>() {
                @Override
                public void onOfferedDeadlineMissed(
                        OfferedDeadlineMissedEvent<Reading> status) {
                    if (status.getStatus().getTotalCount() >= 1) fired.countDown();
                }
            });
            dw.write(new Reading(1, 1.0));
            await(fired);
        }
        p.close();
    }

    @Test
    void livelinessLostFiresWithoutPolling() throws Exception {
        DomainParticipant p =
                Qos.factory().createParticipant(512);
        Topic<Reading> t = p.createTopic("LivelinessLostLease", Reading.class);
        DataReaderQos q = Qos.reader().withPolicy(
                Qos.policies().Liveliness().withManualByTopic().withLeaseDuration(30, TimeUnit.MILLISECONDS));
        // §2.2.3.11: LIVELINESS_LOST is the status of a writer that offers a finite lease
        // and does not assert it; the writer offers the lease the reader requests.
        DataWriterQos wq = Qos.writer().withPolicy(
                Qos.policies().Liveliness().withManualByTopic().withLeaseDuration(30, TimeUnit.MILLISECONDS));
        try (Publisher pub = p.createPublisher();
             Subscriber sub = p.createSubscriber();
             DataReader<Reading> dr = Qos.reader(sub, t, SUPPORT, q);
             DataWriter<Reading> dw = Qos.writer(pub, t, SUPPORT, wq)) {

            CountDownLatch fired = new CountDownLatch(1);
            dw.setListener(new DataWriterAdapter<>() {
                @Override
                public void onLivelinessLost(LivelinessLostEvent<Reading> status) {
                    if (status.getStatus().getTotalCount() >= 1) fired.countDown();
                }
            });
            dw.write(new Reading(1, 1.0)); // asserts, then never again
            await(fired); // the lease expires and reports without a status poll
        }
        p.close();
    }

    @Test
    void livelinessChangedFiresWithoutPolling() throws Exception {
        DomainParticipant p =
                Qos.factory().createParticipant(513);
        Topic<Reading> t = p.createTopic("LivelinessChangedLease", Reading.class);
        DataWriterQos writerQos = Qos.writer().withPolicy(Qos.policies().Liveliness().withKind(Liveliness.Kind.MANUAL_BY_TOPIC).withLeaseDuration(30, TimeUnit.MILLISECONDS));
        DataReaderQos readerQos = Qos.reader().withPolicy(Qos.policies().Liveliness().withKind(Liveliness.Kind.AUTOMATIC).withLeaseDuration(50, TimeUnit.MILLISECONDS));
        DataWriterQos defaultWriterQos = Qos.writer();
        try (Publisher pub = p.createPublisher();
             Subscriber sub = p.createSubscriber();
             DataReader<Reading> dr = Qos.reader(sub, t, SUPPORT, readerQos);
             DataWriter<Reading> dw = Qos.writer(pub, t, SUPPORT, writerQos)) {

            CountDownLatch fired = new CountDownLatch(1);
            dr.setListener(new DataReaderAdapter<>() {
                @Override
                public void onLivelinessChanged(LivelinessChangedEvent<Reading> status) {
                    if (status.getStatus().getNotAliveCount() >= 1) fired.countDown();
                }
            });
            dw.write(new Reading(1, 1.0)); // asserts liveliness, then stops
            await(fired); // reader's lease notices the writer go not-alive
        }
        p.close();
    }
}
