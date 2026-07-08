// SPDX-License-Identifier: Apache-2.0
package org.zerodds.internal;

import org.omg.dds.core.InstanceHandle;

import java.util.Arrays;

/**
 * A value-typed wrapper over instance key bytes (DDS-DCPS 1.4 §2.2.1.2.2),
 * usable as a {@code Map} key. Empty bytes denote the single instance of an
 * un-keyed Topic. Provides a stable {@link InstanceHandle} derivation so the
 * same key always maps to the same handle within a process.
 */
public final class InstanceKey {
    private final byte[] bytes;

    public InstanceKey(byte[] bytes) {
        this.bytes = bytes == null ? new byte[0] : bytes.clone();
    }

    public byte[] bytes() {
        return bytes.clone();
    }

    /**
     * Derive a stable 16-byte InstanceHandle from key bytes (a truncated /
     * zero-padded copy of the key — mirrors the runtime's key_hash to
     * instance_handle mapping). The empty (un-keyed) key maps to a fixed
     * non-NIL handle so a single-instance topic still has a real handle.
     */
    public static InstanceHandle toHandle(InstanceKey key) {
        byte[] out = new byte[16];
        byte[] k = key.bytes;
        if (k.length == 0) {
            out[15] = 1; // distinct, non-NIL handle for the single instance
        } else if (k.length <= 16) {
            System.arraycopy(k, 0, out, 0, k.length);
        } else {
            // MD5-free fold: XOR-stripe longer keys into 16 bytes.
            for (int i = 0; i < k.length; i++) {
                out[i % 16] ^= k[i];
            }
        }
        return InstanceHandle.from(out);
    }

    @Override
    public boolean equals(Object o) {
        if (this == o) return true;
        if (!(o instanceof InstanceKey)) return false;
        return Arrays.equals(bytes, ((InstanceKey) o).bytes);
    }

    @Override
    public int hashCode() {
        return Arrays.hashCode(bytes);
    }
}
