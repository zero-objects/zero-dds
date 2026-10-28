// SPDX-License-Identifier: Apache-2.0
package org.zerodds.cdr;

import java.nio.ByteBuffer;

/**
 * Built-in {@link org.zerodds.cdr.TopicTypeSupport} for raw {@code byte[]}
 * topics — DDS-Java-PSM 1.0 §7.8.2 (the {@code DDS::Bytes} built-in type).
 *
 * <p>XTypes 1.3 Annex E defines {@code DDS::Bytes} as
 * {@code @extensibility(APPENDABLE) struct Bytes { ByteSeq value; }}. On the
 * wire that is a DELIMITED_CDR2 struct: a 4-byte DHEADER carrying the
 * serialized size of the body (XTypes 1.3 §7.4.3.4.2, rule 30), followed by
 * the {@code ByteSeq} member — a 4-byte length (rule 11) plus the octets.
 * The sample <em>is</em> its own payload, so serialization wraps it in that
 * framing rather than passing it through raw.
 */
public final class ByteArrayTypeSupport implements TopicTypeSupport<byte[]> {

    /** Shared singleton — the type is stateless. */
    public static final ByteArrayTypeSupport INSTANCE = new ByteArrayTypeSupport();

    private static final byte[] EMPTY = new byte[0];

    /** Size of the DHEADER and the sequence length word, in bytes. */
    private static final int FRAMING = 8;

    public ByteArrayTypeSupport() {}

    @Override
    public String getTypeName() {
        // XTypes 1.3 §7.6.5.3 — the built-in bytes type is "DDS::Bytes".
        return "DDS::Bytes";
    }

    @Override
    public boolean isKeyed() {
        return false;
    }

    @Override
    public ExtensibilityKind getExtensibility() {
        // Annex E: @extensibility(APPENDABLE) struct Bytes { ByteSeq value; }.
        return ExtensibilityKind.APPENDABLE;
    }

    @Override
    public byte[] encode(byte[] sample) {
        return encode(sample, EndianMode.LITTLE_ENDIAN);
    }

    @Override
    public byte[] encode(byte[] sample, EndianMode endian) {
        byte[] value = sample == null ? EMPTY : sample;
        int len = value.length;
        byte[] out = new byte[FRAMING + len];
        boolean le = endian == EndianMode.LITTLE_ENDIAN;
        putInt(out, 0, len + 4, le); // DHEADER = serialized body size (length word + octets)
        putInt(out, 4, len, le);     // ByteSeq length (number of octets)
        System.arraycopy(value, 0, out, FRAMING, len);
        return out;
    }

    @Override
    public byte[] decode(byte[] bytes) {
        return decode(bytes, 0, bytes.length);
    }

    @Override
    public byte[] decode(byte[] bytes, int offset, int length) {
        if (bytes == null) {
            throw new XcdrException("No data for DDS::Bytes");
        }
        if (offset < 0 || length < FRAMING || offset + length > bytes.length) {
            throw new XcdrException("Truncated DDS::Bytes: " + length
                    + " byte(s) where " + FRAMING + " framing bytes are the minimum");
        }
        int body = getInt(bytes, offset, true); // DHEADER = serialized body size
        int len = getInt(bytes, offset + 4, true);
        if (body != len + 4 || FRAMING + len > length) {
            throw new XcdrException("Inconsistent DDS::Bytes framing: DHEADER says "
                    + body + ", sequence holds " + len + " octet(s)");
        }
        byte[] out = new byte[len];
        System.arraycopy(bytes, offset + FRAMING, out, 0, len);
        return out;
    }

    @Override
    public byte[] keyHash(byte[] sample) {
        return new byte[16]; // not keyed → all-zero per interface contract.
    }

    // ------------------------------------------------------------------
    // OMG marker bridge — DDS::Bytes framing (DELIMITED_CDR2 + ByteSeq).
    // ------------------------------------------------------------------

    @Override
    public void serialize(byte[] value, ByteBuffer buf) {
        byte[] framed = encode(value, EndianMode.LITTLE_ENDIAN);
        buf.put(framed);
    }

    @Override
    public byte[] deserialize(ByteBuffer buf) {
        byte[] all = new byte[buf.remaining()];
        buf.get(all);
        return decode(all, 0, all.length);
    }

    private static void putInt(byte[] out, int off, int v, boolean littleEndian) {
        if (littleEndian) {
            out[off] = (byte) v;
            out[off + 1] = (byte) (v >>> 8);
            out[off + 2] = (byte) (v >>> 16);
            out[off + 3] = (byte) (v >>> 24);
        } else {
            out[off] = (byte) (v >>> 24);
            out[off + 1] = (byte) (v >>> 16);
            out[off + 2] = (byte) (v >>> 8);
            out[off + 3] = (byte) v;
        }
    }

    private static int getInt(byte[] bytes, int off, boolean littleEndian) {
        if (littleEndian) {
            return (bytes[off] & 0xFF)
                    | ((bytes[off + 1] & 0xFF) << 8)
                    | ((bytes[off + 2] & 0xFF) << 16)
                    | ((bytes[off + 3] & 0xFF) << 24);
        }
        return ((bytes[off] & 0xFF) << 24)
                | ((bytes[off + 1] & 0xFF) << 16)
                | ((bytes[off + 2] & 0xFF) << 8)
                | (bytes[off + 3] & 0xFF);
    }
}
