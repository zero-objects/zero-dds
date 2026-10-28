// SPDX-License-Identifier: Apache-2.0
package org.zerodds.pub;

import org.omg.dds.core.Duration;
import org.omg.dds.core.ModifiableInstanceHandle;
import org.omg.dds.core.StatusCondition;
import org.omg.dds.core.DomainEntity;
import org.omg.dds.pub.DataWriter;
import org.omg.dds.pub.DataWriterListener;
import org.omg.dds.pub.DataWriterQos;
import org.omg.dds.pub.Publisher;
import org.omg.dds.pub.DataWriterListener;
import org.omg.dds.pub.DataWriterQos;
import org.omg.dds.pub.Publisher;
import org.omg.dds.core.InstanceHandle;
import org.omg.dds.core.QosProvider;
import org.omg.dds.core.ServiceEnvironment;
import org.omg.dds.core.policy.Durability;
import org.omg.dds.core.policy.History;
import org.omg.dds.core.policy.Liveliness;
import org.omg.dds.core.policy.Ownership;
import org.omg.dds.core.policy.QosPolicy;
import org.omg.dds.core.policy.Reliability;
import org.omg.dds.core.policy.ResourceLimits;
import org.omg.dds.core.policy.QosPolicyCount;
import org.omg.dds.core.status.LivelinessLostStatus;
import org.omg.dds.core.status.OfferedDeadlineMissedStatus;
import org.omg.dds.core.status.OfferedIncompatibleQosStatus;
import org.omg.dds.core.status.PublicationMatchedStatus;
import org.omg.dds.core.status.Status;
import org.omg.dds.topic.SubscriptionBuiltinTopicData;
import org.omg.dds.topic.Topic;
import org.zerodds.core.AlreadyClosedException;
import org.zerodds.core.EntityBase;
import org.zerodds.core.ErrorException;
import org.zerodds.core.NotEnabledException;
import org.zerodds.core.OutOfResourcesException;
import org.zerodds.core.PreconditionNotMetException;
import org.zerodds.core.Values;
import org.zerodds.core.event.LivelinessLostEventImpl;
import org.zerodds.core.event.OfferedDeadlineMissedEventImpl;
import org.zerodds.core.event.OfferedIncompatibleQosEventImpl;
import org.zerodds.core.event.PublicationMatchedEventImpl;
import org.zerodds.core.qos.QosCompatibility;
import org.zerodds.core.status.LivelinessLostStatusImpl;
import org.zerodds.core.status.OfferedDeadlineMissedStatusImpl;
import org.zerodds.core.status.OfferedIncompatibleQosStatusImpl;
import org.zerodds.core.status.PublicationMatchedStatusImpl;
import org.zerodds.core.status.QosPolicyCountImpl;
import org.zerodds.cdr.TopicTypeSupport;
import org.zerodds.internal.DurabilityService;
import org.zerodds.internal.InProcessBus;
import org.zerodds.internal.InstanceKey;
import org.zerodds.internal.LeaseMonitor;
import org.zerodds.internal.Ignores;
import org.zerodds.internal.Xcdr2Codec;

import java.nio.ByteBuffer;
import java.util.ArrayDeque;
import java.util.ArrayList;
import java.util.Deque;
import java.util.LinkedHashMap;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.UUID;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.TimeoutException;
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
public final class DataWriterImpl<T> extends EntityBase<DataWriterListener<T>, DataWriterQos>
        implements DataWriter<T>, DomainEntity<DataWriterListener<T>, DataWriterQos>,
        InProcessBus.WriterPresence {
    private static final AtomicLong WRITER_IDS = new AtomicLong(1);

    /** The communication statuses of a DataWriter (DDS 1.2 §7.1.4.1). */
    private static final Class<? extends Status> OFFERED_DEADLINE_MISSED =
            OfferedDeadlineMissedStatus.class;
    private static final Class<? extends Status> OFFERED_INCOMPATIBLE =
            OfferedIncompatibleQosStatus.class;
    private static final Class<? extends Status> LIVELINESS_LOST = LivelinessLostStatus.class;
    private static final Class<? extends Status> PUBLICATION_MATCHED =
            PublicationMatchedStatus.class;

    private final ServiceEnvironment env = env();
    private final int domainId;
    private final Publisher parent;
    private final Topic<T> topic;
    private final TopicTypeSupport<T> typeSupport;
    private volatile DataWriterQos qos;
    /** The partition names of the writer's Publisher (DDS 1.2 §7.1.3.9). */
    private volatile Set<String> partitions;
    private final long writerId = WRITER_IDS.getAndIncrement();

    // Per-instance retained samples for TRANSIENT_LOCAL late-join replay,
    // capped by HISTORY keep-last depth (DDS-DCPS 1.4 §2.2.3.18 / §2.2.3.4).
    private final Map<InstanceKey, Deque<Retained>> retained = new LinkedHashMap<>();

    /** A retained sample with its source timestamp and the LIFESPAN it was written under. */
    private static final class Retained {
        final byte[] payload;
        final long timestampMillis;
        final long lifespanMillis;

        Retained(byte[] payload, long timestampMillis, long lifespanMillis) {
            this.payload = payload;
            this.timestampMillis = timestampMillis;
            this.lifespanMillis = lifespanMillis;
        }

        boolean expiredAt(long now) {
            return lifespanMillis >= 0 && now >= timestampMillis + lifespanMillis;
        }
    }
    // Registered instances (register_instance / first write) — §2.2.2.4.2.5.
    private final Map<InstanceKey, InstanceHandle> instances = new LinkedHashMap<>();

    // PUBLICATION_MATCHED tracking — the mirror of the reader's
    // SUBSCRIPTION_MATCHED bookkeeping.
    private final Set<InProcessBus.Endpoint> matchedReaders =
            java.util.Collections.newSetFromMap(new java.util.IdentityHashMap<>());
    private int pubMatchedTotal = 0;
    private int pubMatchedTotalReadBaseline = 0;
    private int pubMatchedCurrentReadBaseline = 0;
    /** The reader whose match or unmatch changed the status last (DDS 1.4 2.2.4.1). */
    private InstanceHandle lastSubscriptionHandle = Values.nil();

    // OFFERED_INCOMPATIBLE_QOS tracking (DDS 1.2 §7.1.4.1): a reader whose
    // requested QoS is incompatible with this writer's offered QoS is not a
    // match; the refusal is recorded per policy.
    private int offeredIncompatTotal = 0;
    private int offeredIncompatReadBaseline = 0;
    private Class<? extends QosPolicy> lastOfferedIncompatPolicyClass = null;
    private final Map<Class<? extends QosPolicy>, Integer> offeredIncompatPolicies = new LinkedHashMap<>();

    private volatile boolean alive = true;
    // Written under the writer's monitor, read by the bus, by listener
    // callbacks and by application threads without it (DDS-Java-PSM 1.0
    // §7.2.4 — reentrant operations).
    private volatile boolean closed = false;
    private volatile boolean enabled = false;

    // Deadline + liveliness lease bookkeeping (DDS 1.2 §7.1.4.2 — these are
    // time-based, so they are evaluated on a lease thread, not only on demand).
    private final Map<InstanceKey, Long> lastWriteMillis = new LinkedHashMap<>();
    private volatile long lastLivelinessMillis = System.currentTimeMillis();
    private long offeredDeadlineMissTotal = 0;
    private long offeredDeadlineMissReadBaseline = 0;
    private InstanceHandle lastOfferedDeadlineHandle = Values.nil();
    private long livelinessLostTotal = 0;
    private long livelinessLostReadBaseline = 0;
    private java.util.concurrent.ScheduledFuture<?> deadlineLease;
    private java.util.concurrent.ScheduledFuture<?> livelinessLease;

    // Optional pure-Java RTPS wire path (cross-process), alongside InProcessBus.
    private org.zerodds.rtps.RtpsParticipant.WireWriter wire;
    /** The DDS-RPC 1.0 §7.6.2.1 discovery extensions this writer announces. */
    private volatile org.zerodds.rtps.RpcDiscovery rpc = org.zerodds.rtps.RpcDiscovery.NONE;
    /** The sequence number of the last sample this writer sent without a wire writer. */
    private long identitySequence;
    /** The last sample written for each instance, for get_key_value. */
    private final Map<InstanceKey, byte[]> lastPayload = new java.util.HashMap<>();

    public DataWriterImpl(int domainId, Publisher parent, Topic<T> topic,
                      TopicTypeSupport<T> typeSupport,
                      DataWriterQos qos, Set<String> partitions) {
        super(parent == null ? Values.getDefaultEnvironment() : parent.getEnvironment());
        org.zerodds.core.qos.QosRules.checkConsistent(qos);
        this.domainId = domainId;
        this.parent = parent;
        this.topic = topic;
        this.typeSupport = typeSupport;
        this.qos = qos;
        this.partitions = new LinkedHashSet<>(partitions);
        if (topic instanceof org.zerodds.topic.TopicImpl) {
            final org.zerodds.topic.TopicImpl<?> used = (org.zerodds.topic.TopicImpl<?>) topic;
            used.use();
            addCloseHook(used::release);
        }
        if (parent != null && parent.getParent() != null) {
            // the participant that owns this writer decides what it ignores (DDS 1.4 2.2.2.2.1.14)
            Ignores.register(getInstanceHandle(), parent.getParent().getInstanceHandle(),
                    topic.getName());
            addCloseHook(() -> Ignores.forget(getInstanceHandle()));
        }
    }

    @Override
    public Publisher getParent() {
        return parent;
    }

    @Override
    protected DataWriterQos profileQos(QosProvider provider) {
        return provider.getDataWriterQos();
    }

    /**
     * Spec §2.2.4.1 OFFERED_INCOMPATIBLE_QOS. Recorded when a reader with an
     * incompatible requested QoS attempted to match (see
     * {@link #onReaderPresenceChanged}).
     */
    public synchronized OfferedIncompatibleQosStatus getOfferedIncompatibleQosStatus() {
        clearStatus(OFFERED_INCOMPATIBLE);
        return readOfferedIncompatibleStatusLocked();
    }

    private OfferedIncompatibleQosStatus readOfferedIncompatibleStatusLocked() {
        int change = offeredIncompatTotal - offeredIncompatReadBaseline;
        offeredIncompatReadBaseline = offeredIncompatTotal;
        Set<QosPolicyCount> policies = new LinkedHashSet<>();
        for (Map.Entry<Class<? extends QosPolicy>, Integer> e : offeredIncompatPolicies.entrySet()) {
            policies.add(new QosPolicyCountImpl(env, e.getKey(), e.getValue()));
        }
        return new OfferedIncompatibleQosStatusImpl(
                env, offeredIncompatTotal, change, lastOfferedIncompatPolicyClass, policies);
    }

    /**
     * One refused reader: the total count moves once, and every incompatible policy
     * is counted in the per-policy counts (DDS 1.4 §2.2.4.1).
     */
    private void recordOfferedIncompatibleLocked(
            List<Class<? extends QosPolicy>> policyClasses) {
        triggerStatus(OFFERED_INCOMPATIBLE);
        offeredIncompatTotal++;
        for (Class<? extends QosPolicy> policyClass : policyClasses) {
            lastOfferedIncompatPolicyClass = policyClass;
            offeredIncompatPolicies.merge(policyClass, 1, Integer::sum);
        }
    }

    /**
     * Spec §2.2.4.1 PUBLICATION_MATCHED. The counts are maintained by the
     * bus's match notification, so reading the status does not walk the bus.
     */
    public synchronized PublicationMatchedStatus getPublicationMatchedStatus() {
        clearStatus(PUBLICATION_MATCHED);
        return readPublicationMatchedStatusLocked();
    }

    private PublicationMatchedStatus readPublicationMatchedStatusLocked() {
        int current = matchedReaders.size();
        int totalChange = pubMatchedTotal - pubMatchedTotalReadBaseline;
        int currentChange = current - pubMatchedCurrentReadBaseline;
        pubMatchedTotalReadBaseline = pubMatchedTotal;
        pubMatchedCurrentReadBaseline = current;
        return new PublicationMatchedStatusImpl(env, pubMatchedTotal, totalChange, current,
                currentChange, lastSubscriptionHandle);
    }

    /**
     * Spec §2.2.4.1 OFFERED_DEADLINE_MISSED. Evaluated by the lease scheduler
     * (and, on demand, here): for each managed instance whose wall-clock gap
     * since its last write exceeds the offered DEADLINE period, count the
     * elapsed whole periods (each period charged at most once). DDS 1.2
     * §7.1.4.2 — the miss is reported to the listener without a status poll.
     */
    public synchronized OfferedDeadlineMissedStatus getOfferedDeadlineMissedStatus() {
        clearStatus(OFFERED_DEADLINE_MISSED);
        evaluateOfferedDeadline(false);
        int change = (int) (offeredDeadlineMissTotal - offeredDeadlineMissReadBaseline);
        offeredDeadlineMissReadBaseline = offeredDeadlineMissTotal;
        return new OfferedDeadlineMissedStatusImpl(
                env, (int) offeredDeadlineMissTotal, change, lastOfferedDeadlineHandle);
    }

    /** Spec §2.2.4.1 LIVELINESS_LOST — the writer failed to renew its lease. */
    public synchronized LivelinessLostStatus getLivelinessLostStatus() {
        clearStatus(LIVELINESS_LOST);
        evaluateLiveliness(false);
        int change = (int) (livelinessLostTotal - livelinessLostReadBaseline);
        livelinessLostReadBaseline = livelinessLostTotal;
        return new LivelinessLostStatusImpl(env, (int) livelinessLostTotal, change);
    }

    /**
     * Evaluates offered-deadline misses. When {@code fire} is true and a miss
     * is detected, the listener is handed the status — outside the writer's
     * monitor, so the callback cannot block a concurrent call on another
     * entity (DDS-Java-PSM 1.0 §7.2.4).
     */
    private void evaluateOfferedDeadline(boolean fire) {
        DataWriterListener<T> l = null;
        OfferedDeadlineMissedStatus status = null;
        synchronized (this) {
            Duration period = qos.getDeadline().getPeriod();
            if (period.isInfinite()) {
                return;
            }
            long periodMs = period.getDuration(TimeUnit.MILLISECONDS);
            if (periodMs <= 0) {
                return;
            }
            long now = System.currentTimeMillis();
            boolean missed = false;
            for (Map.Entry<InstanceKey, Long> e : lastWriteMillis.entrySet()) {
                long last = e.getValue();
                long elapsed = now - last;
                int periodsElapsed = (int) (elapsed / periodMs);
                if (periodsElapsed > 0) {
                    offeredDeadlineMissTotal += periodsElapsed;
                    lastOfferedDeadlineHandle = InstanceKey.toHandle(e.getKey());
                    lastWriteMillis.put(e.getKey(), last + (long) periodsElapsed * periodMs);
                    missed = true;
                }
            }
            if (missed) {
                triggerStatus(OFFERED_DEADLINE_MISSED);
            }
            if (fire && missed) {
                l = listenerFor(OFFERED_DEADLINE_MISSED);
                if (l != null) {
                    int change = (int) (offeredDeadlineMissTotal - offeredDeadlineMissReadBaseline);
                    offeredDeadlineMissReadBaseline = offeredDeadlineMissTotal;
                    status = new OfferedDeadlineMissedStatusImpl(
                            env, (int) offeredDeadlineMissTotal, change, lastOfferedDeadlineHandle);
                }
            }
        }
        if (l != null) {
            l.onOfferedDeadlineMissed(new OfferedDeadlineMissedEventImpl<>(env, this, status));
        }
    }

    /**
     * Evaluates liveliness loss. A MANUAL-mode writer with a finite lease that
     * has not asserted within {@code lease_duration} is no longer alive; the
     * transition is counted once and reported to the listener outside the
     * writer's monitor (§7.2.4).
     */
    private void evaluateLiveliness(boolean fire) {
        DataWriterListener<T> l = null;
        LivelinessLostStatus status = null;
        synchronized (this) {
            Liveliness liv = qos.getLiveliness();
            if (liv.getKind() == Liveliness.Kind.AUTOMATIC || liv.getLeaseDuration().isInfinite()) {
                return;
            }
            long leaseMs = liv.getLeaseDuration().getDuration(TimeUnit.MILLISECONDS);
            if (!alive || closed || leaseMs <= 0) {
                return;
            }
            if (System.currentTimeMillis() - lastLivelinessMillis > leaseMs) {
                alive = false;
                livelinessLostTotal++;
                triggerStatus(LIVELINESS_LOST);
                if (fire) {
                    l = listenerFor(LIVELINESS_LOST);
                    if (l != null) {
                        int change = (int) (livelinessLostTotal - livelinessLostReadBaseline);
                        livelinessLostReadBaseline = livelinessLostTotal;
                        status = new LivelinessLostStatusImpl(
                                env, (int) livelinessLostTotal, change);
                    }
                }
            }
        }
        if (l != null) {
            l.onLivelinessLost(new LivelinessLostEventImpl<>(env, this, status));
        }
    }

    // ---- WriterPresence (bus-side view) ---------------------------------

    @Override
    public void onReaderPresenceChanged(InProcessBus.Endpoint reader, boolean present) {
        DataWriterListener<T> l;
        PublicationMatchedStatus matched = null;
        OfferedIncompatibleQosStatus refused = null;
        synchronized (this) {
            // §2.2.3.13 — same partition rule as the reader applies, from this
            // side.
            // A reader whose partitions do not overlap is no match; when its partitions or
            // this writer's changed, one that was a match ends being one.
            if (!overlaps(reader.partitions())) {
                present = false;
            }
            // §2.2.3 RxO compatibility: a reader whose requested QoS is
            // incompatible is not a match, and the refusal is recorded
            // (OFFERED_INCOMPATIBLE_QOS, §7.1.4.1).
            Map<Class<? extends QosPolicy>, ? extends QosPolicy> requested = reader.requestedQos();
            if (present && requested != null
                    && !QosCompatibility.isCompatible(requested, offeredPolicies())) {
                recordOfferedIncompatibleLocked(
                        QosCompatibility.incompatiblePolicies(requested, offeredPolicies()));
                l = listenerFor(OFFERED_INCOMPATIBLE);
                refused = l == null ? null : readOfferedIncompatibleStatusLocked();
            } else {
                boolean changed = present
                        ? matchedReaders.add(reader)
                        : matchedReaders.remove(reader);
                if (!changed) {
                    return;
                }
                if (present) {
                    pubMatchedTotal++;
                }
                if (reader.instanceHandle() != null) {
                    lastSubscriptionHandle = reader.instanceHandle();
                }
                triggerStatus(PUBLICATION_MATCHED);
                l = listenerFor(PUBLICATION_MATCHED);
                // The change counters move only when the status is actually
                // handed to a listener.
                matched = l == null ? null : readPublicationMatchedStatusLocked();
            }
        }
        if (l != null && refused != null) {
            l.onOfferedIncompatibleQos(
                    new OfferedIncompatibleQosEventImpl<>(env, this, refused));
        } else if (l != null && matched != null) {
            l.onPublicationMatched(new PublicationMatchedEventImpl<>(env, this, matched));
        }
    }

    /**
     * The listener told about a change of {@code kind}: the writer's own, else the
     * Publisher's, else the DomainParticipant's (DDS 1.4 §2.2.4.3.1).
     */
    private DataWriterListener<T> listenerFor(Class<? extends Status> kind) {
        DataWriterListener<T> own = ownListener(kind);
        if (own != null) {
            return own;
        }
        if (parent instanceof PublisherImpl) {
            PublisherImpl publisher = (PublisherImpl) parent;
            org.omg.dds.pub.PublisherListener up = publisher.ownListener(kind);
            if (up != null) {
                return org.zerodds.core.ListenerUp.forWriter(up);
            }
            if (publisher.getParent() instanceof org.zerodds.domain.DomainParticipantImpl) {
                org.omg.dds.domain.DomainParticipantListener top =
                        ((org.zerodds.domain.DomainParticipantImpl) publisher.getParent())
                                .ownListener(kind);
                if (top != null) {
                    return org.zerodds.core.ListenerUp.forWriter(top);
                }
            }
        }
        return null;
    }

    @Override
    public long writerId() {
        return writerId;
    }

    /**
     * The PARTITION of the Publisher changed (DDS 1.4 §2.2.3.13): the writer matches the readers
     * whose partitions overlap the new names, and no longer those whose partitions overlapped the
     * old ones.
     */
    public void changePartitions(java.util.Collection<String> names) {
        List<String> before;
        synchronized (this) {
            before = new ArrayList<>(partitions);
            partitions = new LinkedHashSet<>(names);
        }
        if (enabled && !closed) {
            InProcessBus.instance().writerPartitionsChanged(
                    domainId, topic.getName(), typeSupport.getTypeName(), this, before);
        }
    }

    @Override
    public List<String> partitions() {
        return new ArrayList<>(partitions);
    }

    @Override
    public Map<Class<? extends QosPolicy>, ? extends QosPolicy> offeredQos() {
        return offeredPolicies();
    }

    private Map<Class<? extends QosPolicy>, QosPolicy> offeredPolicies() {
        // PRESENTATION is a policy of the Publisher and part of the match (DDS 1.4 §2.2.3.6).
        return QosCompatibility.withPresentation(QosCompatibility.policiesOf(qos),
                parent == null || parent.getQos() == null ? null : parent.getQos().getPresentation());
    }

    /** DDS 1.4 §2.2.3.13: two partition sets overlap when a name of one matches a name of the other. */
    private boolean overlaps(List<String> other) {
        return org.zerodds.rtps.QosParameters.partitionsOverlap(new ArrayList<>(partitions), other);
    }

    @Override
    public boolean exclusive() {
        return qos.getOwnership().getKind() == Ownership.Kind.EXCLUSIVE;
    }

    @Override
    public int ownershipStrength() {
        return qos.getOwnershipStrength().getValue();
    }

    /**
     * Whether the remote reader of another type name takes the samples of this writer: it allows
     * type coercion (TYPE_CONSISTENCY_ENFORCEMENT, announced on SEDP) and the type of its
     * TypeObject (PID 0x0072) is assignable from the type of this writer (XTypes 1.3 7.2.4). A
     * reader that announced no TypeObject cannot be judged and does not match.
     */
    private boolean acceptsRemoteReader(org.zerodds.rtps.DiscoveryData.Endpoint reader) {
        if (reader.qos == null || reader.qos.typeConsistency == null
                || reader.qos.typeConsistency.kind == 0 || reader.qos.typeObject == null) {
            return false;
        }
        org.omg.dds.type.dynamic.DynamicType theirs =
                org.zerodds.typeobject.TypeObjects.dynamicTypeOrNull(env, reader.qos.typeObject);
        org.omg.dds.type.dynamic.DynamicType mine = dynamicType();
        return theirs != null && mine != null
                && org.zerodds.cdr.TypeAssignability.assignableFrom(theirs, mine);
    }

    @Override
    public org.omg.dds.type.dynamic.DynamicType dynamicType() {
        return org.zerodds.typeobject.TypeObjects.dynamicTypeOf(env, typeSupport);
    }

    @Override
    public boolean isAlive() {
        if (closed || !alive) {
            return false;
        }
        Liveliness l = qos.getLiveliness();
        if (l.getKind() == Liveliness.Kind.AUTOMATIC || l.getLeaseDuration().isInfinite()) {
            return true;
        }
        long leaseMs = l.getLeaseDuration().getDuration(TimeUnit.MILLISECONDS);
        return leaseMs <= 0 || System.currentTimeMillis() - lastLivelinessMillis <= leaseMs;
    }

    /**
     * Replays this writer's retained samples to a freshly-joined reader. The
     * retained set is copied under this writer's monitor; the delivery — and
     * with it the reader's listener callback — runs without it (§7.2.4).
     */
    @Override
    public void replayTo(InProcessBus.Endpoint reader) {
        // TRANSIENT_LOCAL: replay this writer's own retained samples to a
        // freshly-joined reader (DDS 1.2 §7.1.3.4 — kept for the lifetime of
        // the DataWriter). TRANSIENT and PERSISTENT samples live in the
        // DurabilityService and are replayed from there, not from here.
        List<InProcessBus.Message> replay;
        long now = System.currentTimeMillis();
        synchronized (this) {
            if (qos.getDurability().getKind() != Durability.Kind.TRANSIENT_LOCAL) {
                return;
            }
            replay = new ArrayList<>();
            for (Map.Entry<InstanceKey, Deque<Retained>> e : retained.entrySet()) {
                // DDS 1.4 §2.2.3.16: a sample whose LIFESPAN has passed is no longer replayed.
                e.getValue().removeIf(sample -> sample.expiredAt(now));
                for (Retained sample : e.getValue()) {
                    // The replay carries the source timestamp the sample was written with.
                    replay.add(message(sample.payload, e.getKey().bytes(),
                            InProcessBus.ChangeKind.WRITE, sample.timestampMillis)
                            .withLifespan(sample.lifespanMillis));
                }
            }
        }
        for (InProcessBus.Message m : replay) {
            reader.onMessage(m);
        }
    }

    // ---- write / lifecycle ----------------------------------------------

    @Override
    public void write(T data) throws TimeoutException {
        org.zerodds.core.Args.require(data, "data");
        writeAt(data, System.currentTimeMillis(), null);
    }

    /**
     * Writes {@code data} and returns the identity the sample was sent under: the GUID of
     * this writer and the sequence number it gave the sample. {@code related} is the
     * identity of the sample this one answers or follows; it travels as
     * {@code PID_RELATED_SAMPLE_IDENTITY} in the inline QoS (DDS-RPC 1.0 §7.8.2) and a
     * reader sees it on the sample.
     */
    public org.zerodds.internal.SourceIdentity writeIdentified(T data,
            org.zerodds.internal.SourceIdentity related) {
        return writeIdentified(data, related, null);
    }

    /**
     * As {@link #writeIdentified(Object, org.zerodds.internal.SourceIdentity)}, to the
     * matched readers that announce the service instance {@code instance} only (DDS-RPC 1.0
     * §7.11.1.4.3: "requests sent through a bound requester reference shall be sent to the
     * bound service instance only"); {@code null} is every matched reader.
     */
    public org.zerodds.internal.SourceIdentity writeIdentified(T data,
            org.zerodds.internal.SourceIdentity related, String instance) {
        try {
            return writeAt(data, System.currentTimeMillis(), related, instance);
        } catch (TimeoutException e) {
            // This form has no checked exception; DDS 1.4 §2.2.2.4.2.11 reports a write that
            // cannot get its resources as OUT_OF_RESOURCES as well.
            throw new org.zerodds.core.OutOfResourcesException(e.getMessage());
        }
    }

    /**
     * Sets the DDS-RPC 1.0 §7.6.2.1 discovery extensions the writer announces:
     * {@code service_instance_name} (PID 0x0080), {@code related_datareader_key}
     * (PID 0x0081) and {@code topic_aliases} (PID 0x0082). They are in the SEDP announcement
     * from the enable on, and a call after it announces the writer again.
     */
    public void setRpcDiscovery(org.zerodds.rtps.RpcDiscovery value) {
        org.zerodds.rtps.RpcDiscovery next =
                value == null ? org.zerodds.rtps.RpcDiscovery.NONE : value;
        org.zerodds.rtps.RtpsParticipant.WireWriter remote;
        synchronized (this) {
            rpc = next;
            remote = wire;
        }
        if (remote != null) {
            remote.setRpcDiscovery(next);
        }
    }

    /** The discovery extensions the writer announces. */
    public org.zerodds.rtps.RpcDiscovery rpcDiscovery() {
        return rpc;
    }

    /**
     * The GUIDs and discovery extensions of the matched readers, one entry per reader, those
     * of this process and those discovered by SEDP (DDS-RPC 1.0 §7.6.2.1.2).
     */
    public List<org.zerodds.rtps.RpcPeer> getMatchedRpcPeers() {
        List<org.zerodds.rtps.RpcPeer> out = new ArrayList<>();
        for (InProcessBus.Endpoint reader : matchedReaders) {
            out.add(new org.zerodds.rtps.RpcPeer(reader.endpointGuid(), reader.rpcDiscovery()));
        }
        org.zerodds.rtps.RtpsParticipant.WireWriter remote = wire;
        if (remote != null) {
            for (org.zerodds.rtps.DiscoveryData.Endpoint endpoint : remote.matchedEndpoints()) {
                out.add(new org.zerodds.rtps.RpcPeer(endpoint.endpointGuid, endpoint.rpc));
            }
        }
        return out;
    }

    /** The identity the next sample of this writer will be sent under. */
    public org.zerodds.internal.SourceIdentity nextIdentity() {
        org.zerodds.rtps.RtpsParticipant.WireWriter remote = wire;
        synchronized (this) {
            long next = remote != null ? remote.nextSequence() : identitySequence + 1;
            return new org.zerodds.internal.SourceIdentity(identityGuid(), next);
        }
    }

    /** The GUID this writer's samples carry: its RTPS GUID, or its instance handle when it has none. */
    private byte[] identityGuid() {
        org.zerodds.rtps.RtpsParticipant.WireWriter remote = wire;
        if (remote != null) {
            return remote.guid();
        }
        return org.zerodds.core.InstanceHandleImpl.bytesOf(getInstanceHandle()).clone();
    }

    /**
     * DDS-DCPS 1.4 §2.2.2.4.2.11 write. The writer's state is updated under its
     * monitor; the delivery — which raises the readers' listener callbacks — runs
     * without it, so a callback never runs with the writer's monitor held
     * (DDS-Java-PSM 1.0 §7.2.4).
     */
    private org.zerodds.internal.SourceIdentity writeAt(T data, long ts,
            org.zerodds.internal.SourceIdentity related) throws TimeoutException {
        return writeAt(data, ts, related, null);
    }

    private org.zerodds.internal.SourceIdentity writeAt(T data, long ts,
            org.zerodds.internal.SourceIdentity related, String instance)
            throws TimeoutException {
        org.zerodds.core.Args.require(data, "data");
        return writeEncodedAt(() -> serialize(data), () -> keyOf(data), ts, related, instance);
    }

    /**
     * Writes octets that are already the serialized sample, as {@link #writeIdentified(Object,
     * org.zerodds.internal.SourceIdentity, String)} writes the octets the type support makes of
     * a value. For a writer whose topic type has no key: the DDS-RPC 1.0 binding (§7.11.2)
     * writes the requests and replies it frames itself through the writer it hands to the
     * application typed by the topic type.
     *
     * @throws IllegalStateException when the topic type has a key
     */
    public org.zerodds.internal.SourceIdentity writeSerializedIdentified(byte[] payload,
            org.zerodds.internal.SourceIdentity related, String instance) {
        org.zerodds.core.Args.require(payload, "payload");
        if (typeSupport.isKeyed()) {
            throw new IllegalStateException(
                    "the octets of a keyed sample need its key hash; write the value");
        }
        try {
            return writeEncodedAt(payload::clone, () -> new InstanceKey(new byte[0]),
                    System.currentTimeMillis(), related, instance);
        } catch (TimeoutException e) {
            throw new org.zerodds.core.OutOfResourcesException(e.getMessage());
        }
    }

    private org.zerodds.internal.SourceIdentity writeEncodedAt(
            java.util.function.Supplier<byte[]> encode,
            java.util.function.Supplier<InstanceKey> keyed, long ts,
            org.zerodds.internal.SourceIdentity related, String instance)
            throws TimeoutException {
        byte[] payload;
        InstanceKey key;
        awaitWireHistorySpace();
        synchronized (this) {
            if (closed) {
                throw new AlreadyClosedException("DataWriter is closed");
            }
            if (!enabled) {
                throw new NotEnabledException("DataWriter is not enabled");
            }
            payload = encode.get();
            key = keyed.get();
            awaitResources(key);
            if (closed) {
                // the writer was closed while this write waited for a resource
                throw new AlreadyClosedException("DataWriter is closed");
            }
            instances.putIfAbsent(key, InstanceKey.toHandle(key));
            lastWriteMillis.put(key, ts);
            lastPayload.put(key, payload);
            retainForDurability(key, payload, ts);
            // Writing asserts liveliness (DDS 1.2 §7.1.2.4.2): renew the lease and
            // clear any previously-lost state.
            lastLivelinessMillis = ts;
            alive = true;
        }
        assertForParticipant();
        long sequence;
        org.zerodds.rtps.RtpsParticipant.WireWriter remote = wire;
        if (remote != null) {
            // cross-process RTPS/UDP delivery; a sample of a coherent set says which one
            // (DDSI-RTPS 2.5 8.7.6); the readers of one service instance only if it is named
            long set = parent instanceof PublisherImpl ? ((PublisherImpl) parent).coherentSetName() : 0;
            if (parent instanceof PublisherImpl && ((PublisherImpl) parent).groupScope()) {
                final long coherentSet = set;
                // the group number is assigned and the sample entered in the history under one lock
                sequence = ((PublisherImpl) parent).stampAndWrite(
                        group -> remote.write(payload, related, coherentSet, group, instance));
            } else {
                sequence = remote.write(payload, related, set, null, instance);
            }
        } else {
            synchronized (this) {
                sequence = ++identitySequence;
            }
        }
        org.zerodds.internal.SourceIdentity identity =
                new org.zerodds.internal.SourceIdentity(identityGuid(), sequence);
        publish(message(payload, key.bytes(), InProcessBus.ChangeKind.WRITE, ts)
                .withIdentity(identity, related), instance);
        return identity;
    }

    /**
     * The coherent set {@code name} of the publisher has ended: if this writer sent samples of it
     * over RTPS, the readers are told (a DATA without serialized data, DDSI-RTPS 2.5 8.7.6).
     */
    void endCoherentSet(long name, org.zerodds.rtps.RtpsParticipant.GroupStamp group) {
        org.zerodds.rtps.RtpsParticipant.WireWriter remote = wire;
        if (remote != null) {
            if (group != null) {
                remote.endGroupCoherentSet(group); // every writer of the publisher ends the group set
            } else {
                remote.endCoherentSet(name);
            }
        }
    }

    /** The entity id of this writer on the wire, or {@code null} if it is not on the wire. */
    byte[] wireEntityId() {
        org.zerodds.rtps.RtpsParticipant.WireWriter remote = wire;
        return remote == null ? null : java.util.Arrays.copyOfRange(remote.guid(), 12, 16);
    }

    /** Spec §2.2.2.4.2.13 dispose — instance -> NOT_ALIVE_DISPOSED. */
    public void dispose(T instanceData) {
        disposeAt(instanceData, System.currentTimeMillis());
    }

    /** Spec §2.2.2.4.2.14 dispose_w_timestamp. */
    public void dispose(T instanceData, long sourceTimestampMillis) {
        disposeAt(instanceData, sourceTimestampMillis);
    }

    private void disposeAt(T instanceData, long ts) {
        org.zerodds.core.Args.require(instanceData, "instanceData");
        InstanceKey key;
        synchronized (this) {
            if (closed) {
                throw new AlreadyClosedException("DataWriter is closed");
            }
            if (!enabled) {
                throw new NotEnabledException("DataWriter is not enabled");
            }
            key = keyOf(instanceData);
        }
        disposeKey(key, ts);
    }

    /**
     * RESOURCE_LIMITS of the writer (DDS 1.4 §2.2.3.19, §2.2.3.14): a write that would exceed
     * {@code max_instances} (instances not yet unregistered), or, for the history this writer
     * keeps for TRANSIENT_LOCAL late joiners, {@code max_samples} or
     * {@code max_samples_per_instance}, waits for unregister or dispose to free the resource.
     * A RELIABLE writer waits up to {@code max_blocking_time}; a BEST_EFFORT writer does not wait.
     * Called with this writer's monitor held; waiting gives it up.
     *
     * @throws TimeoutException  if the resource is still missing after {@code max_blocking_time}.
     * @throws org.zerodds.core.OutOfResourcesException  for a BEST_EFFORT writer, which does
     *         not wait.
     */
    private void awaitResources(InstanceKey key) throws TimeoutException {
        ResourceLimits limits = qos.getResourceLimits();
        if (!exceedsLimits(key, limits)) {
            return;
        }
        Reliability reliability = qos.getReliability();
        if (reliability.getKind() != Reliability.Kind.RELIABLE) {
            throw new org.zerodds.core.OutOfResourcesException(
                    "the RESOURCE_LIMITS of the DataWriter are reached");
        }
        Duration blocking = reliability.getMaxBlockingTime();
        long deadline = blocking.isInfinite() ? Long.MAX_VALUE
                : System.nanoTime() + blocking.getDuration(TimeUnit.NANOSECONDS);
        while (exceedsLimits(key, qos.getResourceLimits())) {
            long remaining = deadline == Long.MAX_VALUE ? Long.MAX_VALUE : deadline - System.nanoTime();
            if (remaining <= 0) {
                throw new TimeoutException("the RESOURCE_LIMITS of the DataWriter are reached and "
                        + "max_blocking_time has elapsed");
            }
            try {
                if (remaining == Long.MAX_VALUE) {
                    wait();
                } else {
                    TimeUnit.NANOSECONDS.timedWait(this, remaining);
                }
            } catch (InterruptedException e) {
                Thread.currentThread().interrupt();
                throw new TimeoutException("interrupted while waiting for RESOURCE_LIMITS");
            }
        }
    }

    /**
     * A RELIABLE writer with KEEP_ALL history and a max_samples limit keeps the samples the readers
     * of other processes have not acknowledged (DDS 1.4 2.2.3.19, 2.2.3.14): a write that would
     * exceed the limit waits for acknowledgments up to max_blocking_time. Without the monitor, so
     * the ACKNACK that frees the room is not held up by this writer.
     */
    private void awaitWireHistorySpace() throws TimeoutException {
        org.zerodds.rtps.RtpsParticipant.WireWriter remote = wire;
        if (remote == null || qos.getReliability().getKind() != Reliability.Kind.RELIABLE
                || qos.getHistory().getKind() != History.Kind.KEEP_ALL) {
            return;
        }
        int max = qos.getResourceLimits().getMaxSamples();
        if (max == ResourceLimits.LENGTH_UNLIMITED) {
            return;
        }
        Duration blocking = qos.getReliability().getMaxBlockingTime();
        long nanos = blocking.isInfinite() ? Long.MAX_VALUE : blocking.getDuration(TimeUnit.NANOSECONDS);
        try {
            if (!remote.awaitHistorySpace(max, nanos)) {
                throw new TimeoutException("the RESOURCE_LIMITS of the DataWriter are reached by "
                        + "samples the readers did not acknowledge and max_blocking_time has elapsed");
            }
        } catch (InterruptedException interrupted) {
            Thread.currentThread().interrupt();
            throw new TimeoutException("interrupted while waiting for acknowledgments");
        }
    }

    private boolean exceedsLimits(InstanceKey key, ResourceLimits limits) {
        if (!instances.containsKey(key) && limits.getMaxInstances() != ResourceLimits.LENGTH_UNLIMITED
                && instances.size() >= limits.getMaxInstances()) {
            return true;
        }
        if (qos.getDurability().getKind() != Durability.Kind.TRANSIENT_LOCAL) {
            return false; // nothing but the instances is kept by this writer
        }
        Deque<Retained> own = retained.get(key);
        int ownSamples = own == null ? 0 : own.size();
        History h = qos.getHistory();
        if (h.getKind() == History.Kind.KEEP_LAST && ownSamples >= Math.max(1, h.getDepth())) {
            return false; // the new sample replaces the oldest of the instance
        }
        if (limits.getMaxSamplesPerInstance() != ResourceLimits.LENGTH_UNLIMITED
                && ownSamples >= limits.getMaxSamplesPerInstance()) {
            return true;
        }
        if (limits.getMaxSamples() != ResourceLimits.LENGTH_UNLIMITED) {
            int total = 0;
            for (Deque<Retained> samples : retained.values()) {
                total += samples.size();
            }
            return total >= limits.getMaxSamples();
        }
        return false;
    }

    /**
     * The instance is disposed: this writer no longer replays it, and the durability
     * service marks it disposed (DDS 1.4 §2.2.2.4.2.13).
     */
    private void disposeKey(InstanceKey key, long ts) {
        synchronized (this) {
            retained.remove(key); // disposed instance no longer replayed
            DurabilityService.instance().disposed(domainId, topic.getName(),
                    typeSupport.getTypeName(), key, qos.getDurabilityService());
            notifyAll(); // history samples are free again
        }
        publish(message(null, key.bytes(), InProcessBus.ChangeKind.DISPOSE, ts));
    }

    /**
     * The instance is unregistered by this writer. With WRITER_DATA_LIFECYCLE
     * {@code autodispose_unregistered_instances} true the instance is disposed first, as
     * if the application had called dispose before unregister (DDS 1.4 §2.2.3.21).
     */
    private void unregisterKey(InstanceKey key, long ts) {
        boolean dispose;
        synchronized (this) {
            dispose = qos.getWriterDataLifecycle().isAutDisposeUnregisteredInstances();
            instances.remove(key);
            retained.remove(key);
            if (dispose) {
                DurabilityService.instance().disposed(domainId, topic.getName(),
                        typeSupport.getTypeName(), key, qos.getDurabilityService());
            }
            DurabilityService.instance().unregistered(domainId, topic.getName(),
                    typeSupport.getTypeName(), key, writerId, qos.getDurabilityService());
            notifyAll(); // an instance and its samples are free again
        }
        if (dispose) {
            publish(message(null, key.bytes(), InProcessBus.ChangeKind.DISPOSE, ts));
        }
        publish(message(null, key.bytes(), InProcessBus.ChangeKind.UNREGISTER, ts));
    }

    /** Spec §2.2.2.4.2.7 unregister_instance — instance -> NOT_ALIVE_NO_WRITERS. */
    public void unregisterInstance(T instanceData) {
        unregisterAt(instanceData, System.currentTimeMillis());
    }

    /** Spec §2.2.2.4.2.8 unregister_instance_w_timestamp. */
    public void unregisterInstance(T instanceData, long sourceTimestampMillis) {
        unregisterAt(instanceData, sourceTimestampMillis);
    }

    private void unregisterAt(T instanceData, long ts) {
        org.zerodds.core.Args.require(instanceData, "instanceData");
        InstanceKey key;
        synchronized (this) {
            if (closed) {
                throw new AlreadyClosedException("DataWriter is closed");
            }
            if (!enabled) {
                throw new NotEnabledException("DataWriter is not enabled");
            }
            key = keyOf(instanceData);
        }
        unregisterKey(key, ts);
    }

    /** Spec §2.2.2.4.2.10 lookup_instance. */
    public synchronized InstanceHandle lookupInstance(T instanceData) {
        InstanceHandle h = instances.get(keyOf(instanceData));
        return h == null ? Values.nil() : h;
    }

    /**
     * Spec §2.2.2.4.2.22 assert_liveliness — renew this writer's liveliness, and with it
     * that of the writers of the DomainParticipant whose LIVELINESS is MANUAL_BY_PARTICIPANT
     * (DDS 1.4 §2.2.3.11).
     */
    public void assertLiveliness() {
        renewLiveliness();
        assertOnWire();
        assertForParticipant();
    }

    /** The readers of other processes are told that this writer asserted its liveliness (HEARTBEAT with the L flag). */
    public void assertOnWire() {
        org.zerodds.rtps.RtpsParticipant.WireWriter remote = wire;
        if (remote != null) {
            remote.assertLiveliness();
        }
    }

    /** This writer's own liveliness is renewed now. */
    public synchronized void renewLiveliness() {
        if (closed) {
            throw new AlreadyClosedException("DataWriter is closed");
        }
        alive = true;
        lastLivelinessMillis = System.currentTimeMillis();
    }

    /**
     * MANUAL_BY_PARTICIPANT (DDS 1.4 §2.2.3.11): as long as one entity of the DomainParticipant
     * asserts its liveliness, the writers of that participant with this kind are alive too.
     */
    private void assertForParticipant() {
        if (parent instanceof PublisherImpl
                && parent.getParent() instanceof org.zerodds.domain.DomainParticipantImpl) {
            ((org.zerodds.domain.DomainParticipantImpl) parent.getParent())
                    .renewManualByParticipant();
        }
    }

    // ---- helpers ---------------------------------------------------------

    /**
     * Retains a written sample according to its DURABILITY kind (DDS 1.2
     * §7.1.3.4). TRANSIENT_LOCAL stays in this writer and is replayed from
     * {@link #replayTo}; TRANSIENT and PERSISTENT go to the
     * {@link DurabilityService}, which survives this writer.
     */
    private void retainForDurability(InstanceKey key, byte[] payload, long ts) {
        Durability.Kind d = qos.getDurability().getKind();
        if (d == Durability.Kind.TRANSIENT_LOCAL) {
            retainCapped(key, payload, ts);
        } else if (d == Durability.Kind.TRANSIENT || d == Durability.Kind.PERSISTENT) {
            // DDS 1.4 §2.2.3.5: the DURABILITY_SERVICE policy gives the history and the
            // limits of the store, not the HISTORY of this writer.
            DurabilityService.instance().retain(domainId, topic.getName(),
                    typeSupport.getTypeName(), key, payload, d, writerId,
                    qos.getDurabilityService(), ts, lifespanMillis());
        }
    }

    private void retainCapped(InstanceKey key, byte[] payload, long ts) {
        Deque<Retained> dq = retained.computeIfAbsent(key, k -> new ArrayDeque<>());
        History h = qos.getHistory();
        if (h.getKind() == History.Kind.KEEP_LAST) {
            int depth = Math.max(1, h.getDepth());
            while (dq.size() >= depth) {
                dq.pollFirst();
            }
        }
        dq.addLast(new Retained(payload, ts, lifespanMillis()));
    }

    /** The LIFESPAN of this writer in milliseconds, or {@link InProcessBus.Message#NO_LIFESPAN}. */
    private long lifespanMillis() {
        Duration lifespan = qos.getLifespan().getDuration();
        return lifespan.isInfinite() ? InProcessBus.Message.NO_LIFESPAN
                : lifespan.getDuration(TimeUnit.MILLISECONDS);
    }

    private InProcessBus.Message message(byte[] payload, byte[] keyHash,
                                         InProcessBus.ChangeKind kind, long ts) {
        return new InProcessBus.Message(payload, keyHash, kind, writerId,
                qos.getOwnershipStrength().getValue(), exclusive(),
                new ArrayList<>(partitions), ts, getInstanceHandle())
                .withLifespan(lifespanMillis());
    }

    @Override
    public org.omg.dds.core.InstanceHandle writerHandle() {
        return getInstanceHandle();
    }

    /**
     * Hands the message to the readers, or to the publisher's coherent set when the publisher
     * is inside begin_coherent_changes and its PRESENTATION asks for coherent access: the set
     * is delivered, in order, by the outermost end_coherent_changes (DDS 1.4 2.2.2.4.1.10).
     */
    private void publish(InProcessBus.Message m) {
        publish(m, null);
    }

    private void publish(InProcessBus.Message m, String instance) {
        if (parent instanceof PublisherImpl
                && ((PublisherImpl) parent).deferCoherent(() -> deliver(m, instance))) {
            return;
        }
        deliver(m, instance);
    }

    private void deliver(InProcessBus.Message m, String instance) {
        InProcessBus.instance().publish(domainId, topic.getName(), typeSupport.getTypeName(), m,
                instance);
    }

    private InstanceKey keyOf(T data) {
        return new InstanceKey(typeSupport.isKeyed() ? typeSupport.keyHash(data) : new byte[0]);
    }

    /** The largest encoded sample a writer produces: 256 MiB. */
    public static final int MAX_SAMPLE_BYTES = 1 << 28;

    private byte[] serialize(T data) {
        return serialize(typeSupport, data, MAX_SAMPLE_BYTES);
    }

    /**
     * Encodes {@code data} with {@code support}, growing the buffer up to
     * {@code maxBytes}. An encoding that does not fit is a resource shortage
     * (RETCODE_OUT_OF_RESOURCES, DDS-Java-PSM 1.0 Table 7.1); an encoder that
     * rejects the sample for another reason fails with RETCODE_ERROR.
     */
    public static <S> byte[] serialize(TopicTypeSupport<S> support, S data, int maxBytes) {
        int capacity = 256;
        while (true) {
            ByteBuffer encoder = Xcdr2Codec.encoder(capacity);
            try {
                support.serialize(data, encoder);
            } catch (java.nio.BufferOverflowException overflow) {
                if (capacity >= maxBytes) {
                    throw new OutOfResourcesException("the encoding of the sample exceeds "
                            + maxBytes + " bytes", overflow);
                }
                capacity <<= 1;
                continue;
            } catch (org.zerodds.cdr.XcdrException rejected) {
                throw new ErrorException("the sample cannot be encoded: " + rejected.getMessage(),
                        rejected);
            }
            return Xcdr2Codec.copyToBytes(encoder);
        }
    }

    public Topic<T> getTopic() {
        return topic;
    }

    public DataWriterQos getQos() {
        return qos;
    }

    /**
     * DDS 1.4 §2.2.3: once the writer is enabled only the changeable policies can
     * change ({@link org.zerodds.core.qos.QosRules}); the leases follow a changed
     * DEADLINE or LIVELINESS.
     */
    public void setQos(DataWriterQos qos) {
        boolean announce;
        synchronized (this) {
            org.zerodds.core.qos.QosRules.checkChange(enabled, this.qos, qos);
            this.qos = qos;
            announce = enabled && !closed;
            if (announce) {
                if (deadlineLease != null) {
                    deadlineLease.cancel(false);
                    deadlineLease = null;
                }
                if (livelinessLease != null) {
                    livelinessLease.cancel(false);
                    livelinessLease = null;
                }
                startLeasesLocked();
            }
        }
        if (announce) {
            // The built-in topic data of the writer carries its QoS (DDS 1.4 §2.2.5): the
            // changed QoS is a new sample of the same instance. Outside the monitor, because
            // the built-in readers it reaches run listeners.
            announceQos();
        }
    }

    /** The QoS of this writer is written to the "DCPSPublication" built-in topic again. */
    public void announceQos() {
        if (parent != null && parent.getParent() != null) {
            org.zerodds.domain.Discovery.writerCreated(parent.getParent(), this, parent);
        }
        // the readers of other processes learn the changed QoS from SEDP
        org.zerodds.rtps.RtpsParticipant.WireWriter remote = wire;
        if (remote != null && wireQos() != null) {
            remote.updateQos(wireQos());
        }
    }

    /**
     * DDS-Java-PSM 1.0 §7.2.4 — the state transition runs under this writer's
     * monitor, the bus registration (which raises the matched readers'
     * callbacks) runs without it.
     */
    @Override
    public void enable() {
        synchronized (this) {
            if (closed) {
                throw new AlreadyClosedException("DataWriter is closed");
            }
            if (enabled) {
                return; // enable is idempotent (§7.3.6)
            }
            // DDS 1.4 §2.2.2.1.1.7: enable on an Entity whose factory is not enabled fails.
            if (parent instanceof PublisherImpl && !((PublisherImpl) parent).isEnabled()) {
                throw new PreconditionNotMetException("the Publisher of the DataWriter is not enabled");
            }
            if (org.zerodds.rtps.DdsWireBridge.enabled()
                    && !org.zerodds.topic.BuiltinTopics.isInternalToProcess(topic.getName())) {
                wire = org.zerodds.rtps.DdsWireBridge.typedWriter(domainId, topic.getName(),
                        typeSupport, wireQos(), this::acceptsRemoteReader);
                if (wire != null && parent instanceof PublisherImpl) {
                    final PublisherImpl publisher = (PublisherImpl) parent;
                    // the HEARTBEATs of a writer of a GROUP Publisher carry the group sequence numbers
                    wire.setGroupInfo(() -> publisher.groupScope() ? publisher.currentGroupStamp() : null);
                }
                if (!rpc.isEmpty()) {
                    wire.setRpcDiscovery(rpc);
                }
            }
            enabled = true;
            startLeasesLocked();
        }
        InProcessBus.instance().registerWriter(
                domainId, topic.getName(), typeSupport.getTypeName(), this);
        if (parent instanceof PublisherImpl) {
            ((PublisherImpl) parent).writerEnabled(this);
        }
    }

    public boolean isEnabled() {
        return enabled;
    }

    /**
     * Schedules the deadline and liveliness leases (DDS 1.2 §7.1.4.2). An
     * infinite deadline or an AUTOMATIC/infinite liveliness lease needs no
     * timer.
     */
    private synchronized void startLeasesLocked() {
        Duration deadlinePeriod = qos.getDeadline().getPeriod();
        if (!deadlinePeriod.isInfinite()
                && deadlinePeriod.getDuration(TimeUnit.MILLISECONDS) > 0) {
            deadlineLease = LeaseMonitor.scheduleAtFixedRate(
                    () -> evaluateOfferedDeadline(true),
                    deadlinePeriod.getDuration(TimeUnit.MILLISECONDS));
        }
        Liveliness l = qos.getLiveliness();
        if (l.getKind() != Liveliness.Kind.AUTOMATIC && !l.getLeaseDuration().isInfinite()
                && l.getLeaseDuration().getDuration(TimeUnit.MILLISECONDS) > 0) {
            livelinessLease = LeaseMonitor.scheduleAtFixedRate(
                    () -> evaluateLiveliness(true),
                    l.getLeaseDuration().getDuration(TimeUnit.MILLISECONDS));
        }
    }

    @Override
    public void close() {
        boolean unregister;
        List<InstanceKey> orphaned;
        synchronized (this) {
            if (closed) {
                return;
            }
            closed = true;
            unregister = enabled;
            if (deadlineLease != null) {
                deadlineLease.cancel(false);
                deadlineLease = null;
            }
            if (livelinessLease != null) {
                livelinessLease.cancel(false);
                livelinessLease = null;
            }
            orphaned = unregister ? new ArrayList<>(instances.keySet()) : new ArrayList<>();
        }
        // Delivery runs without the writer's monitor: a reader's listener
        // callback must not execute while this writer holds it (§7.2.4).
        long ts = System.currentTimeMillis();
        for (InstanceKey key : orphaned) {
            unregisterKey(key, ts);
        }
        if (unregister) {
            InProcessBus.instance().unregisterWriter(
                    domainId, topic.getName(), typeSupport.getTypeName(), this);
        }
        runCloseHooks();
    }

    public boolean isClosed() {
        return closed;
    }

    // ---- Interface operations (§7.6.2) -----------------------------------

    @SuppressWarnings("unchecked")
    @Override
    public <OTHER> DataWriter<OTHER> cast() {
        return (DataWriter<OTHER>) this;
    }

    @Override
    public void waitForAcknowledgments(Duration maxWait) throws TimeoutException {
        if (maxWait == null) {
            throw new IllegalArgumentException("maxWait must not be null");
        }
        awaitAcknowledgments(maxWait.isInfinite() ? Long.MAX_VALUE
                : maxWait.getDuration(TimeUnit.NANOSECONDS));
    }

    @Override
    public void waitForAcknowledgments(long maxWait, TimeUnit unit) throws TimeoutException {
        if (maxWait < 0) {
            throw new IllegalArgumentException("maxWait must not be negative");
        }
        awaitAcknowledgments(unit.toNanos(maxWait));
    }

    /**
     * DDS 1.4 2.2.2.4.2.12: blocks until every sample written has been acknowledged by the
     * matched reliable readers, or {@code nanos} have passed (TIMEOUT). The in-process bus
     * delivers synchronously, so a reader of this process has the sample when write returns;
     * the readers of other processes acknowledge over RTPS, which the wire writer tracks
     * (ACKNACK, DDSI-RTPS 2.5 8.4.7).
     */
    void awaitAcknowledgments(long nanos) throws TimeoutException {
        if (closed) {
            throw new AlreadyClosedException("DataWriter is closed");
        }
        if (!enabled) {
            throw new NotEnabledException("DataWriter is not enabled");
        }
        org.zerodds.rtps.RtpsParticipant.WireWriter remote = wire;
        if (remote == null) {
            return;
        }
        try {
            if (!remote.waitForAcknowledgments(nanos)) {
                throw new TimeoutException("not every sample was acknowledged in time");
            }
        } catch (InterruptedException interrupted) {
            Thread.currentThread().interrupt();
            throw new TimeoutException("interrupted while waiting for acknowledgments");
        }
    }

    @SuppressWarnings("unchecked")
    @Override
    public Set<InstanceHandle> getMatchedSubscriptions() {
        Set<InstanceHandle> out = new LinkedHashSet<>();
        for (InProcessBus.Endpoint reader : matchedReaders) {
            if (reader.instanceHandle() != null) {
                out.add(reader.instanceHandle());
            }
        }
        org.zerodds.rtps.RtpsParticipant.WireWriter remote = wire;
        if (remote != null) {
            for (org.zerodds.rtps.DiscoveryData.Endpoint endpoint : remote.matchedEndpoints()) {
                out.add(org.zerodds.core.Values.of(env, endpoint.endpointGuid));
            }
        }
        return out;
    }

    /**
     * DDS 1.4 §2.2.2.4.2.23: the built-in topic data of a matched reader,
     * with the QoS it requested. A reader of this process is described by the
     * reader itself, a remote one by what its SEDP announcement carried
     * (DDSI-RTPS 2.5 §9.6.2).
     */
    @Override
    public SubscriptionBuiltinTopicData getMatchedSubscriptionData(
            InstanceHandle subscriptionHandle) {
        if (subscriptionHandle == null) {
            throw new IllegalArgumentException("subscriptionHandle must not be null");
        }
        for (InProcessBus.Endpoint reader : matchedReaders) {
            if (subscriptionHandle.equals(reader.instanceHandle())
                    && reader instanceof org.zerodds.sub.DataReaderImpl) {
                org.zerodds.sub.DataReaderImpl<?> local = (org.zerodds.sub.DataReaderImpl<?>) reader;
                org.omg.dds.sub.Subscriber subscriber = local.getParent();
                org.omg.dds.domain.DomainParticipant owner =
                        subscriber == null ? null : subscriber.getParent();
                return org.zerodds.topic.BuiltinData.subscription(env, local, subscriber,
                        owner == null ? new org.zerodds.topic.BuiltinTopicKeyImpl(env, new int[4])
                                : org.zerodds.topic.BuiltinTopicKeyImpl.of(env,
                                        owner.getInstanceHandle()));
            }
        }
        org.zerodds.rtps.RtpsParticipant.WireWriter remote = wire;
        if (remote != null) {
            for (org.zerodds.rtps.DiscoveryData.Endpoint endpoint : remote.matchedEndpoints()) {
                if (subscriptionHandle.equals(org.zerodds.core.Values.of(env, endpoint.endpointGuid))) {
                    return org.zerodds.topic.BuiltinData.subscription(env, endpoint);
                }
            }
        }
        throw new IllegalArgumentException(subscriptionHandle
                + " is not a subscription matched with this DataWriter");
    }

    /** The type support of the writer's topic. */
    public TopicTypeSupport<T> typeSupport() {
        return typeSupport;
    }

    /** The QoS this writer announces on SEDP: its publication data (DDS 1.4 §2.2.5). */
    private org.zerodds.rtps.QosParameters wireQos() {
        if (parent == null) {
            return null;
        }
        org.omg.dds.domain.DomainParticipant owner = parent.getParent();
        org.omg.dds.topic.BuiltinTopicKey participantKey = owner == null
                ? new org.zerodds.topic.BuiltinTopicKeyImpl(env, new int[4])
                : org.zerodds.topic.BuiltinTopicKeyImpl.of(env, owner.getInstanceHandle());
        return org.zerodds.topic.BuiltinQos.of(
                org.zerodds.topic.BuiltinData.publication(env, this, parent, participantKey));
    }

    /**
     * Spec §2.2.2.4.2.9 get_key_value: the key fields of the instance the handle names. The
     * writer returns a copy of the last sample it wrote for the instance, so the key fields
     * are set (the other fields are those of that sample).
     */
    @Override
    public T getKeyValue(T keyHolder, InstanceHandle handle) {
        T source = getKeyValue(handle);
        if (keyHolder == null) {
            return source;
        }
        // only the key fields of the holder are overwritten (DDS 1.4 §2.2.2.5.3.29)
        return typeSupport.keyValue(source, keyHolder);
    }

    @Override
    public T getKeyValue(InstanceHandle handle) {
        if (!typeSupport.isKeyed()) {
            throw new PreconditionNotMetException(
                    "the topic type " + typeSupport.getTypeName() + " has no key");
        }
        if (handle == null) {
            throw new IllegalArgumentException("the instance handle must not be null");
        }
        synchronized (this) {
            for (Map.Entry<InstanceKey, InstanceHandle> e : instances.entrySet()) {
                if (e.getValue().equals(handle)) {
                    byte[] last = lastPayload.get(e.getKey());
                    if (last == null) {
                        throw new PreconditionNotMetException(
                                "no sample was written for the registered instance " + handle);
                    }
                    return typeSupport.deserialize(Xcdr2Codec.decoder(last));
                }
            }
        }
        throw new IllegalArgumentException(handle + " is not an instance of this DataWriter");
    }

    @Override
    public ModifiableInstanceHandle lookupInstance(ModifiableInstanceHandle handle,
            T keyHolder) {
        handle.copyFrom(lookupInstance(keyHolder));
        return handle;
    }

    @SuppressWarnings("unchecked")
    @Override
    public StatusCondition<DataWriter<T>> getStatusCondition() {
        return (StatusCondition<DataWriter<T>>) super.getStatusCondition();
    }

    /**
     * The instance a handle names (DDS 1.4 §2.2.2.4.2.7, §2.2.2.4.2.13): a handle this writer
     * registered; with the sample, the nil handle names the instance of the sample. A handle of
     * an instance the sample is not of is PRECONDITION_NOT_MET, a handle of no instance of this
     * writer is BAD_PARAMETER, and so is neither handle nor sample.
     */
    private synchronized InstanceKey resolve(InstanceHandle handle, T data) {
        boolean nil = handle == null || handle.isNil();
        if (nil) {
            if (data == null) {
                throw new IllegalArgumentException("an instance handle or a sample is needed");
            }
            InstanceKey byData = keyOf(data);
            if (!instances.containsKey(byData)) {
                throw new org.zerodds.core.PreconditionNotMetException(
                        "the instance of the sample is not registered with this DataWriter");
            }
            return byData;
        }
        for (Map.Entry<InstanceKey, InstanceHandle> e : instances.entrySet()) {
            if (e.getValue().equals(handle)) {
                if (data != null && !keyOf(data).equals(e.getKey())) {
                    throw new org.zerodds.core.PreconditionNotMetException(
                            "the sample is not of the instance the handle names");
                }
                return e.getKey();
            }
        }
        throw new IllegalArgumentException(
                "the handle " + handle + " is not an instance registered with this DataWriter");
    }

    /**
     * A handle given with a sample to write has to be the handle of the sample's instance
     * (DDS 1.4 §2.2.2.4.2.11): the handle of another instance of this writer is
     * PRECONDITION_NOT_MET, a handle of no instance of this writer is BAD_PARAMETER.
     */
    private void checkHandle(T data, InstanceHandle handle) {
        if (handle == null || handle.isNil()) {
            return;
        }
        if (InstanceKey.toHandle(keyOf(data)).equals(handle)) {
            return;
        }
        synchronized (this) {
            for (InstanceHandle known : instances.values()) {
                if (known.equals(handle)) {
                    throw new org.zerodds.core.PreconditionNotMetException(
                            "the handle is not the handle of the instance of the sample");
                }
            }
        }
        throw new IllegalArgumentException(
                "the handle " + handle + " is not an instance of this DataWriter");
    }

    /** Spec §2.2.2.4.2.14 dispose_w_timestamp — by instance handle. */
    private void disposeAtHandle(InstanceHandle handle, T data, long sourceTimestampMillis) {
        if (closed) {
            throw new AlreadyClosedException("DataWriter is closed");
        }
        if (!enabled) {
            throw new NotEnabledException("DataWriter is not enabled");
        }
        InstanceKey k = resolve(handle, data);
        disposeKey(k, sourceTimestampMillis);
    }


    // ---- dispose by handle (DDS-DCPS 1.4 §2.2.2.4.2.13) ---------------------

    /** Spec §2.2.2.4.2.13 dispose by instance handle. */
    @Override
    public void dispose(InstanceHandle instanceHandle) {
        disposeAtHandle(instanceHandle, null, System.currentTimeMillis());
    }

    @Override
    public void dispose(InstanceHandle instanceHandle, T instanceData) {
        disposeAtHandle(instanceHandle, instanceData, System.currentTimeMillis());
    }

    @Override
    public void dispose(InstanceHandle instanceHandle, T instanceData,
            org.omg.dds.core.Time sourceTimestamp) {
        org.zerodds.core.Args.require(sourceTimestamp, "sourceTimestamp");
        disposeAtHandle(instanceHandle, instanceData, sourceTimestamp.getTime(TimeUnit.MILLISECONDS));
    }

    @Override
    public void dispose(InstanceHandle instanceHandle, T instanceData,
            long sourceTimestamp, TimeUnit unit) {
        org.zerodds.core.Args.require(unit, "unit");
        disposeAtHandle(instanceHandle, instanceData, unit.toMillis(sourceTimestamp));
    }

    /** Spec §2.2.2.4.2.11 write with a source timestamp. */
    @Override
    public void write(T data, org.omg.dds.core.Time sourceTimestamp) throws TimeoutException {
        org.zerodds.core.Args.require(data, "data");
        org.zerodds.core.Args.require(sourceTimestamp, "sourceTimestamp");
        writeAt(data, sourceTimestamp.getTime(TimeUnit.MILLISECONDS), null);
    }

    /** write with a handle, the source timestamp of §2.2.2.4.2.10. */
    @Override
    public void write(T data, InstanceHandle instanceHandle) throws TimeoutException {
        org.zerodds.core.Args.require(data, "data");
        checkHandle(data, instanceHandle);
        write(data, System.currentTimeMillis());
    }

    @Override
    public void write(T data, InstanceHandle instanceHandle,
            org.omg.dds.core.Time sourceTimestamp) throws TimeoutException {
        org.zerodds.core.Args.require(data, "data");
        org.zerodds.core.Args.require(sourceTimestamp, "sourceTimestamp");
        checkHandle(data, instanceHandle);
        writeAt(data, sourceTimestamp.getTime(TimeUnit.MILLISECONDS), null);
    }

    @Override
    public void write(T data, InstanceHandle instanceHandle,
            long sourceTimestamp, TimeUnit unit) throws TimeoutException {
        org.zerodds.core.Args.require(data, "data");
        org.zerodds.core.Args.require(unit, "unit");
        checkHandle(data, instanceHandle);
        writeAt(data, unit.toMillis(sourceTimestamp), null);
    }

    /** register/unregister with an explicit handle (§2.2.2.4.2.13/.15). */
    public synchronized InstanceHandle registerInstance(T instanceData) {
        org.zerodds.core.Args.require(instanceData, "instanceData");
        InstanceKey key = keyOf(instanceData);
        return instances.computeIfAbsent(key, InstanceKey::toHandle);
    }

    /** register_instance with a source timestamp in milliseconds. */
    public InstanceHandle registerInstance(T instanceData, long sourceTimestampMillis) {
        org.zerodds.core.Args.require(instanceData, "instanceData");
        return registerInstanceAt(instanceData, sourceTimestampMillis);
    }

    /** write with a source timestamp given in milliseconds (§2.2.2.4.2.10). */
    public void write(T data, long sourceTimestampMillis) throws TimeoutException {
        org.zerodds.core.Args.require(data, "data");
        writeAt(data, sourceTimestampMillis, null);
    }

    /** write with a source timestamp in an arbitrary unit (§2.2.2.4.2.10). */
    @Override
    public void write(T data, long sourceTimestamp, TimeUnit unit) throws TimeoutException {
        org.zerodds.core.Args.require(data, "data");
        org.zerodds.core.Args.require(unit, "unit");
        writeAt(data, unit.toMillis(sourceTimestamp), null);
    }

    @Override
    public InstanceHandle registerInstance(T instanceData,
            org.omg.dds.core.Time sourceTimestamp) {
        org.zerodds.core.Args.require(instanceData, "instanceData");
        org.zerodds.core.Args.require(sourceTimestamp, "sourceTimestamp");
        return registerInstanceAt(instanceData,
                sourceTimestamp.getTime(TimeUnit.MILLISECONDS));
    }

    @Override
    public InstanceHandle registerInstance(T instanceData,
            long sourceTimestamp, TimeUnit unit) {
        org.zerodds.core.Args.require(instanceData, "instanceData");
        org.zerodds.core.Args.require(unit, "unit");
        return registerInstanceAt(instanceData, unit.toMillis(sourceTimestamp));
    }

    private InstanceHandle registerInstanceAt(T instanceData, long timestamp) {
        if (closed) {
            throw new AlreadyClosedException("DataWriter is closed");
        }
        if (!enabled) {
            throw new NotEnabledException("DataWriter is not enabled");
        }
        lastLivelinessMillis = Math.max(lastLivelinessMillis, timestamp);
        synchronized (this) {
            InstanceKey key = keyOf(instanceData);
            return instances.computeIfAbsent(key, InstanceKey::toHandle);
        }
    }

    /** Spec §2.2.2.4.2.7 unregister_instance — by instance handle. */
    @Override
    public void unregisterInstance(InstanceHandle handle) {
        unregisterAtHandle(handle, null, System.currentTimeMillis());
    }

    @Override
    public void unregisterInstance(InstanceHandle handle, T instanceData) {
        unregisterAtHandle(handle, instanceData, System.currentTimeMillis());
    }

    @Override
    public void unregisterInstance(InstanceHandle handle, T instanceData,
            org.omg.dds.core.Time sourceTimestamp) {
        org.zerodds.core.Args.require(sourceTimestamp, "sourceTimestamp");
        unregisterAtHandle(handle, instanceData, sourceTimestamp.getTime(TimeUnit.MILLISECONDS));
    }

    @Override
    public void unregisterInstance(InstanceHandle handle, T instanceData,
            long sourceTimestamp, TimeUnit unit) {
        org.zerodds.core.Args.require(unit, "unit");
        unregisterAtHandle(handle, instanceData, unit.toMillis(sourceTimestamp));
    }

    private void unregisterAtHandle(InstanceHandle handle, T data, long timestamp) {
        if (closed) {
            throw new AlreadyClosedException("DataWriter is closed");
        }
        if (!enabled) {
            throw new NotEnabledException("DataWriter is not enabled");
        }
        InstanceKey k = resolve(handle, data);
        unregisterKey(k, timestamp);
    }
}
