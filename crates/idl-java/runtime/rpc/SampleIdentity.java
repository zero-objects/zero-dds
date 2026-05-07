// DDS-RPC 1.0 §7.5.1.1.1 — SampleIdentity (writer GUID + sequence number).
package org.zerodds.rpc;

import java.util.Arrays;

/**
 * Mirrors the Rust {@code dds_rpc::common_types::SampleIdentity}.
 * Identifies a sample by its writer GUID (16 bytes) and sequence number.
 */
public final class SampleIdentity {
    /** Reserved "unknown" identity (all-zero). */
    public static final SampleIdentity UNKNOWN = new SampleIdentity(new byte[16], 0L);

    private final byte[] writerGuid;
    private final long sequenceNumber;

    public SampleIdentity(byte[] writerGuid, long sequenceNumber) {
        if (writerGuid == null || writerGuid.length != 16) {
            throw new IllegalArgumentException("writerGuid must be exactly 16 bytes");
        }
        this.writerGuid = writerGuid.clone();
        this.sequenceNumber = sequenceNumber;
    }

    public byte[] writerGuid() {
        return writerGuid.clone();
    }

    public long sequenceNumber() {
        return sequenceNumber;
    }

    @Override
    public boolean equals(Object o) {
        if (this == o) return true;
        if (!(o instanceof SampleIdentity)) return false;
        SampleIdentity that = (SampleIdentity) o;
        return sequenceNumber == that.sequenceNumber && Arrays.equals(writerGuid, that.writerGuid);
    }

    @Override
    public int hashCode() {
        int h = Long.hashCode(sequenceNumber);
        h = 31 * h + Arrays.hashCode(writerGuid);
        return h;
    }
}
