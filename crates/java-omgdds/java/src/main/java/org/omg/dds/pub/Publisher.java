// SPDX-License-Identifier: Apache-2.0
package org.omg.dds.pub;

import org.omg.dds.core.Entity;
import org.omg.dds.core.InstanceHandle;
import org.omg.dds.core.ReturnCode;
import org.omg.dds.core.policy.QosProfile;
import org.omg.dds.topic.Topic;
import org.omg.dds.topic.TopicTypeSupport;
import org.zerodds.cdr.TypeSupportResolver;

import java.util.UUID;

/** OMG DDS Java-PSM Publisher — Spec §7.2.4. */
public final class Publisher implements Entity {
    private final int domainId;
    private final QosProfile defaultDataWriterQos;
    private final InstanceHandle handle;
    private boolean closed = false;
    private boolean enabled = false;

    public Publisher(int domainId, QosProfile defaultDataWriterQos) {
        this.domainId = domainId;
        this.defaultDataWriterQos = defaultDataWriterQos;
        this.handle = uuidHandle();
    }

    /**
     * Convenience overload — derives the {@link TopicTypeSupport} from the
     * topic's data class (built-in {@code byte[]}, {@code idl-java}-generated
     * {@code <Name>TypeSupport.INSTANCE}, or reflective XCDR2 fallback). Lets
     * the front-page quickstart create a writer from just the topic.
     */
    public <T> DataWriter<T> createDataWriter(Topic<T> topic) {
        return createDataWriter(topic, TypeSupportResolver.resolve(topic.getType()));
    }

    public <T> DataWriter<T> createDataWriter(Topic<T> topic, TopicTypeSupport<T> typeSupport) {
        return createDataWriter(topic, typeSupport, defaultDataWriterQos);
    }

    public <T> DataWriter<T> createDataWriter(
            Topic<T> topic, TopicTypeSupport<T> typeSupport, QosProfile qos) {
        DataWriter<T> dw = new DataWriter<>(domainId, topic, typeSupport, qos);
        dw.enable();
        return dw;
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
