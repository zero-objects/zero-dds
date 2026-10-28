// SPDX-License-Identifier: Apache-2.0
package org.zerodds.internal;

import java.util.Arrays;

/**
 * The identity of a written sample: the GUID of the writer and the sequence number
 * the writer gave the sample (DDS-RPC 1.0 §7.5.1.1.1 {@code SampleIdentity}, and
 * the {@code writer_guid} and {@code sequence_number} of a DDSI-RTPS DATA).
 *
 * <p>On the wire as {@code PID_RELATED_SAMPLE_IDENTITY} (DDS-RPC 1.0 §7.8.2) it is
 * 24 octets: the 16 octets of the GUID, then the sequence number as a signed high
 * and an unsigned low 32-bit word, in the byte order of the parameter list.
 */
public final class SourceIdentity implements java.io.Serializable {

    private static final long serialVersionUID = 1L;

    /** The size of the identity on the wire. */
    public static final int WIRE_SIZE = 24;

    private final byte[] guid;
    private final long sequence;

    public SourceIdentity(byte[] guid, long sequence) {
        if (guid == null || guid.length != 16) {
            throw new IllegalArgumentException("a GUID is sixteen octets");
        }
        this.guid = guid.clone();
        this.sequence = sequence;
    }

    public byte[] guid() {
        return guid.clone();
    }

    /** The sequence number: the high word in the upper 32 bits, the low word below. */
    public long sequence() {
        return sequence;
    }

    /** The 24 octets of the parameter, little-endian. */
    public byte[] toBytesLe() {
        byte[] out = new byte[WIRE_SIZE];
        System.arraycopy(guid, 0, out, 0, 16);
        long high = sequence >> 32;
        long low = sequence & 0xffffffffL;
        for (int i = 0; i < 4; i++) {
            out[16 + i] = (byte) (high >>> (8 * i));
            out[20 + i] = (byte) (low >>> (8 * i));
        }
        return out;
    }

    /** The identity of a parameter value written little-endian, or {@code null} if it is short. */
    public static SourceIdentity fromBytesLe(byte[] value) {
        if (value == null || value.length < WIRE_SIZE) {
            return null;
        }
        long high = 0;
        long low = 0;
        for (int i = 0; i < 4; i++) {
            high |= (value[16 + i] & 0xffL) << (8 * i);
            low |= (value[20 + i] & 0xffL) << (8 * i);
        }
        return new SourceIdentity(Arrays.copyOf(value, 16), (((int) high) * 1L << 32) | low);
    }

    @Override
    public boolean equals(Object o) {
        return o instanceof SourceIdentity && ((SourceIdentity) o).sequence == sequence
                && Arrays.equals(((SourceIdentity) o).guid, guid);
    }

    @Override
    public int hashCode() {
        return 31 * Arrays.hashCode(guid) + Long.hashCode(sequence);
    }

    @Override
    public String toString() {
        return org.zerodds.cdr.HexBytes.encode(guid) + ":" + sequence;
    }
}
