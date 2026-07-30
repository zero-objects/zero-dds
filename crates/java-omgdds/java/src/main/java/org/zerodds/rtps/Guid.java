// SPDX-License-Identifier: Apache-2.0
package org.zerodds.rtps;

import java.util.Arrays;

/**
 * RTPS GUID (16 bytes = 12-byte GuidPrefix + 4-byte EntityId) and the builtin
 * EntityId constants. Ported from {@code crates/rtps/src/wire_types.rs}.
 */
public final class Guid {
    // EntityKind bytes (§8.3.5.2, Table 9.1)
    public static final int KIND_USER_WRITER_NO_KEY = 0x03;
    public static final int KIND_USER_WRITER_WITH_KEY = 0x02;
    public static final int KIND_USER_READER_NO_KEY = 0x04;
    public static final int KIND_USER_READER_WITH_KEY = 0x07;
    public static final int KIND_BUILTIN_WRITER_WITH_KEY = 0xC2;
    public static final int KIND_BUILTIN_READER_WITH_KEY = 0xC7;
    public static final int KIND_PARTICIPANT = 0xC1;

    // Builtin EntityIds (§9.3.1.5, Table 9.4) — 4 bytes each.
    public static final byte[] ENTITYID_UNKNOWN = {0, 0, 0, 0};
    public static final byte[] ENTITYID_PARTICIPANT = {0, 0, 1, (byte) 0xC1};
    public static final byte[] SPDP_WRITER = {0, 0x01, 0x00, (byte) 0xC2};
    public static final byte[] SPDP_READER = {0, 0x01, 0x00, (byte) 0xC7};
    public static final byte[] SEDP_PUB_WRITER = {0, 0x00, 0x03, (byte) 0xC2};
    public static final byte[] SEDP_PUB_READER = {0, 0x00, 0x03, (byte) 0xC7};
    public static final byte[] SEDP_SUB_WRITER = {0, 0x00, 0x04, (byte) 0xC2};
    public static final byte[] SEDP_SUB_READER = {0, 0x00, 0x04, (byte) 0xC7};

    public final byte[] value; // 16 bytes

    public Guid(byte[] value16) {
        if (value16.length != 16) {
            throw new IllegalArgumentException("GUID must be 16 bytes");
        }
        this.value = value16.clone();
    }

    public Guid(byte[] prefix12, byte[] entityId4) {
        this.value = new byte[16];
        System.arraycopy(prefix12, 0, value, 0, 12);
        System.arraycopy(entityId4, 0, value, 12, 4);
    }

    public byte[] prefix() {
        return Arrays.copyOfRange(value, 0, 12);
    }

    public byte[] entityId() {
        return Arrays.copyOfRange(value, 12, 16);
    }

    public byte[] bytes() {
        return value.clone();
    }

    /** Build a user endpoint EntityId with a 3-byte key + a kind byte. */
    public static byte[] userEntityId(int key0, int key1, int key2, int kind) {
        return new byte[] {(byte) key0, (byte) key1, (byte) key2, (byte) kind};
    }

    @Override
    public boolean equals(Object o) {
        return o instanceof Guid && Arrays.equals(value, ((Guid) o).value);
    }

    @Override
    public int hashCode() {
        return Arrays.hashCode(value);
    }

    @Override
    public String toString() {
        StringBuilder sb = new StringBuilder(32);
        for (byte b : value) {
            sb.append(String.format("%02x", b));
        }
        return sb.toString();
    }
}
