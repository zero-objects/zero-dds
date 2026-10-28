// SPDX-License-Identifier: Apache-2.0
package org.omg.dds;

import org.omg.dds.core.ServiceEnvironment;
import org.omg.dds.core.policy.PolicyFactory;
import org.omg.dds.domain.DomainParticipantFactory;
import org.omg.dds.pub.DataWriter;
import org.omg.dds.pub.DataWriterQos;
import org.omg.dds.pub.Publisher;
import org.omg.dds.pub.PublisherQos;
import org.omg.dds.sub.DataReader;
import org.omg.dds.sub.DataReaderQos;
import org.omg.dds.sub.Sample;
import org.omg.dds.sub.Subscriber;
import org.omg.dds.sub.SubscriberQos;
import org.omg.dds.topic.Topic;
import org.omg.dds.topic.TopicDescription;
import org.omg.dds.topic.TopicQos;
import org.zerodds.ServiceEnvironmentImpl;
import org.zerodds.core.policy.PolicyFactoryImpl;
import org.zerodds.core.qos.DataReaderQosImpl;
import org.zerodds.core.qos.DataWriterQosImpl;
import org.zerodds.core.qos.PublisherQosImpl;
import org.zerodds.core.qos.SubscriberQosImpl;
import org.zerodds.core.qos.TopicQosImpl;
import org.zerodds.pub.DataWriterImpl;
import org.zerodds.pub.PublisherImpl;
import org.zerodds.sub.DataReaderImpl;
import org.zerodds.sub.SubscriberImpl;

import org.zerodds.cdr.EndianMode;
import org.zerodds.cdr.ExtensibilityKind;
import org.zerodds.cdr.TopicTypeSupport;

import java.nio.ByteBuffer;

/**
 * The QoS of DDS 1.2 §7.1.3's defaults, one factory per entity, for the tests.
 *
 * <p>The PSM models the QoS of each entity separately (§7.3.5.2), so a test
 * names the one it means; this class is where the defaults of those seven
 * interfaces live.
 */
public final class Qos {

    private Qos() {}

    public static ServiceEnvironment env() {
        return ServiceEnvironmentImpl.getDefaultEnvironment();
    }

    /**
     * The participant factory of {@link #env()} — §7.4.1: one per
     * ServiceEnvironment, reached through
     * {@code DomainParticipantFactory.getInstance(env)}.
     */
    public static DomainParticipantFactory factory() {
        return DomainParticipantFactory.getInstance(env());
    }

    /**
     * Creates a writer whose codec the test supplies, rather than the one the
     * topic carries: {@code PublisherImpl.createDataWriter(Topic,
     * TopicTypeSupport, DataWriterQos)}, the ZeroDDS form behind §7.6.2.
     */
    public static <T> DataWriter<T> writer(Publisher publisher, Topic<T> topic,
            TopicTypeSupport<T> support) {
        return ((PublisherImpl) publisher).createDataWriter(topic, support);
    }

    public static <T> DataWriter<T> writer(Publisher publisher, Topic<T> topic,
            TopicTypeSupport<T> support, DataWriterQos qos) {
        return ((PublisherImpl) publisher).createDataWriter(topic, support, qos);
    }

    /** The reader counterpart of {@link #writer}. */
    public static <T> DataReader<T> reader(Subscriber subscriber, TopicDescription<T> topic,
            TopicTypeSupport<T> support) {
        return ((SubscriberImpl) subscriber).createDataReader(topic, support);
    }

    public static <T> DataReader<T> reader(Subscriber subscriber, TopicDescription<T> topic,
            TopicTypeSupport<T> support, DataReaderQos qos) {
        return ((SubscriberImpl) subscriber).createDataReader(topic, support, qos);
    }

    /** §7.7.3 read — the samples, as the plain list the clause describes. */
    public static <T> java.util.List<Sample<T>> read(DataReader<T> reader) {
        return reader.read(new java.util.ArrayList<>());
    }

    /** §7.7.3 take — the samples removed from the reader, as a list. */
    public static <T> java.util.List<Sample<T>> take(DataReader<T> reader) {
        return reader.take(new java.util.ArrayList<>());
    }

    /** §2.2.2.4.2.16 dispose of the instance the sample belongs to. */
    public static <T> void dispose(DataWriter<T> writer, T instanceData) {
        ((DataWriterImpl<T>) writer).dispose(instanceData);
    }

    /** §2.2.2.4.2.15 unregister_instance of the instance the sample belongs to. */
    public static <T> void unregister(DataWriter<T> writer, T instanceData) {
        ((DataWriterImpl<T>) writer).unregisterInstance(instanceData);
    }

    /** Whether an entity of this binding has been closed (§7.2.3). */
    public static boolean isClosed(org.omg.dds.core.Entity<?, ?> entity) {
        if (entity instanceof DataWriterImpl) {
            return ((DataWriterImpl<?>) entity).isClosed();
        }
        if (entity instanceof DataReaderImpl) {
            return ((DataReaderImpl<?>) entity).isClosed();
        }
        if (entity instanceof PublisherImpl) {
            return ((PublisherImpl) entity).isClosed();
        }
        if (entity instanceof SubscriberImpl) {
            return ((SubscriberImpl) entity).isClosed();
        }
        if (entity instanceof org.zerodds.domain.DomainParticipantImpl) {
            return ((org.zerodds.domain.DomainParticipantImpl) entity).isClosed();
        }
        if (entity instanceof org.zerodds.topic.TopicImpl) {
            return ((org.zerodds.topic.TopicImpl<?>) entity).isClosed();
        }
        throw new IllegalArgumentException("entity of another implementation: " + entity);
    }

    /** The environment's default-initialized policies (§7.3.5.1, Table 7.2). */
    public static PolicyFactoryImpl policies() {
        return (PolicyFactoryImpl) env().getSPI().getPolicyFactory();
    }

    public static PolicyFactory pf() {
        return env().getSPI().getPolicyFactory();
    }

    public static DataWriterQos writer() {
        return DataWriterQosImpl.defaults(env());
    }

    public static DataReaderQos reader() {
        return DataReaderQosImpl.defaults(env());
    }

    /**
     * The DataReader QoS with HISTORY KEEP_ALL. The default is KEEP_LAST with depth 1
     * (DDS 1.4 §2.2.3.18), which keeps one sample per instance; a test that writes
     * several samples of one instance and takes them all asks for KEEP_ALL.
     */
    public static DataReaderQos readerKeepAll() {
        return reader().withPolicy(pf().History().withKeepAll());
    }

    public static PublisherQos publisher() {
        return PublisherQosImpl.defaults(env());
    }

    public static SubscriberQos subscriber() {
        return SubscriberQosImpl.defaults(env());
    }

    public static TopicQos topic() {
        return TopicQosImpl.defaults(env());
    }

    /** Writes one sample into {@code buf}. */
    interface Writer<T> {
        void write(T value, ByteBuffer buf);
    }

    /** Reads one sample from {@code buf}. */
    interface Reader<T> {
        T read(ByteBuffer buf);
    }

    /**
     * A hand-written {@link TopicTypeSupport} for a test's record type, in
     * place of what the IDL-to-Java generator emits for a real topic type.
     */
    public static <T> TopicTypeSupport<T> support(String typeName, Writer<T> writer, Reader<T> reader) {
        return new TopicTypeSupport<>() {
            @Override
            public String getTypeName() {
                return typeName;
            }

            @Override
            public void serialize(T value, ByteBuffer buf) {
                writer.write(value, buf);
            }

            @Override
            public T deserialize(ByteBuffer buf) {
                return reader.read(buf);
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
            public byte[] encode(T value) {
                ByteBuffer buf = ByteBuffer.allocate(256);
                serialize(value, buf);
                byte[] out = new byte[buf.position()];
                buf.flip();
                buf.get(out);
                return out;
            }

            @Override
            public byte[] encode(T value, EndianMode endian) {
                return encode(value);
            }

            @Override
            public T decode(byte[] bytes) {
                return decode(bytes, 0, bytes.length);
            }

            @Override
            public T decode(byte[] bytes, int offset, int length) {
                return deserialize(ByteBuffer.wrap(bytes, offset, length));
            }
        };
    }
}