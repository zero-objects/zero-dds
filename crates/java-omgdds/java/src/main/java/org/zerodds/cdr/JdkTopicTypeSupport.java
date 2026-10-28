// SPDX-License-Identifier: Apache-2.0
package org.zerodds.cdr;

import java.util.Arrays;

import org.zerodds.wire.Endianness;
import org.zerodds.wire.PlCdr1Framing;
import org.zerodds.wire.Reader;
import org.zerodds.wire.Writer;

/**
 * The type support of a topic whose data type is itself a {@code Serializable} class of the JDK
 * (DDS-Java-PSM 1.0 §8.1): a {@code java.util.Date}, a {@code java.time.Instant}, … The sample is
 * the value in the form {@link JdkValueTypes} or {@link JdkSerializedValues} gives it, as a FINAL
 * type, without a header.
 */
final class JdkTopicTypeSupport<T> implements TopicTypeSupport<T> {

    private final Class<T> type;

    JdkTopicTypeSupport(Class<T> type) {
        this.type = type;
    }

    /** Whether {@code c} is a JDK class this support carries. */
    static boolean supports(Class<?> c) {
        boolean jdk = c.getClassLoader() == null || c.getName().startsWith("java.")
                || c.getName().startsWith("javax.");
        return JdkValueTypes.supports(c) || (jdk && JdkSerializedValues.supports(c));
    }

    @Override
    public String getTypeName() {
        return type.getName().replace(".", "::");
    }

    @Override
    public boolean isKeyed() {
        return false;
    }

    @Override
    public ExtensibilityKind getExtensibility() {
        return ExtensibilityKind.FINAL;
    }

    @Override
    public byte[] encode(T sample) {
        return encode(sample, EndianMode.LITTLE_ENDIAN);
    }

    @Override
    public byte[] encode(T sample, EndianMode endian) {
        if (sample == null) {
            throw new XcdrException("a " + type.getName() + " sample must not be null");
        }
        Writer w = new Writer(endian == EndianMode.BIG_ENDIAN ? Endianness.BIG : Endianness.LITTLE, 4,
                PlCdr1Framing.defaults(), false);
        if (JdkValueTypes.supports(type)) {
            JdkValueTypes.write(w, type, sample);
        } else {
            JdkSerializedValues.write(w, type, sample);
        }
        return w.finish();
    }

    @Override
    public T decode(byte[] bytes) {
        return decode(bytes, 0, bytes.length);
    }

    @Override
    public T decode(byte[] bytes, int offset, int length) {
        if (bytes == null || offset < 0 || length < 0 || offset + length > bytes.length) {
            throw new XcdrException("no data in the range given");
        }
        Reader r = new Reader(Arrays.copyOfRange(bytes, offset, offset + length), Endianness.LITTLE, 4,
                PlCdr1Framing.defaults());
        try {
            Object value = JdkValueTypes.supports(type) ? JdkValueTypes.read(r, type)
                    : JdkSerializedValues.read(r, type);
            return type.cast(value);
        } catch (org.zerodds.wire.WireException malformed) {
            throw new XcdrException("malformed " + type.getName() + ": " + malformed.getMessage(), malformed);
        }
    }
}
