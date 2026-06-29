// SPDX-License-Identifier: Apache-2.0
package org.omg.dds.sub;

import org.omg.dds.core.Entity;
import org.omg.dds.core.InstanceHandle;
import org.omg.dds.core.ReturnCode;
import org.omg.dds.core.policy.QosProfile;
import org.omg.dds.topic.ContentFilteredTopic;
import org.omg.dds.topic.Topic;
import org.omg.dds.topic.TopicTypeSupport;
import org.zerodds.cdr.TypeSupportResolver;

import java.util.UUID;

/** OMG DDS Java-PSM Subscriber — Spec §7.2.4. */
public final class Subscriber implements Entity {
    private final int domainId;
    private final QosProfile defaultDataReaderQos;
    private final InstanceHandle handle;
    private boolean closed = false;
    private boolean enabled = false;

    public Subscriber(int domainId, QosProfile defaultDataReaderQos) {
        this.domainId = domainId;
        this.defaultDataReaderQos = defaultDataReaderQos;
        this.handle = uuidHandle();
    }

    /**
     * Convenience overload — derives the {@link TopicTypeSupport} from the
     * topic's data class (built-in {@code byte[]}, {@code idl-java}-generated
     * {@code <Name>TypeSupport.INSTANCE}, or reflective XCDR2 fallback).
     */
    public <T> DataReader<T> createDataReader(Topic<T> topic) {
        return createDataReader(topic, TypeSupportResolver.resolve(topic.getType()));
    }

    public <T> DataReader<T> createDataReader(Topic<T> topic, TopicTypeSupport<T> typeSupport) {
        return createDataReader(topic, typeSupport, defaultDataReaderQos);
    }

    public <T> DataReader<T> createDataReader(
            Topic<T> topic, TopicTypeSupport<T> typeSupport, QosProfile qos) {
        DataReader<T> dr = new DataReader<>(domainId, topic, typeSupport, qos);
        dr.enable();
        return dr;
    }

    /**
     * Create a DataReader bound to a {@link ContentFilteredTopic} — DDS-DCPS
     * 1.4 §2.2.2.3.3. Only samples whose content satisfies the filter
     * expression are delivered to the reader.
     */
    public <T> DataReader<T> createDataReader(
            ContentFilteredTopic<T> cft, TopicTypeSupport<T> typeSupport, QosProfile qos) {
        DataReader<T> dr = new DataReader<>(
                domainId, cft.getRelatedTopic(), typeSupport, qos, cft::evaluate);
        dr.enable();
        return dr;
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
