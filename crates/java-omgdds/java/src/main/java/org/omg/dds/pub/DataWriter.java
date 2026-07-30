// SPDX-License-Identifier: Apache-2.0
package org.omg.dds.pub;

import org.omg.dds.core.Entity;
import org.omg.dds.core.InstanceHandle;
import org.omg.dds.core.ReturnCode;
import org.omg.dds.core.policy.Durability;
import org.omg.dds.core.policy.History;
import org.omg.dds.core.policy.Ownership;
import org.omg.dds.core.policy.QosProfile;
import org.omg.dds.topic.Topic;
import org.omg.dds.topic.TopicTypeSupport;
import org.zerodds.internal.InProcessBus;
import org.zerodds.internal.InstanceKey;
import org.zerodds.internal.Xcdr2Codec;

import java.nio.ByteBuffer;
import java.util.ArrayDeque;
import java.util.ArrayList;
import java.util.Deque;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.UUID;
import java.util.concurrent.atomic.AtomicLong;

/**
 * OMG DDS Java-PSM DataWriter — DDS-DCPS 1.4 §2.2.2.4.2.
 *
 * <p>Honors the per-instance QoS surface: HISTORY keep-last depth (per
 * instance) on the retained cache, DURABILITY TRANSIENT_LOCAL late-join
 * replay, OWNERSHIP (+ strength) arbitration metadata, PARTITION matching,
 * LIVELINESS assertion, and the keyed lifecycle operations
 * dispose / unregister_instance / register_instance.
 */
public final class DataWriter<T> implements Entity, InProcessBus.WriterPresence {
    private static final AtomicLong WRITER_IDS = new AtomicLong(1);

    private final int domainId;
    private final Topic<T> topic;
    private final TopicTypeSupport<T> typeSupport;
    private final QosProfile qos;
    private final InstanceHandle handle;
    private final long writerId = WRITER_IDS.getAndIncrement();

    // Per-instance retained samples for TRANSIENT_LOCAL late-join replay,
    // capped by HISTORY keep-last depth (DDS-DCPS 1.4 §2.2.3.18 / §2.2.3.4).
    private final Map<InstanceKey, Deque<byte[]>> retained = new LinkedHashMap<>();
    // Registered instances (register_instance / first write) — §2.2.2.4.2.5.
    private final Map<InstanceKey, InstanceHandle> instances = new LinkedHashMap<>();

    private volatile boolean alive = true;
    private boolean closed = false;
    private boolean enabled = false;

    // Optional pure-Java RTPS wire path (cross-process), alongside InProcessBus.
    private org.zerodds.rtps.RtpsParticipant.WireWriter wire;

    public DataWriter(int domainId, Topic<T> topic, TopicTypeSupport<T> typeSupport, QosProfile qos) {
        this.domainId = domainId;
        this.topic = topic;
        this.typeSupport = typeSupport;
        this.qos = qos;
        this.handle = uuidHandle();
    }

    // ---- WriterPresence (bus-side view) ---------------------------------

    @Override
    public long writerId() {
        return writerId;
    }

    @Override
    public List<String> partitions() {
        return qos.partition().names();
    }

    @Override
    public boolean exclusive() {
        return qos.ownership().kind() == Ownership.Kind.EXCLUSIVE;
    }

    @Override
    public int ownershipStrength() {
        return qos.ownershipStrength().value();
    }

    @Override
    public boolean isAlive() {
        return alive && !closed;
    }

    @Override
    public synchronized void replayTo(InProcessBus.Endpoint reader) {
        // TRANSIENT_LOCAL: replay retained samples to a freshly-joined reader
        // (DDS-DCPS 1.4 §2.2.3.4: TRANSIENT_LOCAL keeps data available to
        // late-joining DataReaders for the lifetime of the DataWriter).
        if (qos.durability() == Durability.VOLATILE) {
            return;
        }
        for (Map.Entry<InstanceKey, Deque<byte[]>> e : retained.entrySet()) {
            for (byte[] payload : e.getValue()) {
                reader.onMessage(message(payload, e.getKey().bytes(),
                        InProcessBus.ChangeKind.WRITE, System.currentTimeMillis()));
            }
        }
    }

    // ---- write / lifecycle ----------------------------------------------

    public ReturnCode write(T data) {
        return writeAt(data, System.currentTimeMillis());
    }

    /** Spec §2.2.2.4.2.11 write_w_timestamp. */
    public ReturnCode write(T data, long sourceTimestampMillis) {
        return writeAt(data, sourceTimestampMillis);
    }

    private synchronized ReturnCode writeAt(T data, long ts) {
        if (closed) return ReturnCode.ALREADY_DELETED;
        if (!enabled) return ReturnCode.NOT_ENABLED;
        byte[] payload = serialize(data);
        InstanceKey key = keyOf(data);
        instances.putIfAbsent(key, InstanceKey.toHandle(key));
        retainCapped(key, payload);
        publish(message(payload, key.bytes(), InProcessBus.ChangeKind.WRITE, ts));
        if (wire != null) {
            wire.write(payload); // cross-process RTPS/UDP delivery
        }
        return ReturnCode.OK;
    }

    /** Spec §2.2.2.4.2.13 register_instance — establishes the instance. */
    public synchronized InstanceHandle registerInstance(T instanceData) {
        InstanceKey key = keyOf(instanceData);
        return instances.computeIfAbsent(key, InstanceKey::toHandle);
    }

    /** Spec §2.2.2.4.2.13 register_instance_w_timestamp. */
    public InstanceHandle registerInstance(T instanceData, long sourceTimestampMillis) {
        return registerInstance(instanceData);
    }

    /** Spec §2.2.2.4.2.16 dispose — instance -> NOT_ALIVE_DISPOSED. */
    public ReturnCode dispose(T instanceData) {
        return disposeAt(instanceData, System.currentTimeMillis());
    }

    /** Spec §2.2.2.4.2.16 dispose_w_timestamp. */
    public ReturnCode dispose(T instanceData, long sourceTimestampMillis) {
        return disposeAt(instanceData, sourceTimestampMillis);
    }

    private synchronized ReturnCode disposeAt(T instanceData, long ts) {
        if (closed) return ReturnCode.ALREADY_DELETED;
        if (!enabled) return ReturnCode.NOT_ENABLED;
        InstanceKey key = keyOf(instanceData);
        retained.remove(key); // disposed instance no longer replayed
        publish(message(null, key.bytes(), InProcessBus.ChangeKind.DISPOSE, ts));
        return ReturnCode.OK;
    }

    /** Spec §2.2.2.4.2.15 unregister_instance — instance -> NOT_ALIVE_NO_WRITERS. */
    public ReturnCode unregisterInstance(T instanceData) {
        return unregisterAt(instanceData, System.currentTimeMillis());
    }

    /** Spec §2.2.2.4.2.15 unregister_instance_w_timestamp. */
    public ReturnCode unregisterInstance(T instanceData, long sourceTimestampMillis) {
        return unregisterAt(instanceData, sourceTimestampMillis);
    }

    private synchronized ReturnCode unregisterAt(T instanceData, long ts) {
        if (closed) return ReturnCode.ALREADY_DELETED;
        if (!enabled) return ReturnCode.NOT_ENABLED;
        InstanceKey key = keyOf(instanceData);
        instances.remove(key);
        retained.remove(key);
        publish(message(null, key.bytes(), InProcessBus.ChangeKind.UNREGISTER, ts));
        return ReturnCode.OK;
    }

    /** Spec §2.2.2.4.2.18 lookup_instance. */
    public synchronized InstanceHandle lookupInstance(T instanceData) {
        InstanceHandle h = instances.get(keyOf(instanceData));
        return h == null ? InstanceHandle.NIL : h;
    }

    /** Spec §2.2.2.4.2.22 assert_liveliness — renew this writer's liveliness. */
    public synchronized ReturnCode assertLiveliness() {
        if (closed) return ReturnCode.ALREADY_DELETED;
        alive = true;
        return ReturnCode.OK;
    }

    /**
     * Test/diagnostic hook: simulate a liveliness lease expiry without closing
     * the writer (DDS-DCPS 1.4 §2.2.3.11 — a writer that fails to renew within
     * its lease is reported not_alive to matched readers).
     */
    public void simulateLivelinessLost() {
        alive = false;
        // Notify matched readers so they update LIVELINESS_CHANGED immediately.
        for (InProcessBus.Endpoint r : InProcessBus.instance().readers(domainId, topic.getName())) {
            r.onMessage(message(null, new byte[0],
                    InProcessBus.ChangeKind.UNREGISTER, System.currentTimeMillis()));
        }
    }

    // ---- helpers ---------------------------------------------------------

    private void retainCapped(InstanceKey key, byte[] payload) {
        Deque<byte[]> dq = retained.computeIfAbsent(key, k -> new ArrayDeque<>());
        History h = qos.history();
        if (h.kind() == History.Kind.KEEP_LAST) {
            int depth = Math.max(1, h.depth());
            while (dq.size() >= depth) {
                dq.pollFirst();
            }
        }
        dq.addLast(payload);
    }

    private InProcessBus.Message message(byte[] payload, byte[] keyHash,
                                         InProcessBus.ChangeKind kind, long ts) {
        return new InProcessBus.Message(payload, keyHash, kind, writerId,
                qos.ownershipStrength().value(), exclusive(),
                qos.partition().names(), ts);
    }

    private void publish(InProcessBus.Message m) {
        InProcessBus.instance().publish(domainId, topic.getName(), m);
    }

    private InstanceKey keyOf(T data) {
        return new InstanceKey(typeSupport.isKeyed() ? typeSupport.keyHash(data) : new byte[0]);
    }

    private byte[] serialize(T data) {
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
            return Xcdr2Codec.copyToBytes(encoder);
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
        if (!enabled) {
            enabled = true;
            InProcessBus.instance().registerWriter(domainId, topic.getName(), this);
            if (org.zerodds.rtps.DdsWireBridge.enabled()) {
                byte[] typeId = new byte[0];
                if (typeSupport instanceof org.zerodds.cdr.TopicTypeSupport) {
                    typeId = ((org.zerodds.cdr.TopicTypeSupport<T>) typeSupport).typeIdentifier();
                }
                wire = org.zerodds.rtps.DdsWireBridge.writer(domainId, topic.getName(),
                        typeSupport.getTypeName(), org.zerodds.rtps.DdsWireBridge.encapFor(typeSupport),
                        typeId, typeSupport.isKeyed());
            }
        }
        return ReturnCode.OK;
    }

    @Override
    public void close() {
        if (enabled && !closed) {
            // Closing a writer makes its instances NOT_ALIVE_NO_WRITERS for
            // matched readers (DDS-DCPS 1.4 §2.2.2.4.2 deletion semantics).
            for (Map.Entry<InstanceKey, InstanceHandle> e : new ArrayList<>(instances.entrySet())) {
                publish(message(null, e.getKey().bytes(),
                        InProcessBus.ChangeKind.UNREGISTER, System.currentTimeMillis()));
            }
            InProcessBus.instance().unregisterWriter(domainId, topic.getName(), this);
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
