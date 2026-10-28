// SPDX-License-Identifier: Apache-2.0
package org.omg.dds.rpc;

/**
 * The codec of one topic type — the Java counterpart of the two
 * functions §7.11.1.4.22's {@code dds_type_traits<T>} carries.
 *
 * <p>Supplied per request and reply type. A binding with no codec for a
 * type is a compile error naming the type rather than a sample that
 * never serialises.
 *
 * @param <T> the topic type
 */
public interface TypeSupport<T> {

    /** The payload octets of one value, without the header. */
    byte[] serialize(T value);

    /** The value the payload octets carry, reading on from {@code cursor}. */
    T deserialize(byte[] octets, HeaderCodec.Cursor cursor);

    /**
     * {@link #serialize(Object)} for a payload that starts {@code offset}
     * octets into its sample and is encoded as {@code encoding}: alignment
     * is counted from the start of the sample, so a member of eight octets
     * pads differently after a header of 39 octets than after one of 40.
     * A codec that does not align by the encoding (one of 32-bit members)
     * keeps this default.
     */
    default byte[] serialize(T value, int offset, WireEncoding encoding) {
        return serialize(value);
    }

    /** {@link #deserialize(byte[], HeaderCodec.Cursor)} for {@code encoding}. */
    default T deserialize(byte[] octets, HeaderCodec.Cursor cursor, WireEncoding encoding) {
        return deserialize(octets, cursor);
    }
}
