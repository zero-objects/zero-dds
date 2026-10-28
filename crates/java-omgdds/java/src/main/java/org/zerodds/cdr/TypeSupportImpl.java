// SPDX-License-Identifier: Apache-2.0
package org.zerodds.cdr;

import org.omg.dds.core.ServiceEnvironment;
import org.omg.dds.type.TypeSupport;

import java.nio.ByteBuffer;

/**
 * ZeroDDS {@link TypeSupport} (DDS-Java-PSM 1.0 §7.5.1, §7.8.2): the PSM
 * {@code TypeSupport} of one topic type, wrapping the ZeroDDS wire codec
 * {@link TopicTypeSupport} that actually serializes it.
 *
 * <p>§7.5.1 gives {@code createTopic} a {@code TypeSupport} so that a topic
 * knows the name its type is registered under; {@link #getTypeName()} is that
 * name, and {@link #encode}/{@link #decode} are the XCDR2 codec of the Java
 * Type Representation (Clause 8).
 */
public final class TypeSupportImpl<T> extends TypeSupport<T> {

    private final ServiceEnvironment environment;
    private final String typeName;
    private final Class<T> type;
    private final TopicTypeSupport<T> codec;
    private final java.util.function.Supplier<T> factory;

    public TypeSupportImpl(ServiceEnvironment environment, String typeName,
            Class<T> type, TopicTypeSupport<T> codec) {
        this(environment, typeName, type, codec, null);
    }

    /**
     * A TypeSupport whose samples are made by {@code factory} rather than by
     * the no-argument constructor of {@code type} (the samples of a
     * DynamicType are interface objects).
     */
    public TypeSupportImpl(ServiceEnvironment environment, String typeName,
            Class<T> type, TopicTypeSupport<T> codec, java.util.function.Supplier<T> factory) {
        this.environment = environment;
        this.typeName = typeName;
        this.type = type;
        this.codec = codec;
        this.factory = factory;
    }

    /** The ZeroDDS codec this TypeSupport wraps. */
    public TopicTypeSupport<T> codec() {
        return codec;
    }

    /** Appends the XCDR2 form of {@code value} to {@code buffer}. */
    public void encode(T value, ByteBuffer buffer) {
        codec.serialize(value, buffer);
    }

    /** Reads one XCDR2 value of this topic type from {@code buffer}. */
    public T decode(ByteBuffer buffer) {
        return codec.deserialize(buffer);
    }

    @Override
    public ServiceEnvironment getEnvironment() {
        return environment;
    }

    @Override
    public Class<T> getType() {
        return type;
    }

    @Override
    public String getTypeName() {
        return typeName;
    }

    @Override
    public T newData() {
        if (factory != null) {
            return factory.get();
        }
        // The built-in types of §7.8.2 have no constructor an application could call: an abstract
        // value type made through the SPI, and an array.
        if (org.omg.dds.type.builtin.KeyedString.class.isAssignableFrom(type)) {
            return type.cast(environment.getSPI().newKeyedString());
        }
        if (org.omg.dds.type.builtin.KeyedBytes.class.isAssignableFrom(type)) {
            return type.cast(environment.getSPI().newKeyedBytes());
        }
        if (type == byte[].class) {
            return type.cast(new byte[0]);
        }
        try {
            return type.getDeclaredConstructor().newInstance();
        } catch (ReflectiveOperationException e) {
            throw new XcdrException(
                    "cannot instantiate " + type.getName() + " for a new sample", e);
        }
    }

    @Override
    public String toString() {
        return "TypeSupport[" + typeName + "]";
    }
}