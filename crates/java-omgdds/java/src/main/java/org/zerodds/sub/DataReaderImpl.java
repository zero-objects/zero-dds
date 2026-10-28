// SPDX-License-Identifier: Apache-2.0
package org.zerodds.sub;

import org.omg.dds.core.Duration;
import org.omg.dds.core.DomainEntity;
import org.omg.dds.core.ModifiableInstanceHandle;
import org.omg.dds.core.StatusCondition;
import org.omg.dds.sub.DataReader;
import org.omg.dds.sub.DataReaderListener;
import org.omg.dds.sub.DataReaderQos;
import org.omg.dds.sub.InstanceState;
import org.omg.dds.sub.QueryCondition;
import org.omg.dds.sub.ReadCondition;
import org.omg.dds.sub.Sample;
import org.omg.dds.sub.SampleState;
import org.omg.dds.sub.Subscriber;
import org.omg.dds.sub.ViewState;
import org.omg.dds.topic.PublicationBuiltinTopicData;
import org.omg.dds.topic.TopicDescription;
import org.omg.dds.core.InstanceHandle;
import org.omg.dds.core.QosProvider;
import org.omg.dds.core.ServiceEnvironment;
import org.omg.dds.core.policy.Durability;
import org.omg.dds.core.policy.History;
import org.omg.dds.core.policy.Ownership;
import org.omg.dds.core.policy.QosPolicy;
import org.omg.dds.core.status.SampleLostStatus;
import org.omg.dds.core.status.SampleRejectedStatus;
import org.omg.dds.core.policy.QosPolicyCount;
import org.omg.dds.core.status.DataAvailableStatus;
import org.omg.dds.core.status.LivelinessChangedStatus;
import org.omg.dds.core.status.RequestedDeadlineMissedStatus;
import org.omg.dds.core.status.RequestedIncompatibleQosStatus;
import org.omg.dds.core.status.SubscriptionMatchedStatus;
import org.omg.dds.core.status.Status;
import org.omg.dds.topic.ContentFilteredTopic;
import org.omg.dds.topic.Topic;
import org.zerodds.core.AlreadyClosedException;
import org.zerodds.core.EntityBase;
import org.zerodds.core.NotEnabledException;
import org.zerodds.core.PreconditionNotMetException;
import org.zerodds.core.Values;
import org.zerodds.core.condition.QueryConditionImpl;
import org.zerodds.core.condition.ReadConditionImpl;
import org.zerodds.core.event.DataAvailableEventImpl;
import org.zerodds.core.event.LivelinessChangedEventImpl;
import org.zerodds.core.event.RequestedDeadlineMissedEventImpl;
import org.zerodds.core.event.RequestedIncompatibleQosEventImpl;
import org.zerodds.core.event.SampleLostEventImpl;
import org.zerodds.core.event.SampleRejectedEventImpl;
import org.zerodds.core.event.SubscriptionMatchedEventImpl;
import org.zerodds.core.qos.QosCompatibility;
import org.zerodds.core.status.DataAvailableStatusImpl;
import org.zerodds.core.status.LivelinessChangedStatusImpl;
import org.zerodds.core.status.SampleLostStatusImpl;
import org.zerodds.core.status.SampleRejectedStatusImpl;
import org.zerodds.core.status.QosPolicyCountImpl;
import org.zerodds.core.status.RequestedDeadlineMissedStatusImpl;
import org.zerodds.core.status.RequestedIncompatibleQosStatusImpl;
import org.zerodds.core.status.SubscriptionMatchedStatusImpl;
import org.zerodds.cdr.TopicTypeSupport;
import org.zerodds.internal.DurabilityService;
import org.zerodds.internal.InProcessBus;
import org.zerodds.internal.InstanceKey;
import org.zerodds.internal.LeaseMonitor;
import org.zerodds.internal.TimeBasedFilterState;
import org.zerodds.internal.Xcdr2Codec;
import org.zerodds.sub.SampleImpl;

import java.nio.ByteBuffer;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.UUID;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.TimeoutException;
import java.util.function.Predicate;

/**
 * OMG DDS Java-PSM DataReader — DDS-DCPS 1.4 §2.2.2.5.3.
 *
 * <p>Enforces the per-instance QoS surface reader-side: HISTORY keep-last
 * depth (per instance), EXCLUSIVE OWNERSHIP arbitration (§2.2.3.12.1),
 * PARTITION matching (§2.2.3.13), CONTENT-FILTERED-TOPIC filtering
 * (§2.2.2.3.3), TRANSIENT_LOCAL late-join replay (§2.2.3.4), instance_state
 * lifecycle (ALIVE / NOT_ALIVE_DISPOSED / NOT_ALIVE_NO_WRITERS,
 * §2.2.2.5.1.3),
 * plus the REQUESTED_DEADLINE_MISSED and LIVELINESS_CHANGED statuses
 * (§2.2.4.1).
 */
public final class DataReaderImpl<T> extends EntityBase<DataReaderListener<T>, DataReaderQos>
        implements DataReader<T>, DomainEntity<DataReaderListener<T>, DataReaderQos>,
        InProcessBus.Endpoint {

    /** The communication statuses of a DataReader (DDS 1.2 §7.1.4.1). */
    private static final Class<? extends Status> REQUESTED_DEADLINE_MISSED =
            RequestedDeadlineMissedStatus.class;
    private static final Class<? extends Status> REQUESTED_INCOMPATIBLE =
            RequestedIncompatibleQosStatus.class;
    private static final Class<? extends Status> SUBSCRIPTION_MATCHED =
            SubscriptionMatchedStatus.class;
    private static final Class<? extends Status> LIVELINESS_CHANGED =
            LivelinessChangedStatus.class;
    private static final Class<? extends Status> SAMPLE_REJECTED =
            SampleRejectedStatus.class;
    private static final Class<? extends Status> SAMPLE_LOST =
            SampleLostStatus.class;
    private static final Class<? extends Status> DATA_AVAILABLE =
            DataAvailableStatus.class;

    private final ServiceEnvironment env = env();
    private final int domainId;
    private final Subscriber parent;
    private final Topic<T> topic;
    private final TopicTypeSupport<T> typeSupport;
    private volatile DataReaderQos qos;
    /** The partition names of the reader's Subscriber (DDS 1.2 §7.1.3.9). */
    private volatile Set<String> partitions;
    private final Predicate<T> contentFilter; // null = no CFT
    private volatile TopicDescription<T> description; // null = the topic itself
    private volatile Runnable closeHook;

    // Per-instance sample queue + lifecycle bookkeeping.
    private final Map<InstanceKey, Instance> instances = new LinkedHashMap<>();

    // EXCLUSIVE-ownership: per-instance current owner writerId + its strength.
    private final Map<InstanceKey, OwnerInfo> owners = new LinkedHashMap<>();

    // REQUESTED_DEADLINE_MISSED tracking.
    private int deadlineMissTotal = 0;
    private int deadlineMissReadBaseline = 0;
    private InstanceHandle lastDeadlineMissHandle = Values.nil();

    // LIVELINESS_CHANGED tracking (against matched writers).
    private int livAliveReadBaseline = 0;
    private int livNotAliveReadBaseline = 0;

    // TIME_BASED_FILTER (DDS 1.4 §2.2.3.12): the samples held back per instance.
    private final TimeBasedFilterState<PendingSample<T>> timeFilter = new TimeBasedFilterState<>();
    // READER_DATA_LIFECYCLE / TIME_BASED_FILTER sweep (DDS 1.4 §2.2.3.22).
    private java.util.concurrent.ScheduledFuture<?> sweepLease;

    /** A sample the TIME_BASED_FILTER held back, with the message it arrived in. */
    private static final class PendingSample<T> {
        final InProcessBus.Message message;
        final T value;

        PendingSample(InProcessBus.Message message, T value) {
            this.message = message;
            this.value = value;
        }
    }

    // Lease tasks (DDS 1.2 §7.1.4.2 — deadline/liveliness are time-based and
    // are reported asynchronously, not only when the status is polled).
    private java.util.concurrent.ScheduledFuture<?> deadlineLease;
    private java.util.concurrent.ScheduledFuture<?> livelinessLease;

    // SUBSCRIPTION_MATCHED tracking: the set of matched writers, so
    // "already matched" and "matched again" are distinguishable — the bus can
    // report the same writer twice (register + late-join replay path) and a
    // count kept as a plain integer would drift.
    private final Set<Long> matchedWriters = new LinkedHashSet<>();
    private int subMatchedTotal = 0;
    private int subMatchedTotalReadBaseline = 0;
    private int subMatchedCurrentReadBaseline = 0;

    // REQUESTED_INCOMPATIBLE_QOS tracking (DDS 1.2 §7.1.4.1): writers whose
    // offered QoS is incompatible with this reader's requested QoS do not
    // match, and their samples are not delivered. Their offered QoS and the
    // refusal are recorded.
    private final Map<Long, Map<Class<? extends QosPolicy>, ? extends QosPolicy>> writerQos =
            new LinkedHashMap<>();
    private final Set<Long> incompatibleWriters = new LinkedHashSet<>();
    private int requestedIncompatTotal = 0;
    private int requestedIncompatReadBaseline = 0;
    private Class<? extends QosPolicy> lastRequestedIncompatPolicyClass = null;
    private final Map<Class<? extends QosPolicy>, Integer> requestedIncompatPolicies =
            new LinkedHashMap<>();

    // Read by the bus and by listener callbacks without the monitor
    // (DDS-Java-PSM 1.0 §7.2.4 — reentrant operations).
    private volatile boolean closed = false;
    private volatile boolean enabled = false;
    /** A reader of a MultiTopic join: the implementation reads it, the application does not. */
    private volatile boolean internal = false;

    // Optional pure-Java RTPS wire path (cross-process), alongside InProcessBus.
    private org.zerodds.rtps.RtpsParticipant.WireReader wire;
    /** The DDS-RPC 1.0 §7.6.2.1 discovery extensions this reader announces. */
    private volatile org.zerodds.rtps.RpcDiscovery rpc = org.zerodds.rtps.RpcDiscovery.NONE;
    // Synthetic identity for samples arriving over the wire (best-effort path).
    private static final long WIRE_WRITER_ID = -7_000_000_001L;

    // The ReadConditions and QueryConditions this reader created (DDS 1.4 §2.2.2.5.3.5).
    private final List<ReadConditionImpl<T>> conditions =
            new java.util.concurrent.CopyOnWriteArrayList<>();
    // The writer whose match or liveliness changed last (last_publication_handle).
    private InstanceHandle lastMatchedHandle = Values.nil();
    private InstanceHandle lastLivelinessHandle = Values.nil();
    private final Map<Long, Boolean> livelinessSeen = new LinkedHashMap<>();
    /** The liveliness last reported for each matched remote writer (by GUID). */
    private final Map<String, Boolean> remoteLivelinessSeen = new LinkedHashMap<>();

    // SAMPLE_REJECTED (DDS 1.4 §2.2.4.1): samples refused because of RESOURCE_LIMITS.
    private int rejectedTotal = 0;
    private int rejectedReadBaseline = 0;
    private SampleRejectedStatus.Kind lastRejectedReason = SampleRejectedStatus.Kind.NOT_REJECTED;
    private InstanceHandle lastRejectedHandle = Values.nil();
    private boolean rejectionPending;
    // SAMPLE_LOST: samples a writer sent that never reached this reader (a gap in the
    // sequence numbers of the writer's samples on the wire).
    private int lostTotal = 0;
    private int lostReadBaseline = 0;
    private boolean lossPending;
    private final Map<String, Long> lastSequence = new java.util.HashMap<>();
    /**
     * The order of arrival over all readers of the process: ordered access with access_scope GROUP
     * orders the samples of several readers (DDS 1.4 2.2.3.6).
     */
    private static final java.util.concurrent.atomic.AtomicLong ARRIVALS =
            new java.util.concurrent.atomic.AtomicLong();

    private static final class Instance {
        final java.util.LinkedList<Stored<Object>> samples = new java.util.LinkedList<>();
        /** The first sample of the instance that carried data: the source of get_key_value. */
        Object keySample;
        InstanceState state = InstanceState.ALIVE;
        long lastSampleMillis = System.currentTimeMillis();
        /** The newest source timestamp accepted (DESTINATION_ORDER BY_SOURCE_TIMESTAMP). */
        long lastSourceMillis = Long.MIN_VALUE;
        /** When the instance entered its current instance_state (READER_DATA_LIFECYCLE). */
        long stateSinceMillis = System.currentTimeMillis();
        int deadlinePeriodsCharged = 0; // missed-periods already counted
        // DDS 1.2 §7.1.2.5.1.8: the instance is NEW until the application first
        // accesses it with read/take, then NOT_NEW (until reborn).
        boolean viewNew = true;
        // Writer ids currently writing this instance (DDS 1.2 §7.1.2.4.2.7):
        // the instance becomes NOT_ALIVE_NO_WRITERS only when this set empties.
        final Set<Long> writers = new LinkedHashSet<>();
        // DDS 1.4 §2.2.2.5.1.5 SampleInfo: how many times the instance became ALIVE again
        // after NOT_ALIVE_DISPOSED, and after NOT_ALIVE_NO_WRITERS.
        int disposedGeneration;
        int noWritersGeneration;
        // A sample of the instance: it carries the key fields (get_key_value).
        Object keyData;
    }

    /**
     * A sample as stored by the reader. sample_state is per sample (DDS 1.2
     * §7.1.2.5.1.2); view_state and instance_state are per instance and are
     * snapshotted only when read/take collects the samples (§7.1.2.5.1.3 /
     * §7.1.2.5.1.8), so they are not stored here.
     */
    private static final class Stored<T> {
        final T data;
        final InstanceHandle instanceHandle;
        final InstanceHandle publicationHandle;
        final org.omg.dds.core.Time sourceTimestamp;
        long sourceMillis;
        SampleState sampleState = SampleState.NOT_READ;
        /** The generation counts of the instance when the sample was received. */
        int disposedGeneration;
        int noWritersGeneration;
        /** The order of arrival at this reader. */
        long arrival;
        /** The time from which the sample is gone (LIFESPAN), or {@link Long#MAX_VALUE}. */
        long expiresAtMillis = Long.MAX_VALUE;
        /** The writer GUID and sequence number of the sample, and the identity it relates to. */
        org.zerodds.internal.SourceIdentity identity;
        org.zerodds.internal.SourceIdentity related;
        /** The octets the sample arrived as, kept when the type support retains them. */
        byte[] serialized;

        Stored(T data, InstanceHandle instanceHandle, InstanceHandle publicationHandle,
                org.omg.dds.core.Time sourceTimestamp) {
            this.data = data;
            this.instanceHandle = instanceHandle;
            this.publicationHandle = publicationHandle;
            this.sourceTimestamp = sourceTimestamp;
        }
    }

    private static final class OwnerInfo {
        long writerId;
        int strength;
    }

    public DataReaderImpl(int domainId, Subscriber parent, Topic<T> topic,
                      TopicTypeSupport<T> typeSupport,
                      DataReaderQos qos, Set<String> partitions) {
        this(domainId, parent, topic, typeSupport, qos, partitions, null);
    }

    public DataReaderImpl(int domainId, Subscriber parent, Topic<T> topic,
                      TopicTypeSupport<T> typeSupport,
                      DataReaderQos qos, Set<String> partitions,
                      Predicate<T> contentFilter) {
        super(parent == null ? Values.getDefaultEnvironment() : parent.getEnvironment());
        org.zerodds.core.qos.QosRules.checkConsistent(qos);
        this.domainId = domainId;
        this.parent = parent;
        this.topic = topic;
        if (topic instanceof org.zerodds.topic.TopicImpl) {
            final org.zerodds.topic.TopicImpl<?> used = (org.zerodds.topic.TopicImpl<?>) topic;
            used.use();
            addCloseHook(used::release);
        }
        this.typeSupport = typeSupport;
        this.qos = qos;
        this.partitions = new LinkedHashSet<>(partitions);
        this.contentFilter = contentFilter;
    }

    @Override
    public Subscriber getParent() {
        return parent;
    }

    @Override
    protected DataReaderQos profileQos(QosProvider provider) {
        return provider.getDataReaderQos();
    }

    // ---- bus delivery ----------------------------------------------------

    @Override
    public List<String> partitions() {
        return new ArrayList<>(partitions);
    }

    /** Marks the reader as one the implementation reads on its own behalf. */
    void markInternal() {
        internal = true;
    }

    /**
     * The PARTITION of the Subscriber changed (DDS 1.4 §2.2.3.13): the reader matches the writers
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
            InProcessBus.instance().readerPartitionsChanged(
                    domainId, topic.getName(), typeSupport.getTypeName(), this, before);
        }
    }

    /**
     * Sets the DDS-RPC 1.0 §7.6.2.1 discovery extensions the reader announces:
     * {@code service_instance_name} (PID 0x0080), {@code related_datawriter_key}
     * (PID 0x0081) and {@code topic_aliases} (PID 0x0082). They are in the SEDP announcement
     * from the enable on, and a call after it announces the reader again.
     */
    public void setRpcDiscovery(org.zerodds.rtps.RpcDiscovery value) {
        org.zerodds.rtps.RpcDiscovery next =
                value == null ? org.zerodds.rtps.RpcDiscovery.NONE : value;
        org.zerodds.rtps.RtpsParticipant.WireReader remote;
        synchronized (this) {
            rpc = next;
            remote = wire;
        }
        if (remote != null) {
            remote.setRpcDiscovery(next);
        }
    }

    /**
     * The GUID of this reader: its RTPS endpoint GUID, or its instance handle when it has no
     * wire endpoint (the readers of one process).
     */
    @Override
    public byte[] endpointGuid() {
        org.zerodds.rtps.RtpsParticipant.WireReader remote = wire;
        if (remote != null) {
            return remote.guid();
        }
        return org.zerodds.core.InstanceHandleImpl.bytesOf(getInstanceHandle()).clone();
    }

    /** The discovery extensions the reader announces. */
    @Override
    public org.zerodds.rtps.RpcDiscovery rpcDiscovery() {
        return rpc;
    }

    /**
     * The GUIDs and discovery extensions of the matched writers, one entry per writer, those
     * of this process and those discovered by SEDP (DDS-RPC 1.0 §7.6.2.1.2).
     */
    public List<org.zerodds.rtps.RpcPeer> getMatchedRpcPeers() {
        List<org.zerodds.rtps.RpcPeer> out = new ArrayList<>();
        Set<Long> matched;
        synchronized (this) {
            matched = new LinkedHashSet<>(matchedWriters);
        }
        for (InProcessBus.WriterPresence writer : InProcessBus.instance().writers(
                domainId, topic.getName(), typeSupport.getTypeName())) {
            if (matched.contains(writer.writerId())
                    && writer instanceof org.zerodds.pub.DataWriterImpl) {
                org.zerodds.pub.DataWriterImpl<?> local = (org.zerodds.pub.DataWriterImpl<?>) writer;
                out.add(new org.zerodds.rtps.RpcPeer(
                        local.nextIdentity().guid(), local.rpcDiscovery()));
            }
        }
        org.zerodds.rtps.RtpsParticipant.WireReader remote = wire;
        if (remote != null) {
            for (org.zerodds.rtps.DiscoveryData.Endpoint endpoint : remote.matchedEndpoints()) {
                out.add(new org.zerodds.rtps.RpcPeer(endpoint.endpointGuid, endpoint.rpc));
            }
        }
        return out;
    }


    /** The reader's own instance handle: what a matched writer lists it as (DDS 1.4 §2.2.2.4.2.22). */
    @Override
    public InstanceHandle instanceHandle() {
        return getInstanceHandle();
    }

    @Override
    public Map<Class<? extends QosPolicy>, ? extends QosPolicy> requestedQos() {
        // PRESENTATION is a policy of the Subscriber and part of the match (DDS 1.4 §2.2.3.6).
        return QosCompatibility.withPresentation(QosCompatibility.policiesOf(qos),
                parent == null || parent.getQos() == null ? null : parent.getQos().getPresentation());
    }

    /**
     * Type coercion (TypeConsistencyEnforcement, DDS-Java-PSM 1.0 7.3.5.1): DECLARED and
     * ASSIGNABLE readers also take writers of another type name of the topic.
     */
    @Override
    public boolean coercesTypes() {
        org.omg.dds.core.policy.TypeConsistencyEnforcement.Kind kind = consistencyKind();
        return kind == org.omg.dds.core.policy.TypeConsistencyEnforcement.Kind.DECLARED_TYPE_CONSISTENCY
                || kind == org.omg.dds.core.policy.TypeConsistencyEnforcement.Kind.ASSIGNABLE_TYPE_CONSISTENCY;
    }

    private org.omg.dds.core.policy.TypeConsistencyEnforcement.Kind consistencyKind() {
        DataReaderQos q = qos;
        org.omg.dds.core.policy.TypeConsistencyEnforcement policy = q == null ? null : q.getTypeConsistency();
        return policy == null ? null : policy.getKind();
    }

    /**
     * Whether the remote writer announced a type this reader accepts: the XTypes 1.1 TypeObject of
     * the built-in data (PID 0x0072) is turned into a DynamicType and judged like a local writer.
     * A writer that announced no TypeObject cannot be judged and does not match.
     */
    private boolean acceptsRemoteWriter(org.zerodds.rtps.DiscoveryData.Endpoint writer) {
        if (writer.qos == null || writer.qos.typeObject == null) {
            return false;
        }
        org.omg.dds.type.dynamic.DynamicType remote =
                org.zerodds.typeobject.TypeObjects.dynamicTypeOrNull(env, writer.qos.typeObject);
        return remote != null && acceptsType(writer.typeName, remote);
    }

    @Override
    public boolean acceptsType(String writerTypeName, org.omg.dds.type.dynamic.DynamicType writerType) {
        org.omg.dds.type.dynamic.DynamicType mine =
                org.zerodds.typeobject.TypeObjects.dynamicTypeOf(env, typeSupport);
        if (mine == null || writerType == null) {
            return false;
        }
        return consistencyKind() == org.omg.dds.core.policy.TypeConsistencyEnforcement.Kind.DECLARED_TYPE_CONSISTENCY
                ? org.zerodds.cdr.TypeAssignability.declared(mine, writerType)
                : org.zerodds.cdr.TypeAssignability.assignableFrom(mine, writerType);
    }

    @Override
    public void onWriterPresenceChanged(InProcessBus.WriterPresence writer, boolean present) {
        // A writer whose partitions do not overlap is no match; when its partitions or this
        // reader's changed, one that was a match ends being one.
        if (present && !partitionOverlaps(writer.partitions())) {
            present = false;
        }
        DataReaderListener<T> l;
        SubscriptionMatchedStatus status;
        RequestedIncompatibleQosStatus refused = null;
        synchronized (this) {
            if (!present) {
                writerQos.remove(writer.writerId());
                incompatibleWriters.remove(writer.writerId());
                boolean changed = matchedWriters.remove(writer.writerId());
                livelinessSeen.remove(writer.writerId());
                if (!changed) {
                    return;
                }
                lastMatchedHandle = handleOf(writer);
                lastLivelinessHandle = handleOf(writer);
                triggerStatus(SUBSCRIPTION_MATCHED);
                l = listenerFor(SUBSCRIPTION_MATCHED);
                status = l == null ? null : readSubscriptionMatchedStatusLocked();
            } else {
                // §2.2.3.13: a writer whose partitions do not overlap is not a
                // match, so its arrival is not a SUBSCRIPTION_MATCHED event.
                if (!partitionOverlaps(writer.partitions())) {
                    return;
                }
                // §2.2.3 RxO compatibility: an incompatible offered QoS is not
                // a match; the refusal is recorded (REQUESTED_INCOMPATIBLE_QOS).
                Map<Class<? extends QosPolicy>, ? extends QosPolicy> offered = writer.offeredQos();
                writerQos.put(writer.writerId(), offered);
                if (offered != null && !QosCompatibility.isCompatible(requestedQos(), offered)) {
                    incompatibleWriters.add(writer.writerId());
                    recordRequestedIncompatibleLocked(
                            QosCompatibility.incompatiblePolicies(requestedQos(), offered));
                    l = listenerFor(REQUESTED_INCOMPATIBLE);
                    if (l == null) {
                        return;
                    }
                    refused = getRequestedIncompatibleQosStatus();
                    status = null;
                } else if (!matchedWriters.add(writer.writerId())) {
                    return;
                } else {
                    subMatchedTotal++;
                    lastMatchedHandle = handleOf(writer);
                    triggerStatus(SUBSCRIPTION_MATCHED);
                    l = listenerFor(SUBSCRIPTION_MATCHED);
                    // DDS 1.4 §2.2.4.1: the change counters reset when the status is
                    // read *or* when it is communicated through the listener.
                    status = l == null ? null : readSubscriptionMatchedStatusLocked();
                }
            }
        }
        if (l != null && refused != null) {
            l.onRequestedIncompatibleQos(new RequestedIncompatibleQosEventImpl<>(env, this, refused));
        } else if (l != null) {
            l.onSubscriptionMatched(new SubscriptionMatchedEventImpl<>(env, this, status));
        }
    }

    /** The handle a bus writer is known by: its publication handle, nil when it has none. */
    private static InstanceHandle handleOf(InProcessBus.WriterPresence writer) {
        InstanceHandle handle = writer.writerHandle();
        return handle == null ? Values.nil() : handle;
    }

    @Override
    public void onMessage(InProcessBus.Message m) {
        boolean dataAvailable;
        synchronized (this) {
            // PARTITION (§2.2.3.13): only accept from writers whose partition set
            // overlaps the reader's.
            if (!partitionOverlaps(m.partitions)) {
                return;
            }
            // §2.2.3 RxO compatibility: an incompatible writer never delivers.
            if (incompatibleWriters.contains(m.writerId)) {
                return;
            }
            InstanceKey key = new InstanceKey(m.keyHash);

            dataAvailable = false;
            switch (m.kind) {
                case WRITE:
                    dataAvailable = onWrite(m, key);
                    break;
                case DISPOSE:
                    markState(key, InstanceState.NOT_ALIVE_DISPOSED, m);
                    owners.remove(key);
                    dataAvailable = true;
                    break;
                case UNREGISTER:
                    if (m.keyHash.length == 0 && !typeSupport.isKeyed()
                            && instances.isEmpty()) {
                        // liveliness-lost ping for an un-keyed reader with no data;
                        // nothing to mark.
                        break;
                    }
                    // §7.1.2.4.2.7: NOT_ALIVE_NO_WRITERS only when no other writer
                    // is still writing the instance.
                    if (!stillHasWriter(key, m)) {
                        Instance known = instances.get(key);
                        if (known != null && known.state == InstanceState.NOT_ALIVE_DISPOSED) {
                            // §2.2.2.5.1.3: a disposed instance stays NOT_ALIVE_DISPOSED
                            // when its last writer goes; it may be reclaimed now.
                            reclaim(key);
                        } else {
                            markState(key, InstanceState.NOT_ALIVE_NO_WRITERS, m);
                            dataAvailable = true;
                        }
                    }
                    owners.remove(key);
                    break;
                default:
            }
        }
        // Listener callbacks run without this reader's monitor held: the
        // delivering thread must not block a concurrent call on any entity
        // (DDS-Java-PSM 1.0 §7.2.4).
        if (dataAvailable) {
            fireDataAvailable();
        }
        fireRejectedAndLost();
    }

    /**
     * Removes {@code m.writerId} from the instance's active-writer set and
     * reports whether any other writer is still writing the instance (DDS 1.2
     * §7.1.2.4.2.7).
     */
    private boolean stillHasWriter(InstanceKey key, InProcessBus.Message m) {
        Instance inst = instances.get(key);
        if (inst == null) {
            return false;
        }
        inst.writers.remove(m.writerId);
        return !inst.writers.isEmpty();
    }

    /**
     * DDS 1.4 §2.2.2.5.1.8: once an instance has no live writers and all its samples are
     * taken, the reader can reclaim it; later samples of that key are an instance never seen.
     */
    private void reclaim(InstanceKey key) {
        Instance inst = instances.get(key);
        if (inst != null && inst.state != InstanceState.ALIVE && inst.samples.isEmpty()
                && inst.writers.isEmpty()) {
            instances.remove(key);
            owners.remove(key);
        }
    }

    private boolean onWrite(InProcessBus.Message m, InstanceKey key) {
        return onWrite(m, key, () -> typeSupport.deserialize(Xcdr2Codec.decoder(m.payload)));
    }

    /**
     * RESOURCE_LIMITS (DDS 1.4 §2.2.3.19): a sample that would exceed max_instances,
     * max_samples or max_samples_per_instance is rejected and recorded as SAMPLE_REJECTED.
     * With HISTORY KEEP_LAST the oldest sample of the instance gives way to the new one,
     * so only the other two limits can reject there.
     */
    private boolean rejectedByLimits(InstanceKey key) {
        org.omg.dds.core.policy.ResourceLimits limits = qos.getResourceLimits();
        Instance existing = instances.get(key);
        SampleRejectedStatus.Kind reason = null;
        History h = qos.getHistory();
        boolean keepLast = h.getKind() == History.Kind.KEEP_LAST;
        if (existing == null && limits.getMaxInstances() >= 0
                && instances.size() >= limits.getMaxInstances()) {
            reason = SampleRejectedStatus.Kind.REJECTED_BY_INSTANCES_LIMIT;
        } else if (!keepLast && existing != null && limits.getMaxSamplesPerInstance() >= 0
                && existing.samples.size() >= limits.getMaxSamplesPerInstance()) {
            reason = SampleRejectedStatus.Kind.REJECTED_BY_SAMPLES_PER_INSTANCE_LIMIT;
        } else if (limits.getMaxSamples() >= 0) {
            int stored = 0;
            for (Instance inst : instances.values()) {
                stored += inst.samples.size();
            }
            int replaced = keepLast && existing != null
                    && existing.samples.size() >= Math.max(1, h.getDepth()) ? 1 : 0;
            if (stored - replaced >= limits.getMaxSamples()) {
                reason = SampleRejectedStatus.Kind.REJECTED_BY_SAMPLES_LIMIT;
            }
        }
        if (reason == null) {
            return false;
        }
        rejectedTotal++;
        lastRejectedReason = reason;
        lastRejectedHandle = InstanceKey.toHandle(key);
        rejectionPending = true;
        triggerStatus(SAMPLE_REJECTED);
        return true;
    }

    /**
     * The listener told about a change of {@code kind}: the reader's own, else the
     * Subscriber's, else the DomainParticipant's (DDS 1.4 §2.2.4.3.1).
     */
    private DataReaderListener<T> listenerFor(Class<? extends Status> kind) {
        DataReaderListener<T> own = ownListener(kind);
        if (own != null) {
            return own;
        }
        if (parent instanceof SubscriberImpl) {
            SubscriberImpl subscriber = (SubscriberImpl) parent;
            org.omg.dds.sub.SubscriberListener up = subscriber.ownListener(kind);
            if (up != null) {
                return org.zerodds.core.ListenerUp.forReader(up);
            }
            if (subscriber.getParent() instanceof org.zerodds.domain.DomainParticipantImpl) {
                org.omg.dds.domain.DomainParticipantListener top =
                        ((org.zerodds.domain.DomainParticipantImpl) subscriber.getParent())
                                .ownListener(kind);
                if (top != null) {
                    return org.zerodds.core.ListenerUp.forReader(top);
                }
            }
        }
        return null;
    }

    /** Hands a recorded rejection and a recorded loss to the listener, outside the monitor. */
    private void fireRejectedAndLost() {
        DataReaderListener<T> l;
        SampleRejectedStatus rejected = null;
        SampleLostStatus lost = null;
        synchronized (this) {
            l = getListener();
            if (rejectionPending) {
                rejectionPending = false;
                if (l != null && listenerWants(SAMPLE_REJECTED)) {
                    rejected = readSampleRejectedLocked();
                }
            }
            if (lossPending) {
                lossPending = false;
                if (l != null && listenerWants(SAMPLE_LOST)) {
                    lost = readSampleLostLocked();
                }
            }
        }
        if (rejected != null) {
            l.onSampleRejected(new SampleRejectedEventImpl<>(env, this, rejected));
        }
        if (lost != null) {
            l.onSampleLost(new SampleLostEventImpl<>(env, this, lost));
        }
    }

    /** A gap in the sequence numbers of one writer's samples is SAMPLE_LOST (DDS 1.4 §2.2.4.1). */
    private void noteSequence(org.zerodds.internal.SourceIdentity identity) {
        if (identity == null) {
            return;
        }
        String writer = org.zerodds.cdr.HexBytes.encode(identity.guid());
        Long last = lastSequence.get(writer);
        long next = identity.sequence();
        if (last != null && next > last + 1) {
            lostTotal += (int) (next - last - 1);
            lossPending = true;
            triggerStatus(SAMPLE_LOST);
        }
        if (last == null || next > last) {
            lastSequence.put(writer, next);
        }
    }

    private SampleRejectedStatus readSampleRejectedLocked() {
        int change = rejectedTotal - rejectedReadBaseline;
        rejectedReadBaseline = rejectedTotal;
        return new SampleRejectedStatusImpl(env, rejectedTotal, change, lastRejectedReason,
                lastRejectedHandle);
    }

    private SampleLostStatus readSampleLostLocked() {
        int change = lostTotal - lostReadBaseline;
        lostReadBaseline = lostTotal;
        return new SampleLostStatusImpl(env, lostTotal, change);
    }

    /** Stores the sample; returns whether the reader gained data to read/take. */
    private boolean onWrite(InProcessBus.Message m, InstanceKey key,
            java.util.function.Supplier<T> decode) {
        // LIFESPAN (DDS 1.4 §2.2.3.16): a sample whose validity has passed is not delivered.
        if (m.expiresAtMillis() != Long.MAX_VALUE
                && System.currentTimeMillis() >= m.expiresAtMillis()) {
            return false;
        }
        // EXCLUSIVE OWNERSHIP arbitration (§2.2.3.12.1): for each instance only
        // the highest-strength writer (the owner) updates the reader's view.
        if (m.exclusive && qos.getOwnership().getKind() == Ownership.Kind.EXCLUSIVE) {
            OwnerInfo owner = owners.get(key);
            if (owner == null || m.ownershipStrength > owner.strength
                    || (m.ownershipStrength == owner.strength && m.writerId == owner.writerId)) {
                if (owner == null) owner = new OwnerInfo();
                owner.writerId = m.writerId;
                owner.strength = m.ownershipStrength;
                owners.put(key, owner);
            } else if (m.writerId != owner.writerId) {
                return false; // lower-strength non-owner writer -> filtered out
            }
        }

        T value = decode.get();

        // CONTENT-FILTERED-TOPIC (§2.2.2.3.3): drop samples failing the filter.
        if (contentFilter != null && !contentFilter.test(value)) {
            return false;
        }

        // DESTINATION_ORDER BY_SOURCE_TIMESTAMP (DDS 1.4 §2.2.3.17, §2.2.2.5.3.8): a sample
        // that arrives with an older source timestamp than the newest of its instance is not
        // dropped; insertOrdered puts it before the newer ones, so the samples of an instance
        // appear in the order of their source timestamps and the last one is the final value.
        // TIME_BASED_FILTER (DDS 1.4 §2.2.3.12): at most one sample of an instance per
        // minimum_separation; the newest one held back is delivered when that has passed.
        long separation = timeFilterMillis();
        if (separation > 0 && !timeFilter.offer(key, System.currentTimeMillis(), separation,
                new PendingSample<>(m, value))) {
            return false;
        }
        return storeSample(m, key, value);
    }

    /** The {@code minimum_separation} of this reader in milliseconds; zero when there is none. */
    private long timeFilterMillis() {
        return qos.getTimeBasedFilter().getMinimumSeparation().getDuration(TimeUnit.MILLISECONDS);
    }

    /** Stores an accepted sample in its instance; returns whether the reader gained data. */
    private boolean storeSample(InProcessBus.Message m, InstanceKey key, T value) {
        if (rejectedByLimits(key)) {
            return false;
        }
        Instance inst = instances.computeIfAbsent(key, k -> new Instance());
        boolean reborn = inst.state != InstanceState.ALIVE;
        // DDS 1.4 §2.2.2.5.1.5: the generation counts grow when the instance becomes ALIVE
        // again, after NOT_ALIVE_DISPOSED and after NOT_ALIVE_NO_WRITERS respectively.
        if (inst.state == InstanceState.NOT_ALIVE_DISPOSED) {
            inst.disposedGeneration++;
        } else if (inst.state == InstanceState.NOT_ALIVE_NO_WRITERS) {
            inst.noWritersGeneration++;
        }
        if (inst.keyData == null) {
            inst.keyData = value;
        }
        inst.state = InstanceState.ALIVE;
        // DEADLINE (§2.2.3.7) counts from the reception of the sample.
        inst.lastSampleMillis = System.currentTimeMillis();
        inst.lastSourceMillis = Math.max(inst.lastSourceMillis, m.sourceTimestampMillis);
        if (reborn) {
            inst.stateSinceMillis = System.currentTimeMillis();
        }
        inst.deadlinePeriodsCharged = 0;
        inst.writers.add(m.writerId);
        if (inst.keySample == null) {
            inst.keySample = value;
        }
        // A reborn instance is NEW again (§7.1.2.5.1.8): the application has
        // not yet seen the new generation.
        if (reborn) {
            inst.viewNew = true;
        }

        Stored<Object> s = new Stored<>(
                value,
                InstanceKey.toHandle(key),
                m.writerHandle,
                Values.ofMillis(m.sourceTimestampMillis));
        s.identity = m.identity;
        s.related = m.related;
        if (typeSupport.retainsSerialized()) {
            s.serialized = m.payload;
        }
        s.disposedGeneration = inst.disposedGeneration;
        s.noWritersGeneration = inst.noWritersGeneration;
        s.arrival = ARRIVALS.incrementAndGet();
        s.expiresAtMillis = m.expiresAtMillis();

        // HISTORY keep-last depth (§2.2.3.18) per instance.
        History h = qos.getHistory();
        if (h.getKind() == History.Kind.KEEP_LAST) {
            int depth = Math.max(1, h.getDepth());
            while (inst.samples.size() >= depth) {
                inst.samples.pollFirst();
            }
        }
        insertOrdered(inst, s, m.sourceTimestampMillis);
        return true;
    }

    /**
     * DESTINATION_ORDER (DDS 1.4 §2.2.3.17): with BY_SOURCE_TIMESTAMP the samples of an
     * instance are kept in the order of their source timestamps, a sample that arrives late
     * with an older timestamp goes before the newer ones; with BY_RECEPTION_TIMESTAMP they
     * stay in the order of arrival.
     */
    private void insertOrdered(Instance inst, Stored<Object> s, long sourceMillis) {
        s.sourceMillis = sourceMillis;
        if (qos.getDestinationOrder().getKind()
                != org.omg.dds.core.policy.DestinationOrder.Kind.BY_SOURCE_TIMESTAMP) {
            inst.samples.addLast(s);
            return;
        }
        int position = inst.samples.size();
        while (position > 0 && inst.samples.get(position - 1).sourceMillis > sourceMillis) {
            position--;
        }
        inst.samples.add(position, s);
    }

    /**
     * The time-based clean-up of the reader's store, run before every access and by the
     * sweep: samples whose LIFESPAN has passed (DDS 1.4 §2.2.3.16), samples the
     * TIME_BASED_FILTER held back whose separation has passed (§2.2.3.12), and the instances
     * and samples READER_DATA_LIFECYCLE gives up (§2.2.3.22).
     */
    private boolean purgeLocked() {
        boolean gained = false;
        long now = System.currentTimeMillis();
        long separation = timeFilterMillis();
        if (separation > 0 && timeFilter.holdsSamples()) {
            for (PendingSample<T> held : timeFilter.due(now, separation)) {
                InstanceKey heldKey = new InstanceKey(held.message.keyHash);
                gained |= storeSample(held.message, heldKey, held.value);
            }
        }
        org.omg.dds.core.policy.ReaderDataLifecycle lifecycle = qos.getReaderDataLifecycle();
        Duration noWriters = lifecycle.getAutoPurgeNoWriterSamplesDelay();
        Duration disposed = lifecycle.getAutoPurgeDisposedSamplesDelay();
        java.util.Iterator<Map.Entry<InstanceKey, Instance>> it = instances.entrySet().iterator();
        while (it.hasNext()) {
            Map.Entry<InstanceKey, Instance> entry = it.next();
            Instance inst = entry.getValue();
            inst.samples.removeIf(sample -> sample.data != null && now >= sample.expiresAtMillis);
            if (inst.state == InstanceState.NOT_ALIVE_NO_WRITERS && !noWriters.isInfinite()
                    && now - inst.stateSinceMillis >= noWriters.getDuration(TimeUnit.MILLISECONDS)) {
                // All internal information about the instance goes, untaken samples with it.
                it.remove();
                owners.remove(entry.getKey());
                timeFilter.forget(entry.getKey());
            } else if (inst.state == InstanceState.NOT_ALIVE_DISPOSED && !disposed.isInfinite()
                    && now - inst.stateSinceMillis >= disposed.getDuration(TimeUnit.MILLISECONDS)) {
                inst.samples.clear();
            }
        }
        return gained;
    }

    /**
     * The sweep that applies {@link #purgeLocked} when nobody accesses the reader, so that a
     * reader whose application never takes does not keep what READER_DATA_LIFECYCLE gives up,
     * and a sample the TIME_BASED_FILTER held back is delivered when its separation has passed.
     */
    private void sweep() {
        boolean gained;
        synchronized (this) {
            if (closed) {
                return;
            }
            gained = purgeLocked();
        }
        if (gained) {
            fireDataAvailable();
        }
    }

    /** The period of the sweep in milliseconds, or zero when nothing needs one. */
    private long sweepPeriodMillis() {
        org.omg.dds.core.policy.ReaderDataLifecycle lifecycle = qos.getReaderDataLifecycle();
        long shortest = Long.MAX_VALUE;
        for (Duration delay : new Duration[] {lifecycle.getAutoPurgeNoWriterSamplesDelay(),
                lifecycle.getAutoPurgeDisposedSamplesDelay()}) {
            if (!delay.isInfinite()) {
                shortest = Math.min(shortest, delay.getDuration(TimeUnit.MILLISECONDS));
            }
        }
        long separation = timeFilterMillis();
        if (separation > 0) {
            shortest = Math.min(shortest, separation);
        }
        return shortest == Long.MAX_VALUE ? 0 : Math.max(10, Math.min(1000, shortest / 2));
    }

    private synchronized void startSweepLocked() {
        if (sweepLease != null) {
            sweepLease.cancel(false);
            sweepLease = null;
        }
        long period = sweepPeriodMillis();
        if (period > 0) {
            sweepLease = LeaseMonitor.scheduleAtFixedRate(this::sweep, period);
        }
    }

    /**
     * Spec §2.2.4.2.6.1 — raised once per sample that read/take will return.
     *
     * <p>Fired from the delivering thread <em>without</em> this reader's
     * monitor held, so an application call on another entity from inside the
     * callback cannot block against a concurrent operation on this reader
     * (DDS-Java-PSM 1.0 §7.2.4).
     */
    private void fireDataAvailable() {
        triggerStatus(DATA_AVAILABLE);
        if (parent instanceof SubscriberImpl) {
            ((SubscriberImpl) parent).dataOnReadersChanged();
        }
        // a ReadCondition of this reader may have become true
        org.zerodds.core.condition.WaitSetImpl.conditionChanged();
        // DDS 1.4 §2.2.4.3.1: a Subscriber listener that takes DATA_ON_READERS is told
        // that and the readers' own DATA_AVAILABLE is not raised.
        if (parent instanceof SubscriberImpl) {
            SubscriberImpl subscriber = (SubscriberImpl) parent;
            org.omg.dds.sub.SubscriberListener up =
                    subscriber.ownListener(org.omg.dds.core.status.DataOnReadersStatus.class);
            if (up == null && subscriber.getParent() instanceof org.zerodds.domain.DomainParticipantImpl) {
                up = ((org.zerodds.domain.DomainParticipantImpl) subscriber.getParent())
                        .ownListener(org.omg.dds.core.status.DataOnReadersStatus.class);
            }
            if (up != null) {
                // the status is communicated through the listener: it is no longer changed
                subscriber.dataOnReadersAccessed();
                up.onDataOnReaders(new org.zerodds.core.event.DataOnReadersEventImpl(env, subscriber,
                        new org.zerodds.core.status.DataOnReadersStatusImpl(env)));
                return;
            }
        }
        DataReaderListener<T> l = listenerFor(DATA_AVAILABLE);
        if (l != null) {
            l.onDataAvailable(new DataAvailableEventImpl<>(
                    env, this, new DataAvailableStatusImpl(env)));
        }
    }

    private void markState(InstanceKey key, InstanceState state, InProcessBus.Message m) {
        Instance inst = instances.get(key);
        if (inst == null) {
            inst = new Instance();
            instances.put(key, inst);
        }
        inst.state = state;
        inst.stateSinceMillis = System.currentTimeMillis();
        // Surface a NOT_ALIVE notification sample carrying the new instance
        // state with no data (DDS-DCPS 1.4 §2.2.2.5.1.4 valid_data — readers see the
        // disposed/no-writers transition via SampleInfo).
        Stored<Object> s = new Stored<>(null, InstanceKey.toHandle(key),
                m.writerHandle, Values.ofMillis(m.sourceTimestampMillis));
        s.disposedGeneration = inst.disposedGeneration;
        s.noWritersGeneration = inst.noWritersGeneration;
        s.arrival = ARRIVALS.incrementAndGet();
        s.sourceMillis = m.sourceTimestampMillis;
        inst.samples.addLast(s);
    }

    /**
     * Decodes once, stores that value and fires the callback — the wire path of
     * {@link #onMessage}, kept separate so the callback also runs without the
     * monitor (§7.2.4).
     */
    void deliverWire(org.zerodds.rtps.SerializedSample sample) {
        T value = org.zerodds.rtps.DdsWireBridge.decode(typeSupport, sample);
        byte[] keyHash = typeSupport.isKeyed() ? typeSupport.keyHash(value) : new byte[0];
        // publication_handle: the remote writer, named by its GUID as get_matched_publications
        // names it; source_timestamp: the one the writer sent (INFO_TS), else the arrival time.
        InstanceHandle publication = sample.identity() == null
                ? Values.nil() : Values.of(env, sample.identity().guid());
        long sourceMillis = sample.sourceTimestampMillis() >= 0
                ? sample.sourceTimestampMillis() : System.currentTimeMillis();
        InProcessBus.Message m = new InProcessBus.Message(sample.body(), keyHash,
                InProcessBus.ChangeKind.WRITE, WIRE_WRITER_ID, 0, false,
                new ArrayList<>(partitions), sourceMillis, publication,
                sample.identity(), sample.relatedIdentity());
        boolean dataAvailable;
        synchronized (this) {
            noteSequence(sample.identity());
            dataAvailable = onWrite(m, new InstanceKey(keyHash), () -> value);
        }
        if (dataAvailable) {
            fireDataAvailable();
        }
        fireRejectedAndLost();
    }

    /**
     * DDS 1.4 §2.2.3.13: the default partition is the name ""; a name may be a
     * pattern with {@code *} and {@code ?} (fnmatch), and two sets overlap when
     * a name of one matches a name of the other.
     */
    private boolean partitionOverlaps(List<String> writerPartitions) {
        return org.zerodds.rtps.QosParameters.partitionsOverlap(new java.util.ArrayList<>(partitions), writerPartitions);
    }

    // ---- read / take -----------------------------------------------------

    /**
     * Spec §2.2.2.5.3.8 read — snapshot without removing.
     *
     * <p>§7.7.3's two flavours: an iterator over the samples this binding
     * collected, and a copy into the list the application supplied.
     */
    @Override
    public Sample.Iterator<T> read() {
        return new SampleIterator<>(collect(false, null));
    }

    @Override
    public Sample.Iterator<T> read(int maxSamples) {
        return new SampleIterator<>(collect(false, select().maxSamples(maxSamples)));
    }

    @Override
    public Sample.Iterator<T> read(Selector<T> query) {
        return new SampleIterator<>(collect(false, query));
    }

    @Override
    public List<Sample<T>> read(List<Sample<T>> samples) {
        return fill(samples, collect(false, null));
    }

    @Override
    public List<Sample<T>> read(List<Sample<T>> samples, Selector<T> selector) {
        return fill(samples, collect(false, selector));
    }

    /**
     * DDS-Java-PSM 1.0 §7.7.3: the list holds the samples the operation returned, whatever it
     * held before (a trimmed or extended list); a {@code null} list is a new one.
     */
    private static <E> List<E> fill(List<E> target, List<E> collected) {
        List<E> list = target == null ? new ArrayList<E>() : target;
        list.clear();
        list.addAll(collected);
        return list;
    }

    /** Spec §2.2.2.5.3.9 take — remove and return. */
    @Override
    public Sample.Iterator<T> take() {
        return new SampleIterator<>(collect(true, null));
    }

    @Override
    public Sample.Iterator<T> take(int maxSamples) {
        return new SampleIterator<>(collect(true, select().maxSamples(maxSamples)));
    }

    @Override
    public Sample.Iterator<T> take(Selector<T> query) {
        return new SampleIterator<>(collect(true, query));
    }

    @Override
    public List<Sample<T>> take(List<Sample<T>> samples) {
        return fill(samples, collect(true, null));
    }

    @Override
    public List<Sample<T>> take(List<Sample<T>> samples, Selector<T> query) {
        return fill(samples, collect(true, query));
    }

    /** The selector of read_next_sample and take_next_sample: one sample not yet read. */
    private SelectorImpl<T> nextSampleSelector() {
        DataStateImpl notRead = new DataStateImpl(env).with(SampleState.NOT_READ)
                .withAnyViewState().withAnyInstanceState();
        return select().dataState(notRead).maxSamples(1);
    }

    @Override
    public boolean readNextSample(Sample<T> sample) {
        org.zerodds.core.Args.require(sample, "sample");
        List<Sample<T>> out = collect(false, nextSampleSelector());
        if (out.isEmpty()) {
            return false;
        }
        copyInto(out.get(0), sample);
        return true;
    }

    @Override
    public boolean takeNextSample(Sample<T> sample) {
        org.zerodds.core.Args.require(sample, "sample");
        List<Sample<T>> out = collect(true, nextSampleSelector());
        if (out.isEmpty()) {
            return false;
        }
        copyInto(out.get(0), sample);
        return true;
    }

    /** Overwrites the content of {@code target} with that of {@code source}. */
    @SuppressWarnings("unchecked")
    private void copyInto(Sample<T> source, Sample<T> target) {
        if (!(target instanceof SampleImpl)) {
            throw new IllegalArgumentException(
                    "the sample to fill must be a org.zerodds.sub.SampleImpl");
        }
        ((SampleImpl<T>) source).copyInto((SampleImpl<T>) target);
    }

    /**
     * The one place read and take differ: {@code remove} decides whether the
     * samples stay in the reader's history.
     */
    private List<Sample<T>> collect(boolean remove, Selector<T> query) {
        return collect(remove, query, true);
    }

    /**
     * Takes every sample, as {@link #take(List)} does, without making a value of any: each
     * sample is a {@link SampleImpl} that carries the octets it arrived as
     * ({@link SampleImpl#serialized()}) and no data. For a reader whose type support retains
     * the octets ({@code TopicTypeSupport.retainsSerialized()}): the DDS-RPC 1.0 binding
     * (§7.11.2) reads its requests and replies from them, through the reader it hands the
     * application typed by the topic type.
     *
     * @throws IllegalStateException when the type support does not retain the octets
     */
    public List<Sample<T>> takeSerialized() {
        if (!typeSupport.retainsSerialized()) {
            throw new IllegalStateException(
                    "the type support of " + typeSupport.getTypeName() + " keeps no octets");
        }
        return collect(true, null, false);
    }

    private List<Sample<T>> collect(boolean remove, Selector<T> query, boolean values) {
        if (!internal && parent instanceof SubscriberImpl) {
            ((SubscriberImpl) parent).requireGroupAccess();
        }
        // reading or taking resets DATA_AVAILABLE (DDS 1.4 §2.2.4.1), and DATA_ON_READERS
        // of the subscriber
        clearStatus(DATA_AVAILABLE);
        if (parent instanceof SubscriberImpl) {
            ((SubscriberImpl) parent).dataOnReadersAccessed();
        }
        // outermost: the delivery of a coherent set holds the gate and then the monitors of the
        // readers, so a reader never takes the gate while it holds one of them
        java.util.concurrent.locks.Lock gate = InProcessBus.instance().readerGate();
        gate.lock();
        try {
            java.util.concurrent.locks.Lock access = accessLock();
            access.lock();
            try {
                return collectLocked(remove, query, values);
            } finally {
                access.unlock();
            }
        } finally {
            gate.unlock();
        }
    }

    /** A sample chosen by a read or take, with the instance it belongs to. */
    private static final class Pick {
        final InstanceKey key;
        final Instance instance;
        final Stored<Object> stored;
        final ViewState view;
        final SampleState sampleState;

        Pick(InstanceKey key, Instance instance, Stored<Object> stored, ViewState view) {
            this.key = key;
            this.instance = instance;
            this.stored = stored;
            this.view = view;
            this.sampleState = stored.sampleState;
        }

        int generation() {
            return stored.disposedGeneration + stored.noWritersGeneration;
        }
    }

    private synchronized List<Sample<T>> collectLocked(boolean takeThem, Selector<T> query,
            boolean values) {
        ensureEnabled();
        purgeLocked();
        SelectorImpl<T> selector = query == null ? null : (SelectorImpl<T>) query;
        Predicate<T> content = null;
        if (selector != null && selector.getQueryExpression() != null) {
            // §7.7.3: the query expression of a Selector narrows the samples
            // the same way a ContentFilteredTopic does.
            content = ContentFilter.compile(selector.getQueryExpression(),
                    selector.getQueryParameters());
        }
        int max = selector == null || selector.getMaxSamples() < 0
                ? Integer.MAX_VALUE : selector.getMaxSamples();
        org.omg.dds.core.policy.Presentation scope = parent == null
                ? null : parent.getQos().getPresentation();
        if (scope != null && scope.isOrderedAccess() && scope.getAccessScope()
                == org.omg.dds.core.policy.Presentation.AccessScopeKind.GROUP) {
            // DDS 1.4 2.2.2.5.3.8: with access_scope GROUP and ordered_access the collection holds at
            // most one sample, so the application can read the samples of several readers in order
            max = Math.min(max, 1);
        }
        Set<SampleState> sampleStates = selector == null
                ? java.util.EnumSet.allOf(SampleState.class)
                : new LinkedHashSet<>(selector.sampleStates());
        Set<ViewState> viewStates = selector == null
                ? java.util.EnumSet.allOf(ViewState.class)
                : new LinkedHashSet<>(selector.viewStates());
        Set<InstanceState> instanceStates = selector == null
                ? java.util.EnumSet.allOf(InstanceState.class)
                : new LinkedHashSet<>(selector.instanceStates());
        // read_w_condition / take_w_condition (DDS 1.4 §2.2.2.5.3.8): the states and, for a
        // QueryCondition, the expression of a condition of this reader narrow the samples further.
        ReadCondition<T> condition = selector == null ? null : selector.getCondition();
        if (condition != null) {
            if (condition instanceof ReadConditionImpl) {
                ((ReadConditionImpl<T>) condition).ensureOpen();
            }
            if (condition.getParent() != this) {
                throw new PreconditionNotMetException(
                        "the ReadCondition belongs to another DataReader");
            }
            sampleStates.retainAll(condition.getSampleStates());
            viewStates.retainAll(condition.getViewStates());
            instanceStates.retainAll(condition.getInstanceStates());
            if (condition instanceof QueryCondition) {
                QueryCondition<T> q = (QueryCondition<T>) condition;
                Predicate<T> byCondition = ContentFilter.compile(q.getQueryExpression(),
                        q.getQueryParameters());
                content = content == null ? byCondition : content.and(byCondition);
            }
        }

        // The instances in the order they were first seen, or in the order of their handles
        // when the next instance is asked for (§2.2.2.5.3.11: a total order of the handles).
        InstanceHandle wanted = selector == null ? null : selector.getInstance();
        boolean anyInstance = wanted == null || wanted.isNil();
        boolean next = selector != null && selector.retrieveNextInstance();
        List<Map.Entry<InstanceKey, Instance>> order = new ArrayList<>(instances.entrySet());
        if (next) {
            java.util.Collections.sort(order, (x, y) ->
                    InstanceKey.toHandle(x.getKey()).compareTo(InstanceKey.toHandle(y.getKey())));
        }
        org.omg.dds.core.policy.Presentation presentation = parent == null
                ? null : parent.getQos().getPresentation();
        boolean acrossInstances = presentation != null && presentation.isOrderedAccess()
                && presentation.getAccessScope()
                        != org.omg.dds.core.policy.Presentation.AccessScopeKind.INSTANCE;

        List<Pick> picks = new ArrayList<>();
        for (Map.Entry<InstanceKey, Instance> entry : order) {
            Instance inst = entry.getValue();
            InstanceHandle handle = InstanceKey.toHandle(entry.getKey());
            if (!anyInstance) {
                int compared = handle.compareTo(wanted);
                if (next ? compared <= 0 : compared != 0) {
                    continue;
                }
            }
            ViewState view = inst.viewNew ? ViewState.NEW : ViewState.NOT_NEW;
            if (!viewStates.contains(view) || !instanceStates.contains(inst.state)) {
                continue;
            }
            int before = picks.size();
            for (Stored<Object> stored : inst.samples) {
                if (!acrossInstances && picks.size() >= max) {
                    break;
                }
                if (!sampleStates.contains(stored.sampleState)) {
                    continue;
                }
                if (content != null && (stored.data == null || !content.test(dataOf(stored)))) {
                    continue;
                }
                picks.add(new Pick(entry.getKey(), inst, stored, view));
            }
            if (next && picks.size() > before) {
                break; // the next instance with samples, and only that one
            }
            if (!acrossInstances && picks.size() >= max) {
                break;
            }
        }
        if (acrossInstances) {
            // ordered access with scope TOPIC or GROUP (§2.2.3.6): the samples of all instances
            // in one order, the one DESTINATION_ORDER gives.
            final boolean bySource = qos.getDestinationOrder().getKind()
                    == org.omg.dds.core.policy.DestinationOrder.Kind.BY_SOURCE_TIMESTAMP;
            java.util.Collections.sort(picks, (x, y) -> {
                if (bySource && x.stored.sourceMillis != y.stored.sourceMillis) {
                    return Long.compare(x.stored.sourceMillis, y.stored.sourceMillis);
                }
                return Long.compare(x.stored.arrival, y.stored.arrival);
            });
            while (picks.size() > max) {
                picks.remove(picks.size() - 1);
            }
        }

        // DDS 1.4 §2.2.2.5.1.6: sample_rank counts the samples of the same instance after this
        // one, generation_rank is measured against the last sample of the instance in the
        // collection, absolute_generation_rank against the instance as the reader holds it.
        int count = picks.size();
        int[] sampleRank = new int[count];
        int[] generationRank = new int[count];
        Map<Instance, Integer> following = new java.util.IdentityHashMap<>();
        Map<Instance, Integer> lastGeneration = new java.util.IdentityHashMap<>();
        for (int i = count - 1; i >= 0; i--) {
            Pick pick = picks.get(i);
            Integer seen = following.get(pick.instance);
            sampleRank[i] = seen == null ? 0 : seen;
            following.put(pick.instance, sampleRank[i] + 1);
            if (seen == null) {
                lastGeneration.put(pick.instance, pick.generation());
            }
            generationRank[i] = lastGeneration.get(pick.instance) - pick.generation();
        }
        List<Sample<T>> out = new ArrayList<>(count);
        for (int i = 0; i < count; i++) {
            Pick pick = picks.get(i);
            int absoluteRank = pick.instance.disposedGeneration + pick.instance.noWritersGeneration
                    - pick.generation();
            out.add(cast(toSample(pick, sampleRank[i], generationRank[i], absoluteRank, values)));
        }
        // The access marks the samples READ and the instances NOT_NEW, after the snapshot of
        // all states above (§2.2.2.5.1.2, §2.2.2.5.1.8).
        for (Pick pick : picks) {
            pick.stored.sampleState = SampleState.READ;
            pick.instance.viewNew = false;
            if (takeThem) {
                pick.instance.samples.remove(pick.stored);
            }
        }
        if (takeThem) {
            for (Pick pick : picks) {
                reclaim(pick.key);
            }
        }
        return out;
    }

    @Override
    public SelectorImpl<T> select() {
        return new SelectorImpl<T>(env).reader(self());
    }

    /** The read of §7.7.3 the selector performs itself. */
    public org.omg.dds.sub.Sample.Iterator<T> readWith(SelectorImpl<T> selector,
            List<Sample<T>> into) {
        List<Sample<T>> out = collect(false, selector);
        if (into != null) {
            into.clear();
            into.addAll(out);
        }
        return new SampleIterator<>(out);
    }

    /** The take of §7.7.3 the selector performs itself. */
    public SampleIterator<T> takeWith(SelectorImpl<T> selector, List<Sample<T>> into) {
        List<Sample<T>> out = collect(true, selector);
        if (into != null) {
            into.clear();
            into.addAll(out);
        }
        return new SampleIterator<>(out);
    }

    /**
     * The subscriber's access scope (DDS 1.2 §2.2.2.2.3): a read or take
     * holds it, so {@code begin_access}/{@code end_access} on the subscriber
     * holds every reader of that subscriber still.
     */
    private java.util.concurrent.locks.Lock accessLock() {
        if (parent instanceof org.zerodds.sub.SubscriberImpl) {
            org.zerodds.sub.SubscriberImpl subscriber = (org.zerodds.sub.SubscriberImpl) parent;
            return subscriber.accessLock();
        }
        return NO_ACCESS;
    }

    private static final java.util.concurrent.locks.Lock NO_ACCESS =
            new java.util.concurrent.locks.ReentrantLock();

    /**
     * RETCODE_ALREADY_DELETED / RETCODE_NOT_ENABLED as exceptions
     * (DDS-Java-PSM 1.0 §7.3.2, Table 7.1): a read or take on a closed or not
     * yet enabled reader fails; it does not report an empty collection that
     * cannot be told apart from "no data".
     */
    private void ensureEnabled() {
        if (closed) {
            throw new AlreadyClosedException("DataReader is closed");
        }
        if (!enabled) {
            throw new NotEnabledException("DataReader is not enabled");
        }
    }

    /** This reader as the interface of §7.7.3. */
    @SuppressWarnings("unchecked")
    private DataReader<T> self() {
        return (DataReader<T>) this;
    }

    /**
     * The stored value as the reader's type. A reader stores its samples as
     * {@code Object} (the store is shared with {@link #cast()}); every value
     * in it was written through a writer of this reader's topic, whose type is
     * {@code T}.
     */
    @SuppressWarnings("unchecked")
    private T dataOf(Stored<Object> stored) {
        return (T) stored.data;
    }

    @SuppressWarnings("unchecked")
    private Sample<T> cast(Sample<Object> s) {
        return (Sample<T>) (Sample<?>) s;
    }

    /**
     * Snapshots a picked sample for return by read/take. sample_state is the sample's state
     * before this access (§2.2.2.5.1.2); view_state and instance_state are the instance's
     * state at collection time, the same for every sample of the instance in the collection
     * (§2.2.2.5.1.3, §2.2.2.5.1.8). The data is deep-copied so an application mutating a
     * returned sample does not corrupt the reader's internal state (§7.7.3, the "copy" flavor).
     */
    private Sample<Object> toSample(Pick pick, int sampleRank, int generationRank,
            int absoluteGenerationRank, boolean values) {
        Stored<Object> s = pick.stored;
        // A value decoded again from the octets it arrived as is a copy by construction.
        Object data = !values ? null
                : s.serialized != null && s.data != null ? decodeRetained(s.serialized)
                : deepCopy(s.data);
        SampleImpl<Object> out = SampleImpl.of(env, data, s.instanceHandle,
                s.publicationHandle, s.sourceTimestamp, pick.sampleState, pick.view,
                pick.instance.state, s.disposedGeneration, s.noWritersGeneration,
                sampleRank, generationRank, absoluteGenerationRank);
        out.setIdentity(s.identity, s.related);
        out.setSerialized(s.serialized);
        return out;
    }

    /** The value of retained octets, decoded as on arrival. */
    private Object decodeRetained(byte[] octets) {
        return typeSupport.deserialize(Xcdr2Codec.decoder(octets));
    }

    /**
     * Deep-copies sample data via the TypeSupport (serialize + deserialize),
     * so read/take never hand out the reader's own object (§7.7.3).
     */
    private Object deepCopy(Object data) {
        if (data == null) {
            return null;
        }
        @SuppressWarnings("unchecked")
        T value = (T) data;
        int capacity = 256;
        while (true) {
            ByteBuffer encoder = Xcdr2Codec.encoder(capacity);
            try {
                typeSupport.serialize(value, encoder);
                byte[] bytes = Xcdr2Codec.copyToBytes(encoder);
                return typeSupport.deserialize(Xcdr2Codec.decoder(bytes));
            } catch (java.nio.BufferOverflowException overflow) {
                if (capacity >= (1 << 28)) {
                    throw overflow; // refuse to grow past 256 MiB.
                }
                capacity <<= 1;
            }
        }
    }

    /**
     * §7.7.3: the topic description this reader reads. A reader of a
     * content-filtered topic reports the filter, not the plain topic.
     */
    @Override
    public TopicDescription<T> getTopicDescription() {
        TopicDescription<T> described = description;
        return described != null ? described : topic;
    }

    /**
     * Records the topic description this reader was created with when it is
     * not the topic it reads (a ContentFilteredTopic, a MultiTopic).
     */
    public void describedBy(TopicDescription<T> description) {
        this.description = description;
    }

    /** Runs {@code hook} when this reader is closed (the join of a MultiTopic ends with it). */
    public void onClose(Runnable hook) {
        this.closeHook = hook;
    }

    /**
     * Whether this reader holds a sample the given data state selects on —
     * {@code get_dataReaders(readers, data_state)} of §7.7.4.
     */
    public boolean hasMatchingSample(Subscriber.DataState state) {
        return anySample(state, null);
    }

    /**
     * The order keys of the samples {@code state} selects: for each, the source timestamp (when this
     * reader orders BY_SOURCE_TIMESTAMP, else {@code Long.MIN_VALUE}) and the arrival number. A
     * subscriber with access_scope GROUP and ordered_access puts the keys of its readers in one
     * order (DDS 1.4 2.2.2.5.2.10).
     */
    public synchronized List<long[]> orderKeys(Subscriber.DataState state) {
        purgeLocked();
        boolean bySource = qos.getDestinationOrder().getKind()
                == org.omg.dds.core.policy.DestinationOrder.Kind.BY_SOURCE_TIMESTAMP;
        List<long[]> keys = new ArrayList<>();
        for (Instance inst : instances.values()) {
            if (state != null) {
                if (!state.getInstanceStates().contains(inst.state)) {
                    continue;
                }
                ViewState view = inst.viewNew ? ViewState.NEW : ViewState.NOT_NEW;
                if (!state.getViewStates().contains(view)) {
                    continue;
                }
            }
            for (Stored<Object> stored : inst.samples) {
                if (state != null && !state.getSampleStates().contains(stored.sampleState)) {
                    continue;
                }
                keys.add(new long[] {bySource ? stored.sourceMillis : Long.MIN_VALUE, stored.arrival});
            }
        }
        return keys;
    }

    /**
     * Whether a sample the reader holds has a state in the given sets and, when {@code matches}
     * is given, satisfies it (DDS-DCPS 1.4 §2.2.2.5.3.6: the trigger value of a read condition
     * is "there is at least one sample in the states", of a query condition "...and the
     * query expression is true for it"). A sample without data satisfies no expression.
     */
    private synchronized boolean anySample(Subscriber.DataState state, Predicate<T> matches) {
        purgeLocked();
        for (Instance inst : instances.values()) {
            if (state != null) {
                if (!state.getInstanceStates().contains(inst.state)) {
                    continue;
                }
                ViewState view = inst.viewNew ? ViewState.NEW : ViewState.NOT_NEW;
                if (!state.getViewStates().contains(view)) {
                    continue;
                }
            }
            for (Stored<Object> stored : inst.samples) {
                if (state != null && !state.getSampleStates().contains(stored.sampleState)) {
                    continue;
                }
                if (matches != null && (stored.data == null || !matches.test(dataOf(stored)))) {
                    continue;
                }
                return true;
            }
        }
        return false;
    }

    /**
     * §7.7.4 {@code notify_datareaders} (DDS 1.4 §2.2.2.5.2.7): invokes {@code onDataAvailable}
     * on the listener attached to this reader, if its DATA_AVAILABLE status is changed, that is,
     * if the reader holds samples nobody has read or taken since they arrived. It does not
     * go through the listener of the Subscriber, which is what calls this operation.
     */
    public void notifyDataAvailable() {
        if (!getStatusChanges().contains(DATA_AVAILABLE)) {
            return;
        }
        DataReaderListener<T> l = ownListener(DATA_AVAILABLE);
        if (l != null) {
            l.onDataAvailable(new DataAvailableEventImpl<>(
                    env, this, new DataAvailableStatusImpl(env)));
        }
    }

    /** Spec §2.2.2.5.3.30 lookup_instance. */
    @Override
    public synchronized InstanceHandle lookupInstance(T instanceData) {
        if (instanceData == null) {
            throw new IllegalArgumentException("the sample whose instance is looked up is null");
        }
        InstanceKey key = new InstanceKey(
                typeSupport.isKeyed() ? typeSupport.keyHash(instanceData) : new byte[0]);
        return instances.containsKey(key) ? InstanceKey.toHandle(key) : Values.nil();
    }

    // ---- statuses --------------------------------------------------------

    /**
     * Spec §2.2.4.1 REQUESTED_DEADLINE_MISSED. Evaluated by the lease
     * scheduler (and, on demand, here): for each ALIVE instance whose
     * wall-clock gap since its last sample exceeds the requested DEADLINE
     * period, count the elapsed whole periods as misses (each period charged at
     * most once). The listener is alerted asynchronously by the lease.
     */
    public synchronized RequestedDeadlineMissedStatus getRequestedDeadlineMissedStatus() {
        clearStatus(REQUESTED_DEADLINE_MISSED);
        evaluateDeadline();
        int change = deadlineMissTotal - deadlineMissReadBaseline;
        deadlineMissReadBaseline = deadlineMissTotal;
        return new RequestedDeadlineMissedStatusImpl(
                env, deadlineMissTotal, change, lastDeadlineMissHandle);
    }

    private void evaluateDeadline() {
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
        synchronized (this) {
            for (Map.Entry<InstanceKey, Instance> e : instances.entrySet()) {
                Instance inst = e.getValue();
                if (inst.state != InstanceState.ALIVE) continue;
                long elapsed = now - inst.lastSampleMillis;
                int periodsElapsed = (int) (elapsed / periodMs);
                if (periodsElapsed > inst.deadlinePeriodsCharged) {
                    deadlineMissTotal += periodsElapsed - inst.deadlinePeriodsCharged;
                    inst.deadlinePeriodsCharged = periodsElapsed;
                    lastDeadlineMissHandle = InstanceKey.toHandle(e.getKey());
                    missed = true;
                }
            }
        }
        if (missed) {
            triggerStatus(REQUESTED_DEADLINE_MISSED);
            fireDeadlineMissed();
        }
    }

    /** Hands the REQUESTED_DEADLINE_MISSED status to the listener outside the monitor. */
    private void fireDeadlineMissed() {
        DataReaderListener<T> l;
        RequestedDeadlineMissedStatus status;
        synchronized (this) {
            l = listenerFor(REQUESTED_DEADLINE_MISSED);
            if (l == null) {
                return; // the counts stay pending until the status is read
            }
            int change = deadlineMissTotal - deadlineMissReadBaseline;
            deadlineMissReadBaseline = deadlineMissTotal;
            status = new RequestedDeadlineMissedStatusImpl(
                    env, deadlineMissTotal, change, lastDeadlineMissHandle);
        }
        // The callback runs with no monitor held, so a lease thread cannot block
        // a concurrent call on any entity (DDS-Java-PSM 1.0 §7.2.4).
        l.onRequestedDeadlineMissed(new RequestedDeadlineMissedEventImpl<>(env, this, status));
    }

    /**
     * Spec §2.2.4.1 SUBSCRIPTION_MATCHED. The counts are maintained by the
     * bus's match notification, so reading the status does not walk the bus.
     */
    public synchronized SubscriptionMatchedStatus getSubscriptionMatchedStatus() {
        clearStatus(SUBSCRIPTION_MATCHED);
        return readSubscriptionMatchedStatusLocked();
    }

    private SubscriptionMatchedStatus readSubscriptionMatchedStatusLocked() {
        int current = matchedWriters.size();
        int totalChange = subMatchedTotal - subMatchedTotalReadBaseline;
        int currentChange = current - subMatchedCurrentReadBaseline;
        subMatchedTotalReadBaseline = subMatchedTotal;
        subMatchedCurrentReadBaseline = current;
        return new SubscriptionMatchedStatusImpl(env, subMatchedTotal, totalChange, current,
                currentChange, lastMatchedHandle);
    }

    /**
     * Spec §2.2.4.1 REQUESTED_INCOMPATIBLE_QOS. Recorded when a writer with an
     * incompatible offered QoS attempted to match (see
     * {@link #onWriterPresenceChanged}).
     */
    public synchronized RequestedIncompatibleQosStatus getRequestedIncompatibleQosStatus() {
        clearStatus(REQUESTED_INCOMPATIBLE);
        int change = requestedIncompatTotal - requestedIncompatReadBaseline;
        requestedIncompatReadBaseline = requestedIncompatTotal;
        Set<QosPolicyCount> policies = new LinkedHashSet<>();
        for (Map.Entry<Class<? extends QosPolicy>, Integer> e
                : requestedIncompatPolicies.entrySet()) {
            policies.add(new QosPolicyCountImpl(env, e.getKey(), e.getValue()));
        }
        return new RequestedIncompatibleQosStatusImpl(
                env, requestedIncompatTotal, change, lastRequestedIncompatPolicyClass, policies);
    }

    /**
     * DDS 1.4 §2.2.4.1: one refused connection counts once in total_count, and once in the
     * QosPolicyCount of each policy that refused it; last_policy_id is one of them.
     */
    private void recordRequestedIncompatibleLocked(List<Class<? extends QosPolicy>> policies) {
        triggerStatus(REQUESTED_INCOMPATIBLE);
        requestedIncompatTotal++;
        for (Class<? extends QosPolicy> policyClass : policies) {
            lastRequestedIncompatPolicyClass = policyClass;
            requestedIncompatPolicies.merge(policyClass, 1, Integer::sum);
        }
    }

    /**
     * Spec §2.2.4.1 LIVELINESS_CHANGED. Counts matched writers (partition +
     * RxO compatible) that are currently asserting liveliness vs. not. The
     * listener is alerted asynchronously by the lease when a writer's
     * {@code isAlive()} state transitions.
     */
    public synchronized LivelinessChangedStatus getLivelinessChangedStatus() {
        clearStatus(LIVELINESS_CHANGED);
        int[] counts = countLivelinessLocked();
        int alive = counts[0];
        int notAlive = counts[1];
        int aliveChange = alive - livAliveReadBaseline;
        int notAliveChange = notAlive - livNotAliveReadBaseline;
        livAliveReadBaseline = alive;
        livNotAliveReadBaseline = notAlive;
        return new LivelinessChangedStatusImpl(
                env, alive, notAlive, aliveChange, notAliveChange, lastLivelinessHandle);
    }

    /**
     * Spec §2.1.4.1 LIVELINESS_CHANGED — counts the matched writers whose
     * liveliness state changed since the status was last reported, and hands
     * the status to the listener <em>outside</em> the monitor (§7.2.4).
     */
    private void evaluateLiveliness(boolean fire) {
        DataReaderListener<T> l;
        LivelinessChangedStatus status;
        synchronized (this) {
            int[] counts = countLivelinessLocked();
            int alive = counts[0];
            int notAlive = counts[1];
            int aliveChange = alive - livAliveReadBaseline;
            int notAliveChange = notAlive - livNotAliveReadBaseline;
            if (!fire || (aliveChange == 0 && notAliveChange == 0)) {
                return;
            }
            l = listenerFor(LIVELINESS_CHANGED);
            if (l == null) {
                return; // the counts stay pending until the status is read
            }
            livAliveReadBaseline = alive;
            livNotAliveReadBaseline = notAlive;
            status = new LivelinessChangedStatusImpl(
                    env, alive, notAlive, aliveChange, notAliveChange, lastLivelinessHandle);
        }
        l.onLivelinessChanged(new LivelinessChangedEventImpl<>(env, this, status));
    }

    private int[] countLivelinessLocked() {
        int alive = 0, notAlive = 0;
        for (InProcessBus.WriterPresence w : InProcessBus.instance().writers(
                domainId, topic.getName(), typeSupport.getTypeName())) {
            // only writers matched with this reader count: PARTITION overlaps and the offered
            // QoS is compatible (§2.2.4.1 LIVELINESS_CHANGED)
            if (!matchedWriters.contains(w.writerId())) {
                continue;
            }
            boolean isAlive = w.isAlive();
            Boolean seen = livelinessSeen.put(w.writerId(), isAlive);
            if (seen == null || seen.booleanValue() != isAlive) {
                lastLivelinessHandle = handleOf(w);
            }
            if (isAlive) alive++;
            else notAlive++;
        }
        // writers of other processes: alive within the lease they announced, by the evidence
        // the RTPS reader has of them (DATA, HEARTBEAT with the L flag, messages of the participant)
        org.zerodds.rtps.RtpsParticipant.WireReader remote = wire;
        if (remote != null) {
            for (org.zerodds.rtps.RtpsParticipant.RemoteLiveliness r : remote.remoteLiveliness()) {
                String key = java.util.Arrays.toString(r.guid);
                Boolean seen = remoteLivelinessSeen.put(key, r.alive);
                if (seen == null || seen.booleanValue() != r.alive) {
                    lastLivelinessHandle = new org.zerodds.core.InstanceHandleImpl(env, r.guid);
                }
                if (r.alive) alive++;
                else notAlive++;
            }
        }
        return new int[]{alive, notAlive};
    }

    public Topic<T> getTopic() {
        return topic;
    }

    public DataReaderQos getQos() {
        return qos;
    }

    /**
     * DDS 1.4 §2.2.3: once the reader is enabled only the changeable policies can
     * change ({@link org.zerodds.core.qos.QosRules}); the leases follow a changed
     * DEADLINE or LIVELINESS.
     */
    public void setQos(DataReaderQos qos) {
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
            // The built-in topic data of the reader carries its QoS (DDS 1.4 §2.2.5): the
            // changed QoS is a new sample of the same instance. Outside the monitor, because
            // the built-in readers it reaches run listeners.
            announceQos();
        }
    }

    /** The QoS of this reader is written to the "DCPSSubscription" built-in topic again. */
    public void announceQos() {
        if (parent != null && parent.getParent() != null && parent instanceof SubscriberImpl) {
            org.zerodds.domain.Discovery.readerCreated(parent.getParent(), this, parent.getQos());
        }
        // the writers of other processes learn the changed QoS from SEDP
        org.zerodds.rtps.RtpsParticipant.WireReader remote = wire;
        if (remote != null && wireQos() != null) {
            remote.updateQos(wireQos());
        }
    }

    /**
     * DDS-Java-PSM 1.0 §7.2.4 — the state transition runs under this reader's
     * monitor, the bus registration and the late-join replay (which deliver
     * samples and therefore raise listener callbacks) run without it, so a
     * concurrent call on any entity is never blocked by this one.
     */
    @Override
    public void enable() {
        List<InProcessBus.WriterPresence> present;
        synchronized (this) {
            if (closed) {
                throw new AlreadyClosedException("DataReader is closed");
            }
            if (enabled) {
                return; // enable is idempotent (§7.3.6)
            }
            // DDS 1.4 §2.2.2.1.1.7: enable on an Entity whose factory is not enabled fails.
            if (parent instanceof SubscriberImpl && !((SubscriberImpl) parent).isEnabled()) {
                throw new PreconditionNotMetException("the Subscriber of the DataReader is not enabled");
            }
            if (org.zerodds.rtps.DdsWireBridge.enabled()
                    && !org.zerodds.topic.BuiltinTopics.isInternalToProcess(topic.getName())) {
                wire = org.zerodds.rtps.DdsWireBridge.typedReader(domainId, topic.getName(),
                        typeSupport.getTypeName(), typeSupport, this::deliverWire, wireQos(),
                        coercesTypes() ? this::acceptsRemoteWriter : null);
                wire.setLivelinessListener(() -> evaluateLiveliness(true));
                if (!rpc.isEmpty()) {
                    wire.setRpcDiscovery(rpc);
                }
            }
            enabled = true;
            startLeasesLocked();
            present = InProcessBus.instance().registerReader(
                    domainId, topic.getName(), typeSupport.getTypeName(), this);
        }
        // TRANSIENT_LOCAL late-join replay (§2.2.3.4): pull retained samples
        // from already-present, partition-matching writers.
        for (InProcessBus.WriterPresence w : present) {
            if (partitionOverlaps(w.partitions())) {
                w.replayTo(this);
            }
        }
        // TRANSIENT / PERSISTENT late-join replay (DDS 1.2 §7.1.3.4): the
        // DurabilityService holds samples that survive their writer.
        if (qos.getDurability().getKind() != Durability.Kind.VOLATILE) {
            DurabilityService.instance().replay(domainId, topic.getName(),
                    typeSupport.getTypeName(), this, qos.getDurability().getKind());
        }
        if (parent instanceof SubscriberImpl) {
            ((SubscriberImpl) parent).readerEnabled(this);
        }
    }

    public boolean isEnabled() {
        return enabled;
    }

    /**
     * Schedules the deadline and liveliness leases (DDS 1.2 §7.1.4.2). An
     * infinite deadline or liveliness lease needs no timer.
     */
    private synchronized void startLeasesLocked() {
        startSweepLocked();
        Duration deadlinePeriod = qos.getDeadline().getPeriod();
        if (!deadlinePeriod.isInfinite()
                && deadlinePeriod.getDuration(TimeUnit.MILLISECONDS) > 0) {
            deadlineLease = LeaseMonitor.scheduleAtFixedRate(
                    () -> evaluateDeadline(),
                    deadlinePeriod.getDuration(TimeUnit.MILLISECONDS));
        }
        Duration lease = qos.getLiveliness().getLeaseDuration();
        if (!lease.isInfinite() && lease.getDuration(TimeUnit.MILLISECONDS) > 0) {
            livelinessLease = LeaseMonitor.scheduleAtFixedRate(
                    () -> evaluateLiveliness(true),
                    lease.getDuration(TimeUnit.MILLISECONDS));
        }
    }

    @Override
    public void close() {
        boolean unregister;
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
            if (sweepLease != null) {
                sweepLease.cancel(false);
                sweepLease = null;
            }
        }
        if (unregister) {
            InProcessBus.instance().unregisterReader(
                    domainId, topic.getName(), typeSupport.getTypeName(), this);
        }
        for (ReadConditionImpl<T> condition : new ArrayList<>(conditions)) {
            condition.close();
        }
        Runnable hook = closeHook;
        if (hook != null) {
            hook.run();
        }
        runCloseHooks();
    }

    public boolean isClosed() {
        return closed;
    }

    // ---- Remaining DCPS operations (§7.7.3) ------------------------------

    @SuppressWarnings("unchecked")
    @Override
    public <OTHER> DataReader<OTHER> cast() {
        return (DataReader<OTHER>) this;
    }

    /** The conditions of this reader are the only entities it contains; they are closed. */
    @Override
    public void closeContainedEntities() {
        for (ReadConditionImpl<T> condition : new ArrayList<>(conditions)) {
            condition.close();
        }
    }

    private <C extends ReadConditionImpl<T>> C register(C condition) {
        if (closed) {
            throw new AlreadyClosedException("DataReader is closed");
        }
        conditions.add(condition);
        condition.onClose(closing -> conditions.remove(closing));
        return condition;
    }

    /**
     * §7.7.3 {@code create_read_condition}. The condition is true while this
     * reader holds a sample the three state sets select on.
     */
    @Override
    public ReadCondition<T> createReadCondition(Subscriber.DataState states) {
        org.zerodds.core.Args.require(states, "states");
        final Subscriber.DataState fixed = snapshot(states);
        return register(new ReadConditionImpl<>(env, self(), fixed.getSampleStates(),
                fixed.getViewStates(), fixed.getInstanceStates(),
                () -> anySample(fixed, null)));
    }

    @Override
    public QueryCondition<T> createQueryCondition(String queryExpression,
            List<String> queryParameters) {
        org.zerodds.core.Args.require(queryExpression, "queryExpression");
        return createQueryCondition(subscriberDataState(), queryExpression, queryParameters);
    }

    /**
     * §7.7.3 {@code create_querycondition}: true while one sample both is in the state sets and
     * satisfies the expression, with the parameters the condition holds at that moment
     * ({@code setQueryParameters} takes effect).
     */
    @Override
    public QueryCondition<T> createQueryCondition(Subscriber.DataState states,
            String queryExpression, List<String> queryParameters) {
        org.zerodds.core.Args.require(states, "states");
        org.zerodds.core.Args.require(queryExpression, "queryExpression");
        final Subscriber.DataState fixed = snapshot(states);
        ContentFilter.compile(queryExpression, queryParameters); // an expression that does not parse fails here
        final java.util.concurrent.atomic.AtomicReference<QueryConditionImpl<T>> self =
                new java.util.concurrent.atomic.AtomicReference<>();
        QueryConditionImpl<T> condition = new QueryConditionImpl<>(env, self(),
                fixed.getSampleStates(), fixed.getViewStates(), fixed.getInstanceStates(),
                queryExpression, queryParameters,
                () -> {
                    QueryConditionImpl<T> own = self.get();
                    return anySample(fixed, ContentFilter.compile(own.getQueryExpression(),
                            own.getQueryParameters()));
                });
        self.set(condition);
        return register(condition);
    }

    /** The states of a condition are fixed when it is created. */
    private Subscriber.DataState snapshot(Subscriber.DataState states) {
        if (states == null) {
            return subscriberDataState();
        }
        return DataStateImpl.of(env, states.getSampleStates(), states.getViewStates(),
                states.getInstanceStates());
    }

    /** The default data state of read/take: all three states. */
    private Subscriber.DataState subscriberDataState() {
        return DataStateImpl.any(env);
    }

    /**
     * Spec §2.2.2.5.3.25 get_sample_rejected_status — SAMPLE_REJECTED: samples the reader
     * refused because of RESOURCE_LIMITS (see {@link #rejectedByLimits}).
     */
    @Override
    public synchronized SampleRejectedStatus getSampleRejectedStatus() {
        clearStatus(SAMPLE_REJECTED);
        return readSampleRejectedLocked();
    }

    /**
     * Spec §2.2.2.5.3.24 get_sample_lost_status — SAMPLE_LOST: samples of a writer that
     * never arrived, seen as a gap in the writer's sequence numbers on the wire. The
     * in-process bus loses none.
     */
    @Override
    public synchronized SampleLostStatus getSampleLostStatus() {
        clearStatus(SAMPLE_LOST);
        return readSampleLostLocked();
    }

    /**
     * Spec §2.2.2.5.3.32 wait_for_historical_data. The in-process bus delivers and replays
     * synchronously in {@code enable()}, so the historical data of every writer of this
     * process has arrived when this returns. For the writers reached over RTPS it waits until
     * every matched reliable, non-VOLATILE writer has announced (HEARTBEAT) a last sequence
     * number this reader has received up to, or until {@code maxWait} passes.
     */
    @Override
    public void waitForHistoricalData(Duration maxWait) throws TimeoutException {
        if (maxWait == null) {
            throw new IllegalArgumentException("maxWait must not be null");
        }
        awaitHistoricalData(maxWait.isInfinite()
                ? Long.MAX_VALUE : maxWait.getDuration(TimeUnit.MILLISECONDS));
    }

    @Override
    public void waitForHistoricalData(long maxWait, TimeUnit unit) throws TimeoutException {
        if (maxWait < 0) {
            throw new IllegalArgumentException("maxWait must not be negative");
        }
        if (unit == null) {
            throw new IllegalArgumentException("unit must not be null");
        }
        awaitHistoricalData(unit.toMillis(maxWait));
    }

    private void awaitHistoricalData(long millis) throws TimeoutException {
        ensureEnabled();
        org.zerodds.rtps.RtpsParticipant.WireReader remote = wire;
        if (remote == null || qos.getDurability().getKind() == Durability.Kind.VOLATILE) {
            return; // nothing historical to wait for, or everything arrived in enable()
        }
        long start = System.nanoTime();
        while (!remote.historicalDataReceived()) {
            long elapsed = (System.nanoTime() - start) / 1_000_000L;
            if (millis != Long.MAX_VALUE && elapsed >= millis) {
                throw new TimeoutException("the historical data did not arrive within "
                        + millis + " ms");
            }
            try {
                Thread.sleep(5);
            } catch (InterruptedException interrupted) {
                Thread.currentThread().interrupt();
                throw new TimeoutException("interrupted while waiting for the historical data");
            }
            ensureEnabled();
        }
    }

    /** Spec §2.2.2.5.3.33 get_matched_publications. */
    @Override
    public Set<InstanceHandle> getMatchedPublications() {
        Set<InstanceHandle> out = new LinkedHashSet<>();
        Set<Long> matched;
        synchronized (this) {
            matched = new LinkedHashSet<>(matchedWriters);
        }
        for (InProcessBus.WriterPresence writer : InProcessBus.instance().writers(
                domainId, topic.getName(), typeSupport.getTypeName())) {
            if (matched.contains(writer.writerId()) && writer.writerHandle() != null) {
                out.add(writer.writerHandle());
            }
        }
        org.zerodds.rtps.RtpsParticipant.WireReader remote = wire;
        if (remote != null) {
            for (org.zerodds.rtps.DiscoveryData.Endpoint endpoint : remote.matchedEndpoints()) {
                out.add(Values.of(env, endpoint.endpointGuid));
            }
        }
        return out;
    }

    /**
     * DDS 1.4 §2.2.2.5.3.32 get_matched_publication_data: the built-in topic
     * data of a matched writer, with the QoS it offers. A writer of this
     * process is described by the writer itself, a remote one by what its
     * SEDP announcement carried (DDSI-RTPS 2.5 §9.6.2).
     */
    @Override
    public PublicationBuiltinTopicData getMatchedPublicationData(
            InstanceHandle publicationHandle) {
        if (publicationHandle == null) {
            throw new IllegalArgumentException("publicationHandle must not be null");
        }
        Set<Long> matched;
        synchronized (this) {
            matched = new LinkedHashSet<>(matchedWriters);
        }
        for (InProcessBus.WriterPresence writer : InProcessBus.instance().writers(
                domainId, topic.getName(), typeSupport.getTypeName())) {
            if (matched.contains(writer.writerId())
                    && publicationHandle.equals(writer.writerHandle())
                    && writer instanceof org.zerodds.pub.DataWriterImpl) {
                org.zerodds.pub.DataWriterImpl<?> local = (org.zerodds.pub.DataWriterImpl<?>) writer;
                org.omg.dds.pub.Publisher publisher = local.getParent();
                org.omg.dds.domain.DomainParticipant owner =
                        publisher == null ? null : publisher.getParent();
                return org.zerodds.topic.BuiltinData.publication(env, local, publisher,
                        owner == null ? new org.zerodds.topic.BuiltinTopicKeyImpl(env, new int[4])
                                : org.zerodds.topic.BuiltinTopicKeyImpl.of(env,
                                        owner.getInstanceHandle()));
            }
        }
        org.zerodds.rtps.RtpsParticipant.WireReader remote = wire;
        if (remote != null) {
            for (org.zerodds.rtps.DiscoveryData.Endpoint endpoint : remote.matchedEndpoints()) {
                if (publicationHandle.equals(Values.of(env, endpoint.endpointGuid))) {
                    return org.zerodds.topic.BuiltinData.publication(env, endpoint);
                }
            }
        }
        throw new IllegalArgumentException(publicationHandle
                + " is not a publication matched with this DataReader");
    }

    /** The type support of the reader's topic. */
    public TopicTypeSupport<T> typeSupport() {
        return typeSupport;
    }

    /** The QoS this reader announces on SEDP: its subscription data (DDS 1.4 §2.2.5). */
    private org.zerodds.rtps.QosParameters wireQos() {
        if (parent == null) {
            return null;
        }
        org.omg.dds.domain.DomainParticipant owner = parent.getParent();
        org.omg.dds.topic.BuiltinTopicKey participantKey = owner == null
                ? new org.zerodds.topic.BuiltinTopicKeyImpl(env, new int[4])
                : org.zerodds.topic.BuiltinTopicKeyImpl.of(env, owner.getInstanceHandle());
        return org.zerodds.topic.BuiltinQos.of(
                org.zerodds.topic.BuiltinData.subscription(env, this, parent, participantKey));
    }

    /**
     * Spec §2.2.2.5.3.29 get_key_value — the key of the instance the handle
     * names, written into the key fields of {@code keyHolder}; the other fields
     * of the holder are not changed. Returns {@code keyHolder}, or a new sample
     * when it is {@code null}.
     *
     * @throws IllegalArgumentException if the handle names no instance of this reader
     * @throws PreconditionNotMetException if the instance has not delivered a sample with data
     */
    @Override
    public T getKeyValue(T keyHolder, InstanceHandle handle) {
        @SuppressWarnings("unchecked")
        T source = (T) keySampleOf(handle);
        if (keyHolder == null) {
            return source;
        }
        return typeSupport.keyValue(source, keyHolder);
    }

    public T getKeyValue(InstanceHandle handle) {
        @SuppressWarnings("unchecked")
        T source = (T) keySampleOf(handle);
        return source;
    }

    /** A copy of the first sample with data of the instance {@code handle} names. */
    private Object keySampleOf(InstanceHandle handle) {
        if (!typeSupport.isKeyed()) {
            throw new PreconditionNotMetException(
                    "the topic type " + typeSupport.getTypeName() + " has no key");
        }
        if (handle == null) {
            throw new IllegalArgumentException("the instance handle must not be null");
        }
        Object source = null;
        boolean known = false;
        synchronized (this) {
            for (Map.Entry<InstanceKey, Instance> e : instances.entrySet()) {
                if (InstanceKey.toHandle(e.getKey()).equals(handle)) {
                    known = true;
                    source = e.getValue().keySample;
                    break;
                }
            }
        }
        if (!known) {
            throw new IllegalArgumentException(handle + " is not an instance of this DataReader");
        }
        if (source == null) {
            throw new PreconditionNotMetException(
                    "no sample with data arrived for the instance " + handle);
        }
        return deepCopy(source);
    }

    /** The handle goes into {@code handle}, or into a new object when that is null. */
    @Override
    public ModifiableInstanceHandle lookupInstance(ModifiableInstanceHandle handle,
            T keyHolder) {
        ModifiableInstanceHandle target =
                handle == null ? InstanceHandle.newInstanceHandle(env) : handle;
        target.copyFrom(lookupInstance(keyHolder));
        return target;
    }

    @SuppressWarnings("unchecked")
    @Override
    public StatusCondition<DataReader<T>> getStatusCondition() {
        return (StatusCondition<DataReader<T>>) super.getStatusCondition();
    }
}
