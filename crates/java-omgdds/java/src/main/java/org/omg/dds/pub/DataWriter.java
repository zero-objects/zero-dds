// SPDX-License-Identifier: Apache-2.0
package org.omg.dds.pub;

import org.omg.dds.core.Entity;
import org.omg.dds.core.InstanceHandle;
import org.omg.dds.core.ReturnCode;
import org.omg.dds.core.policy.QosProfile;
import org.omg.dds.topic.Topic;
import org.omg.dds.topic.TopicTypeSupport;
import org.zerodds.internal.InProcessBus;
import org.zerodds.internal.Xcdr2Codec;

import java.nio.ByteBuffer;
import java.util.UUID;

/**
 * OMG DDS Java-PSM DataWriter — Spec §7.2.4.
 */
public final class DataWriter<T> implements Entity {
    private final int domainId;
    private final Topic<T> topic;
    private final TopicTypeSupport<T> typeSupport;
    private final QosProfile qos;
    private final InstanceHandle handle;
    private boolean closed = false;
    private boolean enabled = false;

    public DataWriter(int domainId, Topic<T> topic, TopicTypeSupport<T> typeSupport, QosProfile qos) {
        this.domainId = domainId;
        this.topic = topic;
        this.typeSupport = typeSupport;
        this.qos = qos;
        this.handle = uuidHandle();
    }

    public ReturnCode write(T data) {
        if (closed) return ReturnCode.ALREADY_DELETED;
        if (!enabled) return ReturnCode.NOT_ENABLED;
        // Grow the encode buffer on overflow so large samples (e.g. raw byte[]
        // payloads bigger than the initial capacity) serialize without loss.
        int capacity = 256;
        while (true) {
            ByteBuffer encoder = Xcdr2Codec.encoder(capacity);
            try {
                typeSupport.serialize(data, encoder);
            } catch (java.nio.BufferOverflowException overflow) {
                if (capacity >= (1 << 28)) {
                    throw overflow; // refuse to grow past 256 MiB.
                }
                capacity <<= 1;
                continue;
            }
            InProcessBus.instance().publish(
                    domainId, topic.getName(), Xcdr2Codec.copyToBytes(encoder));
            return ReturnCode.OK;
        }
    }

    public Topic<T> getTopic() {
        return topic;
    }

    public QosProfile getQos() {
        return qos;
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

    private static InstanceHandle uuidHandle() {
        UUID id = UUID.randomUUID();
        byte[] out = new byte[16];
        long msb = id.getMostSignificantBits();
        long lsb = id.getLeastSignificantBits();
        for (int i = 0; i < 8; i++) out[i] = (byte) (msb >>> (56 - 8 * i));
        for (int i = 0; i < 8; i++) out[8 + i] = (byte) (lsb >>> (56 - 8 * i));
        return InstanceHandle.from(out);
    }
}
