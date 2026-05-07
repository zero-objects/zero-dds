// SPDX-License-Identifier: Apache-2.0
package org.omg.dds.sub;

import org.omg.dds.core.Entity;
import org.omg.dds.core.InstanceHandle;
import org.omg.dds.core.ReturnCode;
import org.omg.dds.core.Time;
import org.omg.dds.core.policy.QosProfile;
import org.omg.dds.topic.Topic;
import org.omg.dds.topic.TopicTypeSupport;
import org.zerodds.internal.InProcessBus;
import org.zerodds.internal.Xcdr2Codec;

import java.util.ArrayList;
import java.util.List;
import java.util.UUID;
import java.util.concurrent.ConcurrentLinkedQueue;
import java.util.function.Consumer;

/**
 * OMG DDS Java-PSM DataReader — Spec §7.2.4.
 */
public final class DataReader<T> implements Entity {
    private final int domainId;
    private final Topic<T> topic;
    private final TopicTypeSupport<T> typeSupport;
    private final QosProfile qos;
    private final InstanceHandle handle;
    private final ConcurrentLinkedQueue<Sample<T>> queue = new ConcurrentLinkedQueue<>();
    private final Consumer<byte[]> dispatcher;
    private boolean closed = false;
    private boolean enabled = false;

    public DataReader(int domainId, Topic<T> topic, TopicTypeSupport<T> typeSupport, QosProfile qos) {
        this.domainId = domainId;
        this.topic = topic;
        this.typeSupport = typeSupport;
        this.qos = qos;
        this.handle = uuidHandle();
        this.dispatcher = bytes -> {
            T value = typeSupport.deserialize(Xcdr2Codec.decoder(bytes));
            queue.add(new Sample<>(
                    value,
                    InstanceHandle.NIL,
                    Time.fromMillis(System.currentTimeMillis()),
                    Sample.SampleState.NOT_READ,
                    Sample.ViewState.NEW,
                    Sample.InstanceState.ALIVE));
        };
    }

    /** Spec §7.2.4 — read returns a snapshot of available samples without removing them. */
    public List<Sample<T>> read() {
        if (closed || !enabled) return List.of();
        return new ArrayList<>(queue);
    }

    /** Spec §7.2.4 — take removes samples from the cache and returns them. */
    public List<Sample<T>> take() {
        if (closed || !enabled) return List.of();
        List<Sample<T>> out = new ArrayList<>();
        Sample<T> s;
        while ((s = queue.poll()) != null) {
            out.add(s);
        }
        return out;
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
        if (!enabled) {
            InProcessBus.instance().subscribe(domainId, topic.getName(), dispatcher);
            enabled = true;
        }
        return ReturnCode.OK;
    }

    @Override
    public void close() {
        if (enabled) {
            InProcessBus.instance().unsubscribe(domainId, topic.getName(), dispatcher);
        }
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
