// SPDX-License-Identifier: Apache-2.0
package org.zerodds.pub;

import java.util.ArrayList;
import java.util.Collection;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.concurrent.CopyOnWriteArrayList;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.TimeoutException;

import org.omg.dds.core.DomainEntity;
import org.omg.dds.core.Duration;
import org.omg.dds.core.policy.QosPolicy;
import org.omg.dds.core.QosProvider;
import org.omg.dds.core.StatusCondition;
import org.omg.dds.core.status.Status;
import org.omg.dds.domain.DomainParticipant;
import org.omg.dds.pub.DataWriter;
import org.omg.dds.pub.DataWriterListener;
import org.omg.dds.pub.DataWriterQos;
import org.omg.dds.pub.Publisher;
import org.omg.dds.pub.PublisherListener;
import org.omg.dds.pub.PublisherQos;
import org.omg.dds.topic.Topic;
import org.omg.dds.topic.TopicQos;
import org.zerodds.cdr.TopicTypeSupport;
import org.zerodds.cdr.TypeSupportImpl;
import org.zerodds.core.AlreadyClosedException;
import org.zerodds.core.EntityBase;
import org.zerodds.core.PreconditionNotMetException;

/**
 * ZeroDDS {@link Publisher} (DDS-Java-PSM 1.0 §7.6.1).
 *
 * <p>A publisher is the factory of the data writers of its partition (DDS 1.2
 * §7.1.3.9) and the entity that scopes {@code begin_coherent_changes} and
 * {@code suspend_publications}.
 */
public final class PublisherImpl extends EntityBase<PublisherListener, PublisherQos>
implements Publisher, DomainEntity<PublisherListener, PublisherQos> {

    private final int domainId;
    private final DomainParticipant parent;
    private volatile PublisherQos qos;
    private DataWriterQos defaultDataWriterQos;
    private final List<DataWriter<?>> writers = new CopyOnWriteArrayList<>();
    /** The writers announced on the built-in "DCPSPublication" topic. */
    private final Set<DataWriter<?>> announced =
            java.util.concurrent.ConcurrentHashMap.newKeySet();

    private volatile boolean closed = false;
    private volatile boolean enabled = false;
    private int suspendDepth = 0;
    private int coherentDepth = 0;
    /** Counts the coherent sets this publisher began; names the set that is in progress. */
    private long coherentEpoch = 0;
    /** The deliveries held back by the coherent set that is in progress, in write order. */
    private final List<Runnable> coherentSet = new ArrayList<>();

    public PublisherImpl(org.omg.dds.core.ServiceEnvironment environment, int domainId,
            DomainParticipant parent, PublisherQos qos) {
        super(environment);
        this.domainId = domainId;
        this.parent = parent;
        this.qos = qos;
        this.defaultDataWriterQos =
                org.zerodds.core.qos.DataWriterQosImpl.defaults(environment);
    }

    // --- Create DataWriter (§7.6.2) ----------------------------------------

    @Override
    public <TYPE> DataWriter<TYPE> createDataWriter(Topic<TYPE> topic) {
        org.zerodds.core.Args.require(topic, "topic");
        return createDataWriter(topic, defaultDataWriterQos);
    }

    @Override
    public <TYPE> DataWriter<TYPE> createDataWriter(Topic<TYPE> topic, DataWriterQos qos) {
        org.zerodds.core.Args.require(topic, "topic");
        org.zerodds.core.Args.require(qos, "qos");
        return createDataWriter(topic, codecOf(topic), qos);
    }

    @Override
    public <TYPE> DataWriter<TYPE> createDataWriter(Topic<TYPE> topic, DataWriterQos qos,
            DataWriterListener<TYPE> listener,
            Collection<Class<? extends Status>> statuses) {
        org.zerodds.core.Args.require(topic, "topic");
        org.zerodds.core.Args.require(qos, "qos");
        // the listener is set before the writer is enabled, so the matches made by
        // enable() reach it (DDS 1.4 §2.2.4.3.1)
        return createDataWriter(topic, codecOf(topic), qos, listener, statuses);
    }

    /**
     * The ZeroDDS form: the writer serializes with {@code typeSupport}
     * instead of the topic's own codec. Used by an application that holds a
     * codec it built itself.
     */
    public <TYPE> DataWriter<TYPE> createDataWriter(Topic<TYPE> topic,
            TopicTypeSupport<TYPE> typeSupport) {
        org.zerodds.core.Args.require(topic, "topic");
        return createDataWriter(topic, typeSupport, defaultDataWriterQos);
    }

    public <TYPE> DataWriter<TYPE> createDataWriter(Topic<TYPE> topic,
            TopicTypeSupport<TYPE> typeSupport, DataWriterQos writerQos) {
        org.zerodds.core.Args.require(topic, "topic");
        return createDataWriter(topic, typeSupport, writerQos, null, null);
    }

    private <TYPE> DataWriter<TYPE> createDataWriter(Topic<TYPE> topic,
            TopicTypeSupport<TYPE> typeSupport, DataWriterQos writerQos,
            DataWriterListener<TYPE> listener, Collection<Class<? extends Status>> statuses) {
        ensureOpen();
        ensureTopicOpen(topic);
        org.zerodds.core.qos.QosRules.checkConsistent(writerQos);
        DataWriterImpl<TYPE> dw = new DataWriterImpl<>(domainId, this, topic, typeSupport,
                writerQos, partitions());
        if (listener != null) {
            dw.setListener(listener, statuses);
        }
        // DDS 1.4 §2.2.3.20: what a factory creates is enabled when the factory is
        // enabled and its ENTITY_FACTORY says so (§2.2.2.1.1.7).
        if (createsEnabled()) {
            dw.enable();
        }
        writers.add(dw);
        // The DataWriter leaves this Publisher, and the built-in "DCPSPublication" topic of
        // the participant, when it is closed (DDS 1.2 §2.2.1.3.1).
        dw.addCloseHook(() -> {
            writers.remove(dw);
            if (announced.remove(dw)) {
                org.zerodds.domain.Discovery.writerClosed(parent, dw);
            }
        });
        return dw;
    }

    /**
     * A writer of this Publisher was enabled: it appears on the built-in "DCPSPublication"
     * topic of the participant, once (DDS 1.2 §2.2.1.3.1).
     */
    void writerEnabled(DataWriterImpl<?> writer) {
        if (announced.add(writer)) {
            org.zerodds.domain.Discovery.writerCreated(parent, writer, this);
        }
        if (writer.getQos().getLiveliness().getKind()
                == org.omg.dds.core.policy.Liveliness.Kind.MANUAL_BY_PARTICIPANT
                && parent instanceof org.zerodds.domain.DomainParticipantImpl) {
            org.zerodds.domain.DomainParticipantImpl participant =
                    (org.zerodds.domain.DomainParticipantImpl) parent;
            participant.manualWriterAdded();
            writer.addCloseHook(participant::manualWriterRemoved);
        }
    }

    /** A write or an assert_liveliness of a writer asserted the liveliness of the participant. */
    void writerAsserted() {
        if (parent instanceof org.zerodds.domain.DomainParticipantImpl) {
            ((org.zerodds.domain.DomainParticipantImpl) parent).livelinessAsserted();
        }
    }

    /** The codec of the topic (§7.5.1: the topic defines the type). */
    @SuppressWarnings("unchecked")
    static <TYPE> TopicTypeSupport<TYPE> codecOf(Topic<TYPE> topic) {
        if (topic.getTypeSupport() instanceof TypeSupportImpl) {
            return ((TypeSupportImpl<TYPE>) topic.getTypeSupport()).codec();
        }
        throw new UnsupportedOperationException(
                "topic carries a TypeSupport of another implementation");
    }

    private void ensureOpen() {
        if (closed) {
            throw new AlreadyClosedException("Publisher is closed");
        }
    }

    /** DDS 1.2 §2.2.2.3.2: a writer can only be created for an open topic. */
    static void ensureTopicOpen(Topic<?> topic) {
        if (org.zerodds.topic.TopicImpl.isClosed(topic)) {
            throw new AlreadyClosedException("Topic is closed");
        }
    }

    /** The partition names of this publisher (DDS 1.2 §7.1.3.9). */
    private Set<String> partitions() {
        return new LinkedHashSet<>(qos.getPartition().getName());
    }

    // --- Lookup (§7.6.3) ---------------------------------------------------

    @SuppressWarnings("unchecked")
    @Override
    public <TYPE> DataWriter<TYPE> lookupDataWriter(String topicName) {
        org.zerodds.core.Args.require(topicName, "topicName");
        for (DataWriter<?> writer : writers) {
            if (writer.getTopic().getName().equals(topicName)) {
                return (DataWriter<TYPE>) writer;
            }
        }
        return null;
    }

    @SuppressWarnings("unchecked")
    @Override
    public <TYPE> DataWriter<TYPE> lookupDataWriter(Topic<TYPE> topic) {
        org.zerodds.core.Args.require(topic, "topic");
        return lookupDataWriter(topic.getName());
    }

    // --- Coherent changes and suspension (§7.6.4) --------------------------

    /**
     * DDS 1.4 §2.2.2.4.1.14 delete_contained_entities: every DataWriter is deleted; each
     * leaves this Publisher and the built-in topic through its close hook.
     */
    @Override
    public void closeContainedEntities() {
        for (DataWriter<?> writer : new ArrayList<>(writers)) {
            writer.close();
        }
    }

    /**
     * DDS 1.4 §2.2.2.4.1.8 suspend_publications: a hint, so nothing is held back. The calls
     * nest like the ones of begin_coherent_changes: each must be matched by a
     * resume_publications.
     */
    @Override
    public void suspendPublications() {
        ensureOpen();
        synchronized (this) {
            suspendDepth++;
        }
    }

    /**
     * DDS 1.4 §2.2.2.4.1.9: a resume has to match a previous suspend, otherwise the
     * operation fails with PRECONDITION_NOT_MET.
     */
    @Override
    public void resumePublications() {
        ensureOpen();
        synchronized (this) {
            if (suspendDepth == 0) {
                throw new PreconditionNotMetException(
                        "resume_publications without a matching suspend_publications");
            }
            suspendDepth--;
        }
    }

    /** Whether {@code suspend_publications} is in force (DDS 1.2 §2.2.2.3.5). */
    public boolean isSuspended() {
        synchronized (this) {
            return suspendDepth > 0;
        }
    }

    /**
     * DDS 1.4 §2.2.2.4.1.10 begin_coherent_changes: the calls nest, and the coherent set ends
     * with the last end_coherent_changes. While it lasts, the samples of the writers are held
     * back when the PRESENTATION of this Publisher has coherent_access, and delivered together,
     * in the order they were written, when the set ends.
     */
    @Override
    public void beginCoherentChanges() {
        ensureOpen();
        synchronized (this) {
            if (coherentDepth++ == 0) {
                coherentEpoch++;
            }
        }
    }

    /**
     * The name of the coherent set in progress, or 0 if there is none or PRESENTATION does not
     * ask for coherent access; the writers put it on the wire as the coherent set of their samples.
     */
    /** Whether the PRESENTATION of this publisher has access scope GROUP (DDSI-RTPS 2.5 8.7.5). */
    boolean groupScope() {
        return qos.getPresentation().getAccessScope()
                == org.omg.dds.core.policy.Presentation.AccessScopeKind.GROUP;
    }

    /** The group sequence number the last sample of this publisher got. */
    private long groupSeq;

    /**
     * Held while a group sequence number is assigned and the change is entered in the history of its
     * writer, and while the HEARTBEAT reads the number: a HEARTBEAT announces only numbers whose
     * changes are entered (DDSI-RTPS 2.5 8.3.7.5), so the reader never gives up a number early.
     */
    private final Object groupLock = new Object();

    /** Assigns the group stamp of a sample and runs {@code write} (which enters it) under one lock. */
    <R> R stampAndWrite(java.util.function.Function<org.zerodds.rtps.RtpsParticipant.GroupStamp, R> write) {
        synchronized (groupLock) {
            return write.apply(groupStamp());
        }
    }
    /** The coherent set (by epoch) the group set below belongs to, and the group number of its first sample. */
    private long groupSetEpoch;
    private long groupSetFirst;

    /**
     * The group stamp of the next sample of a writer of this publisher: the group sequence number
     * (strictly increasing over all writers), the group sequence number of the first sample of the
     * group coherent set in progress, and the digest of the writers (DDSI-RTPS 2.5 8.7.5, 8.7.6).
     */
    org.zerodds.rtps.RtpsParticipant.GroupStamp groupStamp() {
        synchronized (this) {
            long gsn = ++groupSeq;
            long set = 0;
            if (coherentDepth > 0 && qos.getPresentation().isCoherentAccess()) {
                if (groupSetEpoch != coherentEpoch) {
                    groupSetEpoch = coherentEpoch;
                    groupSetFirst = gsn;
                }
                set = groupSetFirst;
            }
            return new org.zerodds.rtps.RtpsParticipant.GroupStamp(gsn, set, writerDigest());
        }
    }

    /**
     * The group stamp as it stands, for the HEARTBEAT with the GroupInfoFlag: the last group sequence
     * number given out and the digest of the writers (DDSI-RTPS 2.5 8.3.7.5).
     */
    org.zerodds.rtps.RtpsParticipant.GroupStamp currentGroupStamp() {
        synchronized (groupLock) {
            synchronized (this) {
                return groupSeq == 0 ? null
                        : new org.zerodds.rtps.RtpsParticipant.GroupStamp(groupSeq, 0L, writerDigest());
            }
        }
    }

    /** The GroupDigest_t of the writers of this publisher that are on the wire (DDSI-RTPS 2.5 9.3.2.5). */
    private byte[] writerDigest() {
        java.util.List<byte[]> ids = new ArrayList<>();
        for (DataWriter<?> writer : writers) {
            if (writer instanceof DataWriterImpl) {
                byte[] id = ((DataWriterImpl<?>) writer).wireEntityId();
                if (id != null) {
                    ids.add(id);
                }
            }
        }
        return org.zerodds.rtps.GroupDigest.of(ids);
    }

    long coherentSetName() {
        synchronized (this) {
            return coherentDepth > 0 && qos.getPresentation().isCoherentAccess() ? coherentEpoch : 0;
        }
    }

    /**
     * DDS 1.4 §2.2.2.4.1.11 end_coherent_changes: PRECONDITION_NOT_MET without a matching
     * begin_coherent_changes.
     */
    @Override
    public void endCoherentChanges() {
        ensureOpen();
        List<Runnable> deliver = null;
        long ended = 0;
        synchronized (this) {
            if (coherentDepth == 0) {
                throw new PreconditionNotMetException(
                        "end_coherent_changes without begin_coherent_changes");
            }
            if (coherentDepth == 1 && qos.getPresentation().isCoherentAccess()) {
                ended = coherentEpoch;
            }
            coherentDepth--;
            if (coherentDepth == 0 && !coherentSet.isEmpty()) {
                deliver = new ArrayList<>(coherentSet);
                coherentSet.clear();
            }
        }
        if (deliver != null) {
            deliverTogether(deliver);
        }
        if (ended != 0) {
            synchronized (groupLock) { // the number of the End Coherent Set DATA is entered by every writer
                org.zerodds.rtps.RtpsParticipant.GroupStamp ecs = null;
                if (groupScope()) {
                    synchronized (this) {
                        if (groupSetEpoch == ended && groupSetFirst != 0) {
                            // the End Coherent Set DATA of every writer: one past the last sample (8.7.6)
                            ecs = new org.zerodds.rtps.RtpsParticipant.GroupStamp(++groupSeq, groupSetFirst,
                                    writerDigest());
                            groupSetFirst = 0;
                        }
                    }
                }
                for (DataWriter<?> writer : writers) {
                    if (writer instanceof DataWriterImpl) {
                        ((DataWriterImpl<?>) writer).endCoherentSet(ended, ecs);
                    }
                }
            }
        }
    }

    /**
     * The samples of one coherent set reach the readers as one step: a reader on another thread
     * sees all of them or none (DDS 1.4 2.2.2.4.1.10).
     */
    private static void deliverTogether(final List<Runnable> deliveries) {
        org.zerodds.internal.InProcessBus.instance().atomically(new Runnable() {
            @Override
            public void run() {
                for (Runnable delivery : deliveries) {
                    delivery.run();
                }
            }
        });
    }

    /**
     * Whether the publisher is inside a coherent changes scope (begin_coherent_changes without
     * the matching end_coherent_changes yet).
     */
    public boolean inCoherentChanges() {
        synchronized (this) {
            return coherentDepth > 0;
        }
    }

    /**
     * A writer of this Publisher has a message to deliver: when a coherent set is in progress
     * and PRESENTATION asks for coherent access, it is kept for the end of the set.
     *
     * @return whether the delivery was taken over.
     */
    boolean deferCoherent(Runnable delivery) {
        synchronized (this) {
            if (coherentDepth > 0 && qos.getPresentation().isCoherentAccess()) {
                coherentSet.add(delivery);
                return true;
            }
            return false;
        }
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
     * DDS 1.4 §2.2.2.4.1.12: until every DataWriter of this Publisher has its samples
     * acknowledged by the matched reliable readers, or {@code nanos} in all have passed
     * (TIMEOUT). A reader of this process has a sample when write returns; the readers of
     * other processes acknowledge over RTPS (see {@link DataWriterImpl#awaitAcknowledgments}).
     */
    private void awaitAcknowledgments(long nanos) throws TimeoutException {
        ensureOpen();
        long start = System.nanoTime();
        for (DataWriter<?> writer : writers) {
            DataWriterImpl<?> impl = (DataWriterImpl<?>) writer;
            if (!impl.isEnabled() || impl.isClosed()) {
                continue;
            }
            long remaining = nanos == Long.MAX_VALUE ? Long.MAX_VALUE
                    : Math.max(0L, nanos - (System.nanoTime() - start));
            impl.awaitAcknowledgments(remaining);
        }
    }

    // --- QoS (§7.3.5) ------------------------------------------------------

    @Override
    public DataWriterQos getDefaultDataWriterQos() {
        return defaultDataWriterQos;
    }

    /**
     * DDS 1.4 §2.2.2.4.1.15: the default QoS of the writers created afterwards; a set of
     * policies that is not consistent (INCONSISTENT_POLICY) leaves the default unchanged.
     */
    @Override
    public synchronized void setDefaultDataWriterQos(DataWriterQos qos) {
        org.zerodds.core.Args.require(qos, "qos");
        org.zerodds.core.qos.QosRules.checkConsistent(qos);
        this.defaultDataWriterQos = qos;
    }

    /**
     * DDS 1.4 §2.2.2.4.1.17: the policies of the topic QoS replace the corresponding policies
     * of the writer QoS, wherever the topic QoS has one a writer knows (the topic wins over
     * what the writer QoS held, default or not). The result is not checked for consistency.
     */
    @Override
    public DataWriterQos copyFromTopicQos(DataWriterQos dwQos, TopicQos tQos) {
        org.zerodds.core.Args.require(dwQos, "dwQos");
        org.zerodds.core.Args.require(tQos, "tQos");
        DataWriterQos merged = dwQos;
        for (Map.Entry<Class<? extends QosPolicy>, ? extends QosPolicy> entry
                : org.zerodds.core.qos.QosCompatibility.wide(tQos).entrySet()) {
            if (entry.getValue() instanceof QosPolicy.ForDataWriter) {
                merged = merged.withPolicy((QosPolicy.ForDataWriter) entry.getValue());
            }
        }
        return merged;
    }

    @Override
    public PublisherQos getQos() {
        return qos;
    }

    /**
     * DDS 1.4 §2.2.2.1.1.1: PRESENTATION cannot change once the Publisher is enabled
     * (IMMUTABLE_POLICY); PARTITION, GROUP_DATA and ENTITY_FACTORY can. A new PARTITION is that
     * of the writers of this Publisher from then on, and they are matched again.
     */
    @Override
    public void setQos(PublisherQos qos) {
        Set<String> before;
        Set<String> after;
        synchronized (this) {
            org.zerodds.core.qos.QosRules.checkChange(enabled, this.qos, qos);
            before = partitions();
            this.qos = qos;
            after = partitions();
        }
        // PARTITION can change (DDS 1.4 §2.2.3.13): the writers match again, outside the monitor
        // because the matches reach listeners.
        if (!before.equals(after)) {
            for (DataWriter<?> writer : new ArrayList<>(writers)) {
                ((DataWriterImpl<?>) writer).changePartitions(after);
            }
        }
        // The built-in topic data of a writer carries the PARTITION and GROUP_DATA of its
        // Publisher (DDS 1.4 §2.2.5): the writers are announced again with the new QoS.
        for (DataWriter<?> writer : new ArrayList<>(writers)) {
            DataWriterImpl<?> impl = (DataWriterImpl<?>) writer;
            if (!impl.isClosed()) {
                impl.announceQos();
            }
        }
    }

    @Override
    protected PublisherQos profileQos(QosProvider provider) {
        return provider.getPublisherQos();
    }

    // --- Entity (§7.3.6) ----------------------------------------------------

    @Override
    public DomainParticipant getParent() {
        return parent;
    }

    @SuppressWarnings("unchecked")
    @Override
    public StatusCondition<Publisher> getStatusCondition() {
        return (StatusCondition<Publisher>) super.getStatusCondition();
    }

    /** The writers this publisher has created (DDS 1.2 §2.2.2.3.1). */
    public List<DataWriter<?>> getDataWriters() {
        return new ArrayList<>(writers);
    }

    /**
     * Whether a DataWriter created now is enabled: this Publisher is enabled and the
     * ENTITY_FACTORY of its QoS has autoenable_created_entities true (DDS 1.4 §2.2.3.20).
     */
    private boolean createsEnabled() {
        return enabled && autoEnablesCreated();
    }

    private boolean autoEnablesCreated() {
        PublisherQos current = qos;
        return current.getEntityFactory() == null
                || current.getEntityFactory().isAutoEnableCreatedEntities();
    }

    /**
     * Enables the Publisher. A DataWriter created while the Publisher was disabled is
     * enabled with it when the ENTITY_FACTORY says so (DDS 1.4 §2.2.2.1.1.7).
     */
    @Override
    public void enable() {
        if (closed) {
            throw new AlreadyClosedException("Publisher is closed");
        }
        if (!enabled && parent instanceof org.zerodds.domain.DomainParticipantImpl
                && !((org.zerodds.domain.DomainParticipantImpl) parent).isEnabled()) {
            throw new PreconditionNotMetException(
                    "the DomainParticipant of the Publisher is not enabled");
        }
        enableAll();
    }

    /** The built-in Publisher exists before its participant is enabled and is enabled with it. */
    public void enableBuiltin() {
        enableAll();
    }

    private void enableAll() {
        boolean first = !enabled;
        enabled = true;
        if (autoEnablesCreated()) {
            for (DataWriter<?> writer : new ArrayList<>(writers)) {
                writer.enable();
            }
        }
    }

    public boolean isEnabled() {
        return enabled;
    }

    @Override
    public void close() {
        // §7.2.3: closing a Publisher closes all contained DataWriters. The flag
        // goes up first, so a concurrent create call cannot slip a new
        // contained entity in behind the close (DDS-Java-PSM 1.0 §7.2.4).
        closed = true;
        // a coherent set still open ends with the Publisher: what was written is delivered
        List<Runnable> deliver;
        synchronized (this) {
            coherentDepth = 0;
            deliver = new ArrayList<>(coherentSet);
            coherentSet.clear();
        }
        deliverTogether(deliver);
        for (DataWriter<?> writer : new ArrayList<>(writers)) {
            writer.close(); // its close hook removes it and retracts it
        }
        runCloseHooks();
    }

    public boolean isClosed() {
        return closed;
    }

    @Override
    public String toString() {
        return "Publisher[domain=" + domainId + "]";
    }
}
