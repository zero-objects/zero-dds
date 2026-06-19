// SPDX-License-Identifier: Apache-2.0
package org.zerodds.cdr;

import java.nio.ByteBuffer;

/**
 * ZeroDDS-extended TypeSupport interface for XCDR2 bindings.
 *
 * <p>Spec: zerodds-xcdr2-java-1.0 §2. Extends the DDS Java PSM
 * marker interface {@link org.omg.dds.topic.TopicTypeSupport} with
 * concrete encode/decode/keyHash methods plus
 * extensibility reflection.
 *
 * <p>Generated code (idl-java) emits per IDL {@code struct} one
 * singleton class {@code <Name>TypeSupport} that implements this
 * interface.
 *
 * @param <T> sample class (POJO with bean accessors).
 */
public interface TopicTypeSupport<T> extends org.omg.dds.topic.TopicTypeSupport<T> {

    /** DDS-Type-Name (Convention {@code Module::Sub::Struct}, ASCII, max 256 Bytes). */
    @Override
    String getTypeName();

    /** {@code true} if at least one member carries {@code @key}. */
    boolean isKeyed();

    /** Extensibility (Final/Appendable/Mutable) per OMG XTypes 1.3 §7.2.2.4.4. */
    ExtensibilityKind getExtensibility();

    /** Encode with default endianness (LE). */
    byte[] encode(T sample);

    /** Encode with the chosen endianness. */
    byte[] encode(T sample, EndianMode endian);

    /** Decode from the full buffer. */
    T decode(byte[] bytes);

    /** Decode aus Subrange. */
    T decode(byte[] bytes, int offset, int length);

    /**
     * Key hash computation: 16 bytes MD5 over {@code PlainCdr2BeKeyHolder}
     * of the {@code @key} fields (XTypes §7.6.8). Returns all-zero if
     * {@link #isKeyed()} {@code false}.
     */
    byte[] keyHash(T sample);

    // ------------------------------------------------------------------
    // OMG-Marker-Bridge (DDS-Java-PSM 1.0 §6.2)
    // ------------------------------------------------------------------

    /**
     * OMG bridge: serialize via the encode path and writes into
     * {@code buf} (position grows accordingly).
     */
    @Override
    default void serialize(T value, ByteBuffer buf) {
        byte[] bytes = encode(value);
        buf.put(bytes);
    }

    /**
     * OMG bridge: deserialize via the decode path. Reads all
     * remaining bytes of the buffer; callers that need subranges call
     * {@link #decode(byte[], int, int)} directly.
     */
    @Override
    default T deserialize(ByteBuffer buf) {
        int len = buf.remaining();
        byte[] tmp = new byte[len];
        buf.get(tmp);
        return decode(tmp);
    }
}
