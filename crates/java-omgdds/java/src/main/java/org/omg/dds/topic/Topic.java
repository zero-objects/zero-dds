// SPDX-License-Identifier: Apache-2.0
package org.omg.dds.topic;

import org.omg.dds.core.Entity;
import org.omg.dds.core.InstanceHandle;
import org.omg.dds.core.ReturnCode;

import java.util.UUID;

/**
 * OMG DDS Java-PSM Topic — Spec §7.2.4.
 *
 * <p>Generic over the topic data type {@code T}.
 */
public final class Topic<T> implements Entity {
    private final String name;
    private final Class<T> type;
    private final InstanceHandle handle;
    private boolean closed = false;
    private boolean enabled = false;

    public Topic(String name, Class<T> type) {
        this.name = name;
        this.type = type;
        byte[] uuid = uuidBytes();
        this.handle = InstanceHandle.from(uuid);
    }

    public String getName() {
        return name;
    }

    public Class<T> getType() {
        return type;
    }

    @Override
    public InstanceHandle getInstanceHandle() {
        return handle;
    }

    @Override
    public ReturnCode enable() {
        if (closed) return ReturnCode.ALREADY_DELETED;
        enabled = true;
        return ReturnCode.OK;
    }

    @Override
    public void close() {
        closed = true;
    }

    @Override
    public boolean isClosed() {
        return closed;
    }

    public boolean isEnabled() {
        return enabled;
    }

    private static byte[] uuidBytes() {
        UUID id = UUID.randomUUID();
        byte[] out = new byte[16];
        long msb = id.getMostSignificantBits();
        long lsb = id.getLeastSignificantBits();
        for (int i = 0; i < 8; i++) out[i] = (byte) (msb >>> (56 - 8 * i));
        for (int i = 0; i < 8; i++) out[8 + i] = (byte) (lsb >>> (56 - 8 * i));
        return out;
    }
}
