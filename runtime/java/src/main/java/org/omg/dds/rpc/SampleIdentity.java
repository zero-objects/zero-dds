// SPDX-License-Identifier: Apache-2.0
package org.omg.dds.rpc;

import java.util.Arrays;

/**
 * A GUID and a sequence number — DDS-RPC 1.0 §7.5.1.1.1: "SampleIdentity =
 * struct {GUID_t writer_guid; SequenceNumber_t sequence_number;}".
 *
 * <p>Immutable, because §7.2.1 makes it the value a reply is
 * correlated by: an identity a caller could edit after sending is an
 * identity the reply no longer matches.
 */
public final class SampleIdentity implements org.omg.dds.core.SampleIdentity {

    /**
     * §7.8.2 — {@code RELATED_SAMPLE_IDENTITY_INVALID}:
     * {@code { GUID_UNKNOWN, SEQUENCENUMBER_UNKNOWN }}, both defined in
     * [RTPS]. The sequence number is {@code { high = -1, low = 0 }}, not 0.
     */
    public static final SampleIdentity RELATED_SAMPLE_IDENTITY_INVALID =
            new SampleIdentity(new byte[16], 0xFFFFFFFF00000000L);

    /**
     * The unset value — an all-zero GUID and a zero sequence number, what a
     * header carries before its writer stamps it. Distinct from
     * {@link #RELATED_SAMPLE_IDENTITY_INVALID}.
     */
    public static final SampleIdentity UNKNOWN = new SampleIdentity(new byte[16], 0L);

    private final byte[] writerGuid;
    private final long sequenceNumber;

    public SampleIdentity(byte[] writerGuid, long sequenceNumber) {
        if (writerGuid == null || writerGuid.length != 16) {
            throw new IllegalArgumentException("a GUID is sixteen octets");
        }
        this.writerGuid = Arrays.copyOf(writerGuid, 16);
        this.sequenceNumber = sequenceNumber;
    }

    /**
     * The identity {@code identity} names, as this class: itself when it is one, else the GUID
     * of its {@link java.util.UUID} (the most significant half first, as {@link #guid()} gives
     * it) and its sequence number. The operations of the machine-readable Java files take an
     * {@code org.omg.dds.core.SampleIdentity}; this is how they read it.
     *
     * @return the identity, or {@code null} for {@code null}
     */
    public static SampleIdentity of(org.omg.dds.core.SampleIdentity identity) {
        if (identity == null || identity instanceof SampleIdentity) {
            return (SampleIdentity) identity;
        }
        java.util.UUID guid = identity.guid();
        java.nio.ByteBuffer octets = java.nio.ByteBuffer.allocate(16);
        octets.putLong(guid.getMostSignificantBits()).putLong(guid.getLeastSignificantBits());
        return new SampleIdentity(octets.array(), identity.sequenceNumber());
    }

    /** The sixteen octets RTPS gives the writing entity. */
    public byte[] writerGuid() {
        return Arrays.copyOf(writerGuid, 16);
    }

    /**
     * The GUID as the {@link java.util.UUID} of the machine-readable
     * {@code org.omg.dds.core.SampleIdentity}: the first eight octets are the
     * most significant half, the last eight the least significant one.
     */
    @Override
    public java.util.UUID guid() {
        java.nio.ByteBuffer octets = java.nio.ByteBuffer.wrap(writerGuid);
        return new java.util.UUID(octets.getLong(), octets.getLong());
    }

    /** The writer's own sequence number for this sample. */
    @Override
    public long sequenceNumber() {
        return sequenceNumber;
    }

    /** Whether this identity names nothing. */
    public boolean isUnknown() {
        return equals(UNKNOWN);
    }

    @Override
    public boolean equals(Object other) {
        if (!(other instanceof SampleIdentity)) {
            return false;
        }
        SampleIdentity that = (SampleIdentity) other;
        return sequenceNumber == that.sequenceNumber && Arrays.equals(writerGuid, that.writerGuid);
    }

    @Override
    public int hashCode() {
        return 31 * Arrays.hashCode(writerGuid) + Long.hashCode(sequenceNumber);
    }

    @Override
    public String toString() {
        StringBuilder out = new StringBuilder("SampleIdentity(");
        for (byte octet : writerGuid) {
            out.append(String.format("%02x", octet));
        }
        return out.append(", ").append(sequenceNumber).append(')').toString();
    }
}
