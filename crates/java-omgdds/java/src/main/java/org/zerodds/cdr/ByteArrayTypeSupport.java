// SPDX-License-Identifier: Apache-2.0
package org.zerodds.cdr;

import java.nio.ByteBuffer;

/**
 * Built-in {@link org.omg.dds.topic.TopicTypeSupport} for raw {@code byte[]}
 * topics — DDS-Java-PSM 1.0 §8 (the bytes/keyed-bytes built-in type).
 *
 * <p>This is the zero-boilerplate path behind {@code createTopic("X",
 * byte[].class)}: the sample <em>is</em> its own wire form, so serialization is
 * an identity pass-through (no XCDR2 framing). It lets the front-page quickstart
 * write {@code "hello".getBytes()} and read it back as a {@code byte[]} without
 * any IDL or generated {@code TypeSupport}.
 */
public final class ByteArrayTypeSupport implements TopicTypeSupport<byte[]> {

    /** Shared singleton — the type is stateless. */
    public static final ByteArrayTypeSupport INSTANCE = new ByteArrayTypeSupport();

    private static final byte[] EMPTY = new byte[0];

    public ByteArrayTypeSupport() {}

    @Override
    public String getTypeName() {
        // DDS built-in raw-bytes type name (RTI/Connext "DDS::Bytes" idiom).
        return "DDS::Octets";
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
    public byte[] encode(byte[] sample) {
        return encode(sample, EndianMode.LITTLE_ENDIAN);
    }

    @Override
    public byte[] encode(byte[] sample, EndianMode endian) {
        if (sample == null) {
            return EMPTY;
        }
        return sample.clone();
    }

    @Override
    public byte[] decode(byte[] bytes) {
        return decode(bytes, 0, bytes.length);
    }

    @Override
    public byte[] decode(byte[] bytes, int offset, int length) {
        byte[] out = new byte[length];
        System.arraycopy(bytes, offset, out, 0, length);
        return out;
    }

    @Override
    public byte[] keyHash(byte[] sample) {
        return new byte[16]; // not keyed → all-zero per interface contract.
    }

    // ------------------------------------------------------------------
    // OMG marker bridge — identity pass-through (no XCDR2 framing).
    // ------------------------------------------------------------------

    @Override
    public void serialize(byte[] value, ByteBuffer buf) {
        if (value != null) {
            buf.put(value);
        }
    }

    @Override
    public byte[] deserialize(ByteBuffer buf) {
        byte[] out = new byte[buf.remaining()];
        buf.get(out);
        return out;
    }
}
