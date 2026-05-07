// SPDX-License-Identifier: Apache-2.0
package org.zerodds.cdr;

import java.nio.ByteBuffer;

/**
 * ZeroDDS-erweitertes TypeSupport-Interface fuer XCDR2-Bindings.
 *
 * <p>Spec: zerodds-xcdr2-java-1.0 §2. Erweitert das DDS-Java-PSM
 * Marker-Interface {@link org.omg.dds.topic.TopicTypeSupport} um
 * konkrete encode/decode/keyHash-Methoden plus
 * Extensibility-Reflection.
 *
 * <p>Generierter Code (idl-java) emittiert pro IDL-{@code struct} eine
 * Singleton-Klasse {@code <Name>TypeSupport} die dieses Interface
 * implementiert.
 *
 * @param <T> Sample-Klasse (POJO mit Bean-Accessoren).
 */
public interface TopicTypeSupport<T> extends org.omg.dds.topic.TopicTypeSupport<T> {

    /** DDS-Type-Name (Convention {@code Module::Sub::Struct}, ASCII, max 256 Bytes). */
    @Override
    String getTypeName();

    /** {@code true} falls mindestens ein Member {@code @key} traegt. */
    boolean isKeyed();

    /** Extensibility (Final/Appendable/Mutable) per OMG XTypes 1.3 §7.2.2.4.4. */
    ExtensibilityKind getExtensibility();

    /** Encode mit Default-Endianness (LE). */
    byte[] encode(T sample);

    /** Encode mit gewaehlter Endianness. */
    byte[] encode(T sample, EndianMode endian);

    /** Decode aus voller Buffer. */
    T decode(byte[] bytes);

    /** Decode aus Subrange. */
    T decode(byte[] bytes, int offset, int length);

    /**
     * Key-Hash-Berechnung: 16 Bytes MD5 ueber {@code PlainCdr2BeKeyHolder}
     * der {@code @key}-Felder (XTypes §7.6.8). Liefert all-zero falls
     * {@link #isKeyed()} {@code false}.
     */
    byte[] keyHash(T sample);

    // ------------------------------------------------------------------
    // OMG-Marker-Bridge (DDS-Java-PSM 1.0 §6.2)
    // ------------------------------------------------------------------

    /**
     * OMG-Bridge: serialize ueber den encode-Pfad und schreibt in
     * {@code buf} (Position waechst entsprechend).
     */
    @Override
    default void serialize(T value, ByteBuffer buf) {
        byte[] bytes = encode(value);
        buf.put(bytes);
    }

    /**
     * OMG-Bridge: deserialize ueber den decode-Pfad. Liest alle
     * verbleibenden Bytes des Buffers; wer Subranges braucht, ruft
     * {@link #decode(byte[], int, int)} direkt auf.
     */
    @Override
    default T deserialize(ByteBuffer buf) {
        int len = buf.remaining();
        byte[] tmp = new byte[len];
        buf.get(tmp);
        return decode(tmp);
    }
}
