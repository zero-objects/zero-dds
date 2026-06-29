// SPDX-License-Identifier: Apache-2.0
package org.omg.dds.sub;

import org.omg.dds.core.Entity;
import org.omg.dds.core.InstanceHandle;
import org.omg.dds.core.ReturnCode;
import org.omg.dds.core.Time;
import org.omg.dds.core.policy.History;
import org.omg.dds.core.policy.Ownership;
import org.omg.dds.core.policy.Partition;
import org.omg.dds.core.policy.QosProfile;
import org.omg.dds.core.status.LivelinessChangedStatus;
import org.omg.dds.core.status.RequestedDeadlineMissedStatus;
import org.omg.dds.topic.ContentFilteredTopic;
import org.omg.dds.topic.Topic;
import org.omg.dds.topic.TopicTypeSupport;
import org.zerodds.internal.InProcessBus;
import org.zerodds.internal.InstanceKey;
import org.zerodds.internal.Xcdr2Codec;

import java.util.ArrayDeque;
import java.util.ArrayList;
import java.util.Deque;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.UUID;
import java.util.function.Predicate;

/**
 * OMG DDS Java-PSM DataReader — DDS-DCPS 1.4 §2.2.2.5.3.
 *
 * <p>Enforces the per-instance QoS surface reader-side: HISTORY keep-last
 * depth (per instance), EXCLUSIVE OWNERSHIP arbitration (§2.2.3.12.1),
 * PARTITION matching (§2.2.3.13), CONTENT-FILTERED-TOPIC filtering
 * (§2.2.2.3.3), TRANSIENT_LOCAL late-join replay (§2.2.3.4), instance_state
 * lifecycle (ALIVE / NOT_ALIVE_DISPOSED / NOT_ALIVE_NO_WRITERS, §2.2.2.5.1.7),
 * plus the REQUESTED_DEADLINE_MISSED and LIVELINESS_CHANGED statuses
 * (§2.2.4.1).
 */
public final class DataReader<T> implements Entity, InProcessBus.Endpoint {
    private final int domainId;
    private final Topic<T> topic;
    private final TopicTypeSupport<T> typeSupport;
    private final QosProfile qos;
    private final InstanceHandle handle;
    private final Predicate<T> contentFilter; // null = no CFT

    // Per-instance sample queue + lifecycle bookkeeping.
    private final Map<InstanceKey, Instance> instances = new LinkedHashMap<>();

    // EXCLUSIVE-ownership: per-instance current owner writerId + its strength.
    private final Map<InstanceKey, OwnerInfo> owners = new LinkedHashMap<>();

    // REQUESTED_DEADLINE_MISSED tracking.
    private int deadlineMissTotal = 0;
    private int deadlineMissReadBaseline = 0;
    private InstanceHandle lastDeadlineMissHandle = InstanceHandle.NIL;

    // LIVELINESS_CHANGED tracking (against matched writers).
    private int livAliveReadBaseline = 0;
    private int livNotAliveReadBaseline = 0;

    private boolean closed = false;
    private boolean enabled = false;

    private static final class Instance {
        final Deque<Sample<Object>> samples = new ArrayDeque<>();
        Sample.InstanceState state = Sample.InstanceState.ALIVE;
        long lastSampleMillis = System.currentTimeMillis();
        int deadlinePeriodsCharged = 0; // missed-periods already counted
        boolean viewNew = true;
    }

    private static final class OwnerInfo {
        long writerId;
        int strength;
    }

    public DataReader(int domainId, Topic<T> topic, TopicTypeSupport<T> typeSupport, QosProfile qos) {
        this(domainId, topic, typeSupport, qos, null);
    }

    public DataReader(int domainId, Topic<T> topic, TopicTypeSupport<T> typeSupport,
                      QosProfile qos, Predicate<T> contentFilter) {
        this.domainId = domainId;
        this.topic = topic;
        this.typeSupport = typeSupport;
        this.qos = qos;
        this.handle = uuidHandle();
        this.contentFilter = contentFilter;
    }

    // ---- bus delivery ----------------------------------------------------

    @Override
    public synchronized void onMessage(InProcessBus.Message m) {
        // PARTITION (§2.2.3.13): only accept from writers whose partition set
        // overlaps the reader's.
        if (!partitionOverlaps(m.partitions)) {
            return;
        }
        InstanceKey key = new InstanceKey(m.keyHash);

        switch (m.kind) {
            case WRITE:
                onWrite(m, key);
                break;
            case DISPOSE:
                markState(key, Sample.InstanceState.NOT_ALIVE_DISPOSED, m);
                owners.remove(key);
                break;
            case UNREGISTER:
                if (m.keyHash.length == 0 && !typeSupport.isKeyed()
                        && instances.isEmpty()) {
                    // liveliness-lost ping for an un-keyed reader with no data;
                    // nothing to mark.
                    break;
                }
                markState(key, Sample.InstanceState.NOT_ALIVE_NO_WRITERS, m);
                owners.remove(key);
                break;
            default:
        }
    }

    private void onWrite(InProcessBus.Message m, InstanceKey key) {
        // EXCLUSIVE OWNERSHIP arbitration (§2.2.3.12.1): for each instance only
        // the highest-strength writer (the owner) updates the reader's view.
        if (m.exclusive && qos.ownership().kind() == Ownership.Kind.EXCLUSIVE) {
            OwnerInfo owner = owners.get(key);
            if (owner == null || m.ownershipStrength > owner.strength
                    || (m.ownershipStrength == owner.strength && m.writerId == owner.writerId)) {
                if (owner == null) owner = new OwnerInfo();
                owner.writerId = m.writerId;
                owner.strength = m.ownershipStrength;
                owners.put(key, owner);
            } else if (m.writerId != owner.writerId) {
                return; // lower-strength non-owner writer -> filtered out
            }
        }

        T value = typeSupport.deserialize(Xcdr2Codec.decoder(m.payload));

        // CONTENT-FILTERED-TOPIC (§2.2.2.3.3): drop samples failing the filter.
        if (contentFilter != null && !contentFilter.test(value)) {
            return;
        }

        Instance inst = instances.computeIfAbsent(key, k -> new Instance());
        boolean reborn = inst.state != Sample.InstanceState.ALIVE;
        inst.state = Sample.InstanceState.ALIVE;
        inst.lastSampleMillis = m.sourceTimestampMillis;
        inst.deadlinePeriodsCharged = 0;
        Sample.ViewState view = (reborn || inst.viewNew) ? Sample.ViewState.NEW
                : Sample.ViewState.NOT_NEW;
        inst.viewNew = false;

        @SuppressWarnings("unchecked")
        Sample<Object> s = (Sample<Object>) new Sample<>(
                value,
                InstanceKey.toHandle(key),
                Time.fromMillis(m.sourceTimestampMillis),
                Sample.SampleState.NOT_READ,
                view,
                Sample.InstanceState.ALIVE);

        // HISTORY keep-last depth (§2.2.3.18) per instance.
        History h = qos.history();
        if (h.kind() == History.Kind.KEEP_LAST) {
            int depth = Math.max(1, h.depth());
            while (inst.samples.size() >= depth) {
                inst.samples.pollFirst();
            }
        }
        inst.samples.addLast(s);
    }

    private void markState(InstanceKey key, Sample.InstanceState state, InProcessBus.Message m) {
        Instance inst = instances.get(key);
        if (inst == null) {
            inst = new Instance();
            instances.put(key, inst);
        }
        inst.state = state;
        // Surface a NOT_ALIVE notification sample carrying the new instance
        // state with no data (DDS-DCPS 1.4 §2.2.2.5.1.7 — readers observe the
        // disposed/no-writers transition via SampleInfo).
        @SuppressWarnings("unchecked")
        Sample<Object> s = (Sample<Object>) new Sample<>(
                null,
                InstanceKey.toHandle(key),
                Time.fromMillis(m.sourceTimestampMillis),
                Sample.SampleState.NOT_READ,
                Sample.ViewState.NOT_NEW,
                state);
        inst.samples.addLast(s);
    }

    private boolean partitionOverlaps(List<String> writerPartitions) {
        Partition mine = qos.partition();
        Partition theirs = new Partition(writerPartitions);
        return mine.overlaps(theirs);
    }

    // ---- read / take -----------------------------------------------------

    /** Spec §2.2.2.5.3.8 read — snapshot without removing. */
    public synchronized List<Sample<T>> read() {
        if (closed || !enabled) return new ArrayList<>();
        List<Sample<T>> out = new ArrayList<>();
        for (Instance inst : instances.values()) {
            for (Sample<Object> s : inst.samples) {
                out.add(cast(s));
            }
        }
        return out;
    }

    /** Spec §2.2.2.5.3.13 take — remove and return. */
    public synchronized List<Sample<T>> take() {
        if (closed || !enabled) return new ArrayList<>();
        List<Sample<T>> out = new ArrayList<>();
        for (Instance inst : instances.values()) {
            Sample<Object> s;
            while ((s = inst.samples.pollFirst()) != null) {
                out.add(cast(s));
            }
        }
        return out;
    }

    @SuppressWarnings("unchecked")
    private Sample<T> cast(Sample<Object> s) {
        return (Sample<T>) (Sample<?>) s;
    }

    /** Spec §2.2.2.5.3.21 lookup_instance. */
    public synchronized InstanceHandle lookupInstance(T instanceData) {
        InstanceKey key = new InstanceKey(
                typeSupport.isKeyed() ? typeSupport.keyHash(instanceData) : new byte[0]);
        return instances.containsKey(key) ? InstanceKey.toHandle(key) : InstanceHandle.NIL;
    }

    // ---- statuses --------------------------------------------------------

    /**
     * Spec §2.2.4.1 REQUESTED_DEADLINE_MISSED. Evaluated on demand: for each
     * ALIVE instance whose wall-clock gap since its last sample exceeds the
     * requested DEADLINE period, count the elapsed whole periods as misses
     * (each period charged at most once). No background thread / busy-poll.
     */
    public synchronized RequestedDeadlineMissedStatus getRequestedDeadlineMissedStatus() {
        long periodMs = qos.deadline().period().isInfinite()
                ? Long.MAX_VALUE : qos.deadline().period().toMillis();
        if (periodMs != Long.MAX_VALUE && periodMs > 0) {
            long now = System.currentTimeMillis();
            for (Map.Entry<InstanceKey, Instance> e : instances.entrySet()) {
                Instance inst = e.getValue();
                if (inst.state != Sample.InstanceState.ALIVE) continue;
                long elapsed = now - inst.lastSampleMillis;
                int periodsElapsed = (int) (elapsed / periodMs);
                if (periodsElapsed > inst.deadlinePeriodsCharged) {
                    int delta = periodsElapsed - inst.deadlinePeriodsCharged;
                    deadlineMissTotal += delta;
                    inst.deadlinePeriodsCharged = periodsElapsed;
                    lastDeadlineMissHandle = InstanceKey.toHandle(e.getKey());
                }
            }
        }
        int change = deadlineMissTotal - deadlineMissReadBaseline;
        deadlineMissReadBaseline = deadlineMissTotal;
        return new RequestedDeadlineMissedStatus(deadlineMissTotal, change, lastDeadlineMissHandle);
    }

    /**
     * Spec §2.2.4.1 LIVELINESS_CHANGED. Counts matched writers (partition +
     * RxO compatible) that are currently asserting liveliness vs. not.
     */
    public synchronized LivelinessChangedStatus getLivelinessChangedStatus() {
        int alive = 0, notAlive = 0;
        for (InProcessBus.WriterPresence w : InProcessBus.instance().writers(domainId, topic.getName())) {
            if (!new Partition(qos.partition().names()).overlaps(new Partition(w.partitions()))) {
                continue;
            }
            if (w.isAlive()) alive++;
            else notAlive++;
        }
        int aliveChange = alive - livAliveReadBaseline;
        int notAliveChange = notAlive - livNotAliveReadBaseline;
        livAliveReadBaseline = alive;
        livNotAliveReadBaseline = notAlive;
        return new LivelinessChangedStatus(alive, notAlive, aliveChange, notAliveChange);
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
            List<InProcessBus.WriterPresence> present =
                    InProcessBus.instance().registerReader(domainId, topic.getName(), this);
            enabled = true;
            // TRANSIENT_LOCAL late-join replay (§2.2.3.4): pull retained samples
            // from already-present, partition-matching writers.
            for (InProcessBus.WriterPresence w : present) {
                if (new Partition(qos.partition().names()).overlaps(new Partition(w.partitions()))) {
                    w.replayTo(this);
                }
            }
        }
        return ReturnCode.OK;
    }

    @Override
    public void close() {
        if (enabled) {
            InProcessBus.instance().unregisterReader(domainId, topic.getName(), this);
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
