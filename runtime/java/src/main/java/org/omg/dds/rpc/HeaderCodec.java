// SPDX-License-Identifier: Apache-2.0
package org.omg.dds.rpc;

import java.io.ByteArrayOutputStream;
import java.nio.charset.StandardCharsets;

/**
 * The octets the two headers of DDS-RPC 1.0 §7.5.1.1.1 take.
 *
 * <p>Little-endian: the sixteen GUID octets, then the sequence number as
 * one 64-bit value, then — for a request — the instance name as its
 * length including the terminator followed by the octets and the
 * terminator, or — for a reply — the exception code as a 32-bit value.
 *
 * <p>That is the layout {@code crates/rpc/src/wire_codec.rs} writes and
 * {@code runtime/cpp/include/dds/rpc.hpp} writes.
 * {@code crates/rpc/tests/java_rpc_runtime.rs} holds this column against
 * the Rust one octet for octet; a second reading of the clause here
 * would be a third wire.
 */
public final class HeaderCodec {

    private HeaderCodec() {}

    /** How far into a buffer a read got, beside what it read. */
    public static final class Cursor {
        /** The next octet to read. */
        public int at;

        public Cursor(int at) {
            this.at = at;
        }
    }

    public static byte[] encodeRequestHeader(RequestHeader header) {
        ByteArrayOutputStream out = new ByteArrayOutputStream();
        putIdentity(out, header.requestId());
        putString(out, header.instanceName());
        return out.toByteArray();
    }

    public static byte[] encodeReplyHeader(ReplyHeader header) {
        ByteArrayOutputStream out = new ByteArrayOutputStream();
        putIdentity(out, header.relatedRequestId());
        putU32(out, header.remoteEx().wireValue());
        return out.toByteArray();
    }

    public static RequestHeader decodeRequestHeader(byte[] in, Cursor cursor) {
        SampleIdentity identity = getIdentity(in, cursor);
        String instance = getString(in, cursor);
        return new RequestHeader(identity, instance);
    }

    public static ReplyHeader decodeReplyHeader(byte[] in, Cursor cursor) {
        SampleIdentity identity = getIdentity(in, cursor);
        return new ReplyHeader(identity, RemoteExceptionCode.fromWire(getU32(in, cursor)));
    }

    /**
     * Pads {@code out} with zero octets to the next multiple of four.
     *
     * <p>The Basic mapping's topic type is {@code struct { RequestHeader
     * header; Call data; }} and {@code data} starts with a {@code long}
     * discriminator (§7.5.1.1.6 rule 5). XCDR aligns that to four octets
     * counted from the start of the sample, and the instance name makes
     * the header's length vary, so the header is followed by 0 to 3
     * padding octets.
     *
     * @param out the sample being written, starting at its first octet
     */
    public static void padTo4(ByteArrayOutputStream out) {
        while (out.size() % 4 != 0) {
            out.write(0);
        }
    }

    /** Moves {@code cursor} to the next multiple of four, as {@link #padTo4} wrote it. */
    public static void alignTo4(Cursor cursor) {
        cursor.at = (cursor.at + 3) & ~3;
    }

    public static void putU32(ByteArrayOutputStream out, int value) {
        out.write(value & 0xFF);
        out.write((value >>> 8) & 0xFF);
        out.write((value >>> 16) & 0xFF);
        out.write((value >>> 24) & 0xFF);
    }

    public static int getU32(byte[] in, Cursor cursor) {
        if (cursor.at + 4 > in.length) {
            throw new RemoteInvalidArgumentError("the sample ends inside a 32-bit value");
        }
        int value = (in[cursor.at] & 0xFF)
                | ((in[cursor.at + 1] & 0xFF) << 8)
                | ((in[cursor.at + 2] & 0xFF) << 16)
                | ((in[cursor.at + 3] & 0xFF) << 24);
        cursor.at += 4;
        return value;
    }

    private static void putIdentity(ByteArrayOutputStream out, SampleIdentity identity) {
        byte[] guid = identity.writerGuid();
        out.write(guid, 0, guid.length);
        long sequence = identity.sequenceNumber();
        // SequenceNumber_t is { long high; unsigned long low }: high first.
        putU32(out, (int) (sequence >>> 32));
        putU32(out, (int) (sequence & 0xFFFFFFFFL));
    }

    private static SampleIdentity getIdentity(byte[] in, Cursor cursor) {
        if (cursor.at + 16 > in.length) {
            throw new RemoteInvalidArgumentError("the sample ends inside a GUID");
        }
        byte[] guid = new byte[16];
        System.arraycopy(in, cursor.at, guid, 0, 16);
        cursor.at += 16;
        long high = getU32(in, cursor) & 0xFFFFFFFFL;
        long low = getU32(in, cursor) & 0xFFFFFFFFL;
        return new SampleIdentity(guid, (high << 32) | low);
    }

    /**
     * §7.5.1.1.1: {@code typedef string<255> InstanceName} — at most 255
     * octets before the terminator, so a length field of at most 256.
     */
    public static final int MAX_INSTANCE_NAME_OCTETS = 255;

    private static void putString(ByteArrayOutputStream out, String text) {
        byte[] octets = text.getBytes(StandardCharsets.UTF_8);
        if (octets.length > MAX_INSTANCE_NAME_OCTETS) {
            throw new IllegalArgumentException(
                    "an instance name is a string<" + MAX_INSTANCE_NAME_OCTETS
                            + "> (§7.5.1.1.1); this one is " + octets.length + " octets");
        }
        putU32(out, octets.length + 1);
        out.write(octets, 0, octets.length);
        out.write(0);
    }

    private static String getString(byte[] in, Cursor cursor) {
        int length = getU32(in, cursor);
        // The length includes the terminator, so a string<255> has a length
        // of at most 256; the field is unsigned.
        if (Integer.compareUnsigned(length, MAX_INSTANCE_NAME_OCTETS + 1) > 0) {
            throw new RemoteInvalidArgumentError(
                    "an instance name is a string<" + MAX_INSTANCE_NAME_OCTETS
                            + ">; the sample says " + (length & 0xFFFFFFFFL) + " octets");
        }
        if (length <= 0 || cursor.at + length > in.length) {
            throw new RemoteInvalidArgumentError("the sample ends inside a string");
        }
        if (in[cursor.at + length - 1] != 0) {
            throw new RemoteInvalidArgumentError("a string in the sample has no terminator");
        }
        String text = new String(in, cursor.at, length - 1, StandardCharsets.UTF_8);
        cursor.at += length;
        return text;
    }
}
