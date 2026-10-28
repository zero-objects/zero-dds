// SPDX-License-Identifier: Apache-2.0
package org.omg.dds;

import org.omg.dds.domain.DomainParticipant;
import org.omg.dds.pub.DataWriter;
import org.omg.dds.pub.DataWriterQos;
import org.omg.dds.pub.Publisher;
import org.omg.dds.pub.PublisherQos;
import org.omg.dds.sub.DataReader;
import org.omg.dds.sub.DataReaderQos;
import org.omg.dds.sub.Subscriber;
import org.omg.dds.sub.SubscriberQos;
import org.omg.dds.topic.Topic;
import org.zerodds.cdr.EndianMode;
import org.zerodds.cdr.ExtensibilityKind;
import org.zerodds.cdr.TopicTypeSupport;

import java.nio.ByteBuffer;
import java.util.concurrent.atomic.AtomicInteger;

/**
 * What the {@code Qos*Test} classes of the QoS area share: a keyed topic type, domain ids
 * from 1400 to 1499, and a participant with a topic, a publisher and a subscriber.
 */
final class QosFixtures {

    private QosFixtures() {}

    /** {@code struct Item { @key long id; long seq; }}. */
    record Item(int id, int seq) {}

    /** The codec of {@link Item}: two big-endian 32-bit integers, the key is {@code id}. */
    static final TopicTypeSupport<Item> SUPPORT = new TopicTypeSupport<>() {
        @Override
        public String getTypeName() {
            return "QosItem";
        }

        @Override
        public void serialize(Item item, ByteBuffer buf) {
            buf.putInt(item.id());
            buf.putInt(item.seq());
        }

        @Override
        public Item deserialize(ByteBuffer buf) {
            return new Item(buf.getInt(), buf.getInt());
        }

        @Override
        public boolean isKeyed() {
            return true;
        }

        @Override
        public byte[] keyHash(Item item) {
            return ByteBuffer.allocate(4).putInt(item.id()).array();
        }

        @Override
        public ExtensibilityKind getExtensibility() {
            return ExtensibilityKind.APPENDABLE;
        }

        @Override
        public byte[] encode(Item item) {
            ByteBuffer buf = ByteBuffer.allocate(8);
            serialize(item, buf);
            return buf.array();
        }

        @Override
        public byte[] encode(Item item, EndianMode endian) {
            return encode(item);
        }

        @Override
        public Item decode(byte[] bytes) {
            return decode(bytes, 0, bytes.length);
        }

        @Override
        public Item decode(byte[] bytes, int offset, int length) {
            return deserialize(ByteBuffer.wrap(bytes, offset, length));
        }
    };

    private static final AtomicInteger NEXT_DOMAIN = new AtomicInteger();

    /** A domain id of 1400 to 1499; a test closes its participants, so an id can come round again. */
    static int nextDomain() {
        return 1400 + Math.floorMod(NEXT_DOMAIN.getAndIncrement(), 100);
    }

    /** One participant on a domain of its own with the topic {@code Items}, a publisher and a subscriber. */
    static final class Rig implements AutoCloseable {
        final DomainParticipant participant;
        final Topic<Item> topic;
        final Publisher publisher;
        final Subscriber subscriber;

        Rig() {
            this(Qos.publisher(), Qos.subscriber());
        }

        Rig(PublisherQos publisherQos, SubscriberQos subscriberQos) {
            participant = Qos.factory().createParticipant(nextDomain());
            topic = participant.createTopic("Items", Item.class);
            publisher = participant.createPublisher(publisherQos);
            subscriber = participant.createSubscriber(subscriberQos);
        }

        DataWriter<Item> writer(DataWriterQos qos) {
            return Qos.writer(publisher, topic, SUPPORT, qos);
        }

        DataReader<Item> reader(DataReaderQos qos) {
            return Qos.reader(subscriber, topic, SUPPORT, qos);
        }

        @Override
        public void close() {
            participant.close();
        }
    }
}
