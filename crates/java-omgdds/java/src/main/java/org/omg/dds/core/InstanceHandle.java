// SPDX-License-Identifier: Apache-2.0
package org.omg.dds.core;

import java.util.Arrays;
import java.util.Objects;

/**
 * OMG DDS Java-PSM InstanceHandle_t — Spec §7.2.5.
 *
 * <p>Opaque 16-byte instance handle.
 */
public final class InstanceHandle {
    public static final InstanceHandle NIL = new InstanceHandle(new byte[16]);

    private final byte[] bytes;

    private InstanceHandle(byte[] bytes) {
        if (bytes.length != 16) {
            throw new IllegalArgumentException("InstanceHandle must be 16 bytes, got " + bytes.length);
        }
        this.bytes = bytes.clone();
    }

    public static InstanceHandle from(byte[] bytes) {
        return new InstanceHandle(bytes);
    }

    public byte[] bytes() {
        return bytes.clone();
    }

    public boolean isNil() {
        for (byte b : bytes) {
            if (b != 0) return false;
        }
        return true;
    }

    @Override
    public boolean equals(Object o) {
        if (this == o) return true;
        if (!(o instanceof InstanceHandle other)) return false;
        return Arrays.equals(bytes, other.bytes);
    }

    @Override
    public int hashCode() {
        return Objects.hash((Object[]) toBoxed(bytes));
    }

    private static Byte[] toBoxed(byte[] arr) {
        Byte[] out = new Byte[arr.length];
        for (int i = 0; i < arr.length; i++) out[i] = arr[i];
        return out;
    }

    @Override
    public String toString() {
        StringBuilder sb = new StringBuilder("InstanceHandle(");
        for (byte b : bytes) {
            sb.append(String.format("%02X", b));
        }
        return sb.append(')').toString();
    }
}
