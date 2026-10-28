// SPDX-License-Identifier: Apache-2.0
package org.omg.dds;

import org.omg.dds.domain.DomainParticipant;
import org.omg.dds.pub.DataWriter;
import org.omg.dds.pub.Publisher;
import org.omg.dds.sub.DataReader;
import org.omg.dds.sub.DataReaderQos;
import org.omg.dds.sub.Subscriber;
import org.omg.dds.topic.Topic;
import org.zerodds.cdr.EndianMode;
import org.zerodds.cdr.ExtensibilityKind;
import org.zerodds.cdr.TopicTypeSupport;

import java.nio.ByteBuffer;
import java.nio.charset.StandardCharsets;

/**
 * A participant with a keyed topic for the Reader* tests: struct Keyed { @key int id; int seq;
 * double value; } with the key as the 4 bytes of {@code id}, big-endian, so the instance
 * handles of ids 1, 2, 3 are ordered by id (an InstanceHandle compares its 16 octets as
 * unsigned values).
 */
public final class ReaderRig {

    /** The sample type. */
    public record Keyed(int id, int seq, double value) {
    }

    /** A sample type with a key longer than 16 octets: the name. */
    public record Named(String name, int seq) {
    }

    /** DDS-DCPS 1.4 §2.2.1.2.2: the key of {@link Keyed} is {@code id}. */
    public static final TopicTypeSupport<Keyed> SUPPORT = new TopicTypeSupport<>() {
        @Override
        public String getTypeName() {
            return "ReaderKeyed";
        }

        @Override
        public void serialize(Keyed value, ByteBuffer buf) {
            buf.putInt(value.id());
            buf.putInt(value.seq());
            buf.putDouble(value.value());
        }

        @Override
        public Keyed deserialize(ByteBuffer buf) {
            return new Keyed(buf.getInt(), buf.getInt(), buf.getDouble());
        }

        @Override
        public boolean isKeyed() {
            return true;
        }

        @Override
        public byte[] keyHash(Keyed value) {
            return ByteBuffer.allocate(4).putInt(value.id()).array();
        }

        @Override
        public ExtensibilityKind getExtensibility() {
            return ExtensibilityKind.APPENDABLE;
        }

        @Override
        public byte[] encode(Keyed value) {
            ByteBuffer buf = ByteBuffer.allocate(256);
            serialize(value, buf);
            byte[] out = new byte[buf.position()];
            buf.flip();
            buf.get(out);
            return out;
        }

        @Override
        public byte[] encode(Keyed value, EndianMode endian) {
            return encode(value);
        }

        @Override
        public Keyed decode(byte[] bytes) {
            return decode(bytes, 0, bytes.length);
        }

        @Override
        public Keyed decode(byte[] bytes, int offset, int length) {
            return deserialize(ByteBuffer.wrap(bytes, offset, length));
        }
    };

    /** The key of {@link Named} is the UTF-8 bytes of {@code name}. */
    public static final TopicTypeSupport<Named> NAMED = new TopicTypeSupport<>() {
        @Override
        public String getTypeName() {
            return "ReaderNamed";
        }

        @Override
        public void serialize(Named value, ByteBuffer buf) {
            byte[] name = value.name().getBytes(StandardCharsets.UTF_8);
            buf.putInt(name.length);
            buf.put(name);
            buf.putInt(value.seq());
        }

        @Override
        public Named deserialize(ByteBuffer buf) {
            byte[] name = new byte[buf.getInt()];
            buf.get(name);
            return new Named(new String(name, StandardCharsets.UTF_8), buf.getInt());
        }

        @Override
        public boolean isKeyed() {
            return true;
        }

        @Override
        public byte[] keyHash(Named value) {
            return value.name().getBytes(StandardCharsets.UTF_8);
        }

        @Override
        public ExtensibilityKind getExtensibility() {
            return ExtensibilityKind.APPENDABLE;
        }

        @Override
        public byte[] encode(Named value) {
            ByteBuffer buf = ByteBuffer.allocate(256);
            serialize(value, buf);
            byte[] out = new byte[buf.position()];
            buf.flip();
            buf.get(out);
            return out;
        }

        @Override
        public byte[] encode(Named value, EndianMode endian) {
            return encode(value);
        }

        @Override
        public Named decode(byte[] bytes) {
            return decode(bytes, 0, bytes.length);
        }

        @Override
        public Named decode(byte[] bytes, int offset, int length) {
            return deserialize(ByteBuffer.wrap(bytes, offset, length));
        }
    };

    public final DomainParticipant participant;
    public final Topic<Keyed> topic;
    public final Publisher publisher;
    public final Subscriber subscriber;

    public ReaderRig(int domain) {
        participant = Qos.factory().createParticipant(domain);
        topic = participant.createTopic("ReaderKeyedTopic", Keyed.class);
        publisher = participant.createPublisher();
        subscriber = participant.createSubscriber();
    }

    public DataReader<Keyed> reader(DataReaderQos qos) {
        return Qos.reader(subscriber, topic, SUPPORT, qos);
    }

    /** A reader with HISTORY KEEP_ALL. */
    public DataReader<Keyed> keepAllReader() {
        return reader(Qos.readerKeepAll());
    }

    public DataWriter<Keyed> writer() {
        // the tests that unregister expect NOT_ALIVE_NO_WRITERS; an unregister disposes first
        // when autodispose_unregistered_instances is true (DDS 1.4 2.2.3.21)
        return Qos.writer(publisher, topic, SUPPORT, Qos.writer().withPolicy(
                Qos.pf().WriterDataLifecycle().withAutDisposeUnregisteredInstances(false)));
    }

    /** The sample {@code Keyed(id, seq, seq)}. */
    public static Keyed sample(int id, int seq) {
        return new Keyed(id, seq, seq);
    }
}
