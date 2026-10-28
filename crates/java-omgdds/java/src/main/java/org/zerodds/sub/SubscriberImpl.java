// SPDX-License-Identifier: Apache-2.0
package org.zerodds.sub;

import java.util.ArrayList;
import java.util.Collection;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.concurrent.CopyOnWriteArrayList;
import java.util.concurrent.locks.ReentrantLock;

import org.omg.dds.core.DomainEntity;
import org.omg.dds.core.QosProvider;
import org.omg.dds.core.ServiceEnvironment;
import org.omg.dds.core.StatusCondition;
import org.omg.dds.core.policy.QosPolicy;
import org.omg.dds.core.status.Status;
import org.omg.dds.domain.DomainParticipant;
import org.omg.dds.sub.DataReader;
import org.omg.dds.sub.DataReaderListener;
import org.omg.dds.sub.DataReaderQos;
import org.omg.dds.sub.Subscriber;
import org.omg.dds.sub.SubscriberListener;
import org.omg.dds.sub.SubscriberQos;
import org.omg.dds.topic.ContentFilteredTopic;
import org.omg.dds.topic.Topic;
import org.omg.dds.topic.TopicDescription;
import org.omg.dds.topic.TopicQos;
import org.zerodds.cdr.TopicTypeSupport;
import org.zerodds.cdr.TypeSupportImpl;
import org.zerodds.core.AlreadyClosedException;
import org.zerodds.core.EntityBase;
import org.zerodds.core.PreconditionNotMetException;
import org.zerodds.topic.ContentFilteredTopicImpl;

/**
 * ZeroDDS {@link Subscriber} (DDS-Java-PSM 1.0 §7.7.1).
 *
 * <p>A subscriber is the factory of the data readers of its partition
 * (DDS 1.2 §7.1.3.9) and the entity that scopes {@code begin_access} /
 * {@code end_access} (DDS 1.2 §2.2.2.2.3: between them the subscriber's
 * readers behave as if they were used by a single thread).
 */
public final class SubscriberImpl extends EntityBase<SubscriberListener, SubscriberQos>
implements Subscriber, DomainEntity<SubscriberListener, SubscriberQos> {

    private final int domainId;
    private final DomainParticipant parent;
    private volatile SubscriberQos qos;
    private DataReaderQos defaultDataReaderQos;
    private final List<DataReader<?>> readers = new CopyOnWriteArrayList<>();
    /** The readers on a MultiTopic that joins: not announced on the built-in topics. */
    private final java.util.Set<DataReader<?>> joined =
            java.util.concurrent.ConcurrentHashMap.newKeySet();
    private final ReentrantLock access = new ReentrantLock();

    private volatile boolean closed = false;
    private volatile boolean enabled = false;

    public SubscriberImpl(ServiceEnvironment environment, int domainId,
            DomainParticipant parent, SubscriberQos qos) {
        super(environment);
        this.domainId = domainId;
        this.parent = parent;
        this.qos = qos;
        this.defaultDataReaderQos =
                org.zerodds.core.qos.DataReaderQosImpl.defaults(environment);
    }

    // --- Create DataReader (§7.7.2) ----------------------------------------

    @Override
    public <TYPE> DataReader<TYPE> createDataReader(TopicDescription<TYPE> topic) {
        org.zerodds.core.Args.require(topic, "topic");
        return createDataReader(topic, defaultDataReaderQos);
    }

    @Override
    public <TYPE> DataReader<TYPE> createDataReader(TopicDescription<TYPE> topic,
            DataReaderQos qos) {
        org.zerodds.core.Args.require(topic, "topic");
        org.zerodds.core.Args.require(qos, "qos");
        return createDataReader(topic, codecOf(topic), qos);
    }

    @Override
    public <TYPE> DataReader<TYPE> createDataReader(TopicDescription<TYPE> topic,
            DataReaderQos qos,
            DataReaderListener<TYPE> listener,
            Collection<Class<? extends Status>> statuses) {
        org.zerodds.core.Args.require(topic, "topic");
        org.zerodds.core.Args.require(qos, "qos");
        // the listener is set before the reader is enabled, so the matches made by
        // enable() reach it (DDS 1.4 §2.2.4.3.1)
        return createDataReader(topic, codecOf(topic), qos, listener, statuses);
    }

    /**
     * The ZeroDDS form: the reader decodes with {@code typeSupport} instead of
     * the topic description's own codec.
     */
    public <TYPE> DataReader<TYPE> createDataReader(TopicDescription<TYPE> topic,
            TopicTypeSupport<TYPE> typeSupport) {
        org.zerodds.core.Args.require(topic, "topic");
        return createDataReader(topic, typeSupport, defaultDataReaderQos);
    }

    public <TYPE> DataReader<TYPE> createDataReader(TopicDescription<TYPE> topic,
            TopicTypeSupport<TYPE> typeSupport, DataReaderQos readerQos) {
        org.zerodds.core.Args.require(topic, "topic");
        return createDataReader(topic, typeSupport, readerQos, null, null);
    }

    @SuppressWarnings("unchecked")
    private <TYPE> DataReader<TYPE> createDataReader(TopicDescription<TYPE> topic,
            TopicTypeSupport<TYPE> typeSupport, DataReaderQos readerQos,
            DataReaderListener<TYPE> listener, Collection<Class<? extends Status>> statuses) {
        ensureOpen();
        ensureDescriptionOpen(topic);
        java.util.function.Predicate<TYPE> filter = null;
        Topic<TYPE> plain;
        if (topic instanceof ContentFilteredTopic) {
            ContentFilteredTopic<TYPE> cft = (ContentFilteredTopic<TYPE>) topic;
            if (!(cft instanceof ContentFilteredTopicImpl)) {
                // RETCODE_UNSUPPORTED (Table 7.1): an object of another implementation
                throw new UnsupportedOperationException(
                        "content-filtered topic of another implementation: " + cft);
            }
            plain = (Topic<TYPE>) cft.getRelatedTopic();
            filter = ((ContentFilteredTopicImpl<TYPE>) cft)::evaluate;
        } else if (topic instanceof org.zerodds.topic.MultiTopicImpl) {
            org.zerodds.topic.MultiTopicImpl<TYPE> multiTopic =
                    (org.zerodds.topic.MultiTopicImpl<TYPE>) topic;
            if (!multiTopic.isDirect()) {
                DataReader<TYPE> joined = createJoinedReader(multiTopic, typeSupport, readerQos);
                if (listener != null) {
                    joined.setListener(listener, statuses);
                }
                return joined;
            }
            // DDS-DCPS 1.4 §2.2.2.3.4, SELECT * FROM <topic> onto the type of
            // that topic: the WHERE clause is a content filter of the source.
            plain = (Topic<TYPE>) multiTopic.source();
            filter = multiTopic::evaluate;
        } else if (topic instanceof Topic) {
            plain = (Topic<TYPE>) topic;
        } else {
            throw new UnsupportedOperationException(
                    "topic description of another implementation: " + topic);
        }
        ensureTopicOpen(plain);
        org.zerodds.core.qos.QosRules.checkConsistent(readerQos);
        DataReaderImpl<TYPE> dr = new DataReaderImpl<>(domainId, this, plain, typeSupport,
                readerQos, partitions(), filter);
        owned(dr, plain);
        if (topic != plain) {
            // §2.2.2.5.3.? get_topicdescription: the description the reader
            // was created with (the filter, the MultiTopic), not the topic
            // under it.
            dr.describedBy(topic);
            usedBy(dr, topic);
        }
        if (listener != null) {
            dr.setListener(listener, statuses);
        }
        // DDS 1.4 §2.2.3.20: what a factory creates is enabled when the factory is
        // enabled and its ENTITY_FACTORY says so (§2.2.2.1.1.7).
        if (createsEnabled()) {
            dr.enable();
        }
        readers.add(dr);
        // DDS 1.2 §2.2.1.3.1: the DataReader is announced on the built-in
        // "DCPSSubscription" topic of the participant when it is enabled (readerEnabled),
        // and leaves it, and this Subscriber, when it is closed.
        dr.addCloseHook(() -> {
            readers.remove(dr);
            readerGone(dr);
        });
        enableCreated(dr);
        return dr;
    }

    /**
     * DDS 1.4 2.2.2.1.1.7: a reader is created enabled when this Subscriber is enabled and its
     * ENTITY_FACTORY says autoenable_created_entities; otherwise it waits for enable.
     */
    private void enableCreated(DataReader<?> reader) {
        if (enabled && (qos.getEntityFactory() == null
                || qos.getEntityFactory().isAutoEnableCreatedEntities())) {
            reader.enable();
        }
    }

    /** A ContentFilteredTopic or MultiTopic cannot be deleted while a reader reads it (DDS 1.4 2.2.2.3.3). */
    private static void ensureDescriptionOpen(TopicDescription<?> topic) {
        if ((topic instanceof ContentFilteredTopicImpl && ((ContentFilteredTopicImpl<?>) topic).isClosed())
                || (topic instanceof org.zerodds.topic.MultiTopicImpl
                        && ((org.zerodds.topic.MultiTopicImpl<?>) topic).isClosed())) {
            throw new AlreadyClosedException("the TopicDescription " + topic.getName() + " is closed");
        }
    }

    private static void usedBy(DataReaderImpl<?> reader, TopicDescription<?> topic) {
        if (topic instanceof ContentFilteredTopicImpl) {
            ContentFilteredTopicImpl<?> filtered = (ContentFilteredTopicImpl<?>) topic;
            filtered.use();
            reader.addCloseHook(filtered::release);
        } else if (topic instanceof org.zerodds.topic.MultiTopicImpl) {
            org.zerodds.topic.MultiTopicImpl<?> multi = (org.zerodds.topic.MultiTopicImpl<?>) topic;
            multi.use();
            reader.addCloseHook(multi::release);
        }
    }

    /** The reader belongs to the participant of this Subscriber (for ignore_*). */
    private void owned(DataReaderImpl<?> reader, Topic<?> plain) {
        if (parent != null) {
            org.zerodds.internal.Ignores.register(reader.getInstanceHandle(),
                    parent.getInstanceHandle(), plain.getName());
            reader.addCloseHook(() -> org.zerodds.internal.Ignores.forget(reader.getInstanceHandle()));
        }
    }

    private final Set<DataReader<?>> announced = java.util.concurrent.ConcurrentHashMap.newKeySet();
    /** The readers made by {@link #createInternalReader}. */
    private final Set<DataReader<?>> internal = java.util.concurrent.ConcurrentHashMap.newKeySet();

    /**
     * A reader of this Subscriber was enabled: it appears on the built-in "DCPSSubscription"
     * topic once (the readers a MultiTopic join holds are not announced).
     */
    public void readerEnabled(DataReader<?> reader) {
        if (joined.contains(reader)) {
            return;
        }
        if (announced.add(reader)) {
            org.zerodds.domain.Discovery.readerCreated(parent, reader, qos);
        }
    }

    private void readerGone(DataReader<?> reader) {
        if (announced.remove(reader)) {
            org.zerodds.domain.Discovery.readerClosed(parent, reader);
        }
    }

    /**
     * A reader on a MultiTopic that joins several topics or projects onto
     * another type (DDS-DCPS 1.4 §2.2.2.3.4): the reader reads the topic of
     * its own {@link org.zerodds.topic.MultiTopicEngine}, which forms the
     * result samples from the readers it holds on the topics of the
     * {@code FROM} clause. The engine ends with the reader.
     */
    private <TYPE> DataReader<TYPE> createJoinedReader(
            org.zerodds.topic.MultiTopicImpl<TYPE> multiTopic,
            TopicTypeSupport<TYPE> typeSupport, DataReaderQos readerQos) {
        org.zerodds.topic.MultiTopicEngine<TYPE> engine =
                multiTopic.newEngine(this, domainId, typeSupport);
        DataReaderImpl<TYPE> dr = new DataReaderImpl<>(domainId, this,
                engine.resultTopic(), typeSupport, readerQos, partitions(), null);
        owned(dr, engine.resultTopic());
        dr.describedBy(multiTopic);
        usedBy(dr, multiTopic);
        dr.onClose(engine::close);
        // The reader of a MultiTopic is not announced on the built-in topics:
        // it reads no topic of the domain. The readers its join holds on the
        // source topics are (createInternalReader).
        joined.add(dr);
        readers.add(dr);
        dr.addCloseHook(() -> {
            readers.remove(dr);
            joined.remove(dr);
        });
        enableCreated(dr);
        engine.start(readerQos, partitions());
        return dr;
    }

    /**
     * A reader of this subscriber that is not one of its contained entities:
     * the reader a MultiTopic join holds on one of its source topics. It is
     * neither listed by {@code getDataReaders} nor announced on the built-in
     * topics; the join closes it.
     */
    public <TYPE> DataReaderImpl<TYPE> createInternalReader(Topic<TYPE> topic,
            DataReaderQos readerQos) {
        ensureOpen();
        ensureTopicOpen(topic);
        DataReaderImpl<TYPE> dr = new DataReaderImpl<>(domainId, this, topic,
                codecOf(topic), readerQos, partitions(), null);
        dr.markInternal();
        dr.enable();
        org.zerodds.domain.Discovery.readerCreated(parent, dr, qos);
        return dr;
    }

    /** Closes a reader made by {@link #createInternalReader}. */
    public void closeInternalReader(DataReader<?> reader) {
        readerGone(reader);
        reader.close();
        internal.remove(reader);
    }

    /** The codec of the topic description (§7.5.1: it defines the type). */
    @SuppressWarnings("unchecked")
    static <TYPE> TopicTypeSupport<TYPE> codecOf(TopicDescription<TYPE> topic) {
        if (topic.getTypeSupport() instanceof TypeSupportImpl) {
            return ((TypeSupportImpl<TYPE>) topic.getTypeSupport()).codec();
        }
        // A TypeSupport zerodds-idlc generated (ZdTypeSupports.Support_<tag>)
        // extends the PSM TypeSupport and is its own codec.
        if (topic.getTypeSupport() instanceof TopicTypeSupport) {
            return (TopicTypeSupport<TYPE>) topic.getTypeSupport();
        }
        throw new UnsupportedOperationException(
                "topic carries a TypeSupport of another implementation");
    }

    private void ensureOpen() {
        if (closed) {
            throw new AlreadyClosedException("Subscriber is closed");
        }
    }

    /** DDS 1.2 §2.2.2.3.2: a reader can only be created for an open topic. */
    static void ensureTopicOpen(Topic<?> topic) {
        if (org.zerodds.topic.TopicImpl.isClosed(topic)) {
            throw new AlreadyClosedException("Topic is closed");
        }
    }

    /** The partition names of this subscriber (DDS 1.2 §7.1.3.9). */
    private Set<String> partitions() {
        return new LinkedHashSet<>(qos.getPartition().getName());
    }

    // --- Lookup (§7.7.3) ---------------------------------------------------

    @SuppressWarnings("unchecked")
    @Override
    public <TYPE> DataReader<TYPE> lookupDataReader(String topicName) {
        org.zerodds.core.Args.require(topicName, "topicName");
        for (DataReader<?> reader : readers) {
            if (reader.getTopicDescription().getName().equals(topicName)) {
                return (DataReader<TYPE>) reader;
            }
        }
        return null;
    }

    @SuppressWarnings("unchecked")
    @Override
    public <TYPE> DataReader<TYPE> lookupDataReader(TopicDescription<TYPE> topic) {
        org.zerodds.core.Args.require(topic, "topic");
        if (topic == null) {
            throw new IllegalArgumentException("the topic description must not be null");
        }
        return lookupDataReader(topic.getName());
    }

    // --- Access scope (§7.7.4) ---------------------------------------------

    /**
     * DDS 1.4 §2.2.2.5.2.14 delete_contained_entities: every DataReader of this subscriber is
     * closed. A reader's close hook removes it from {@code readers} and withdraws it from the
     * built-in topic DCPSSubscription (see {@link #createDataReader}).
     */
    @Override
    public void closeContainedEntities() {
        ensureOpen();
        for (DataReader<?> reader : new ArrayList<>(readers)) {
            reader.close();
        }
    }

    /** DATA_ON_READERS (DDS 1.4 §2.2.4.1): a reader of this subscriber has new data. */
    void dataOnReadersChanged() {
        triggerStatus(org.omg.dds.core.status.DataOnReadersStatus.class);
    }

    /** The application read or took data, or was told through the listener: no longer changed. */
    void dataOnReadersAccessed() {
        clearStatus(org.omg.dds.core.status.DataOnReadersStatus.class);
    }

    /** §7.6.4: {@code readers} is the container the result is placed in. */
    @Override
    public Collection<DataReader<?>> getDataReaders(Collection<DataReader<?>> readers) {
        if (readers == null) {
            throw new IllegalArgumentException("the container for the readers must not be null");
        }
        requireGroupAccess();
        if (qos.getPresentation().isOrderedAccess() && qos.getPresentation().getAccessScope()
                == org.omg.dds.core.policy.Presentation.AccessScopeKind.GROUP) {
            return getDataReaders(readers, null); // a list in the order of reading (2.2.2.5.2.10)
        }
        readers.clear();
        readers.addAll(this.readers);
        return readers;
    }

    @Override
    public Collection<DataReader<?>> getDataReaders(Collection<DataReader<?>> readers,
            DataState dataState) {
        if (readers == null) {
            throw new IllegalArgumentException("the container for the readers must not be null");
        }
        requireGroupAccess();
        readers.clear();
        org.omg.dds.core.policy.Presentation presentation = qos.getPresentation();
        if (presentation.isOrderedAccess() && presentation.getAccessScope()
                == org.omg.dds.core.policy.Presentation.AccessScopeKind.GROUP) {
            // DDS 1.4 2.2.2.5.2.10: a list, each reader once for each sample it holds, in the order
            // the samples are to be read; the application reads one sample from each entry
            final class Entry {
                final DataReader<?> reader;
                final long[] key;

                Entry(DataReader<?> reader, long[] key) {
                    this.reader = reader;
                    this.key = key;
                }
            }
            List<Entry> entries = new java.util.ArrayList<>();
            for (DataReader<?> reader : this.readers) {
                for (long[] key : impl(reader).orderKeys(dataState)) {
                    entries.add(new Entry(reader, key));
                }
            }
            java.util.Collections.sort(entries, new java.util.Comparator<Entry>() {
                @Override
                public int compare(Entry a, Entry b) {
                    if (a.key[0] != b.key[0]) {
                        return Long.compare(a.key[0], b.key[0]);
                    }
                    return Long.compare(a.key[1], b.key[1]);
                }
            });
            for (Entry e : entries) {
                readers.add(e.reader);
            }
            return readers;
        }
        for (DataReader<?> reader : this.readers) {
            if (dataState == null || impl(reader).hasMatchingSample(dataState)) {
                readers.add(reader);
            }
        }
        return readers;
    }

    @Override
    public void notifyDataReaders() {
        for (DataReader<?> reader : readers) {
            impl(reader).notifyDataAvailable();
        }
    }

    @Override
    public void beginAccess() {
        ensureOpen();
        access.lock();
    }

    /**
     * DDS 1.4 §2.2.2.5.2.9: end_access closes a begin_access; without one it is
     * PRECONDITION_NOT_MET.
     */
    @Override
    public void endAccess() {
        ensureOpen();
        if (!access.isHeldByCurrentThread()) {
            // DDS 1.4 §2.2.2.5.2.9
            throw new PreconditionNotMetException("end_access without a matching begin_access");
        }
        access.unlock();
    }

    /**
     * DDS 1.4 §2.2.2.5.2.8: with PRESENTATION access_scope GROUP the sample-accessing
     * operations (read and take on a reader, get_datareaders) are PRECONDITION_NOT_MET unless
     * the calling thread called begin_access. With another access scope begin_access has no
     * effect and is not required.
     */
    void requireGroupAccess() {
        if (qos.getPresentation().getAccessScope()
                == org.omg.dds.core.policy.Presentation.AccessScopeKind.GROUP
                && !access.isHeldByCurrentThread()) {
            throw new org.zerodds.core.PreconditionNotMetException(
                    "the PRESENTATION access_scope is GROUP: begin_access is required before "
                    + "accessing samples");
        }
    }

    /**
     * The lock of the access scope (DDS 1.2 §2.2.2.2.3): the readers of this
     * subscriber take it for the duration of a read or take, so an application
     * can hold it across a sequence of operations.
     */
    public ReentrantLock accessLock() {
        return access;
    }

    // --- QoS (§7.7.5) ------------------------------------------------------

    @Override
    public DataReaderQos getDefaultDataReaderQos() {
        return defaultDataReaderQos;
    }

    @Override
    public synchronized void setDefaultDataReaderQos(DataReaderQos qos) {
        org.zerodds.core.Args.require(qos, "qos");
        org.zerodds.core.qos.QosRules.checkConsistent(qos);
        this.defaultDataReaderQos = qos;
    }

    /**
     * DDS 1.4 §2.2.2.5.2.17 copy_from_topic_qos: the policies the topic QoS has in common with
     * the DataReader QoS replace the ones in {@code drQos}, whatever value those had.
     */
    @Override
    public DataReaderQos copyFromTopicQos(DataReaderQos drQos, TopicQos tQos) {
        if (drQos == null || tQos == null) {
            throw new IllegalArgumentException("the QoS to copy from and into must not be null");
        }
        DataReaderQos merged = drQos;
        for (Map.Entry<Class<? extends QosPolicy>, ? extends QosPolicy> entry
                : org.zerodds.core.qos.QosCompatibility.wide(tQos).entrySet()) {
            if (entry.getValue() instanceof QosPolicy.ForDataReader) {
                merged = merged.withPolicy((QosPolicy.ForDataReader) entry.getValue());
            }
        }
        return merged;
    }

    @Override
    public SubscriberQos getQos() {
        return qos;
    }

    @Override
    public void setQos(SubscriberQos qos) {
        Set<String> before;
        Set<String> after;
        synchronized (this) {
            org.zerodds.core.qos.QosRules.checkChange(enabled, this.qos, qos);
            before = partitions();
            this.qos = qos;
            after = partitions();
        }
        // PARTITION can change (DDS 1.4 §2.2.3.13): the readers match again, outside the monitor
        // because the matches reach listeners.
        if (!before.equals(after)) {
            for (DataReader<?> reader : new ArrayList<>(readers)) {
                ((DataReaderImpl<?>) reader).changePartitions(after);
            }
        }
        // The built-in topic data of a reader carries the PARTITION and GROUP_DATA of its
        // Subscriber (DDS 1.4 §2.2.5): the readers are announced again with the new QoS.
        for (DataReader<?> reader : new ArrayList<>(readers)) {
            DataReaderImpl<?> impl = (DataReaderImpl<?>) reader;
            if (!impl.isClosed()) {
                impl.announceQos();
            }
        }
    }

    @Override
    protected SubscriberQos profileQos(QosProvider provider) {
        return provider.getSubscriberQos();
    }

    @Override
    public DataStateImpl createDataState() {
        return DataStateImpl.any(env());
    }

    // --- Entity (§7.3.6) ----------------------------------------------------

    @Override
    public DomainParticipant getParent() {
        return parent;
    }

    @SuppressWarnings("unchecked")
    @Override
    public StatusCondition<Subscriber> getStatusCondition() {
        return (StatusCondition<Subscriber>) super.getStatusCondition();
    }

    /** A reader of this subscriber as the ZeroDDS implementation. */
    private static <T> DataReaderImpl<T> impl(DataReader<T> reader) {
        if (reader instanceof DataReaderImpl) {
            return (DataReaderImpl<T>) reader;
        }
        throw new UnsupportedOperationException(
                "DataReader of another implementation in a ZeroDDS Subscriber");
    }

    /** The readers this subscriber has created (DDS 1.2 §2.2.2.2.1). */
    public List<DataReader<?>> getDataReaders() {
        return new ArrayList<>(readers);
    }

    /**
     * Whether a DataReader created now is enabled: this Subscriber is enabled and the
     * ENTITY_FACTORY of its QoS has autoenable_created_entities true (DDS 1.4 §2.2.3.20).
     */
    private boolean createsEnabled() {
        return enabled && autoEnablesCreated();
    }

    private boolean autoEnablesCreated() {
        SubscriberQos current = qos;
        return current.getEntityFactory() == null
                || current.getEntityFactory().isAutoEnableCreatedEntities();
    }

    /**
     * Enables the Subscriber. A DataReader created while the Subscriber was disabled is
     * enabled with it when the ENTITY_FACTORY says so (DDS 1.4 §2.2.2.1.1.7).
     */
    @Override
    public void enable() {
        if (closed) {
            throw new AlreadyClosedException("Subscriber is closed");
        }
        if (!enabled && parent instanceof org.zerodds.domain.DomainParticipantImpl
                && !((org.zerodds.domain.DomainParticipantImpl) parent).isEnabled()) {
            throw new org.zerodds.core.PreconditionNotMetException(
                    "the DomainParticipant of the Subscriber is not enabled");
        }
        boolean first = !enabled;
        enabled = true;
        if (first && (qos.getEntityFactory() == null
                || qos.getEntityFactory().isAutoEnableCreatedEntities())) {
            for (DataReader<?> reader : new ArrayList<>(readers)) {
                reader.enable();
            }
        }
    }

    /** The built-in Subscriber exists before its participant is enabled and is enabled with it. */
    public void enableBuiltin() {
        enabled = true;
        if (autoEnablesCreated()) {
            for (DataReader<?> reader : new ArrayList<>(readers)) {
                reader.enable();
            }
        }
    }

    public boolean isEnabled() {
        return enabled;
    }

    @Override
    public void close() {
        // §7.2.3: closing a Subscriber closes all contained DataReaders. The
        // flag goes up first, so a concurrent create call cannot slip a new
        // contained entity in behind the close (DDS-Java-PSM 1.0 §7.2.4).
        closed = true;
        for (DataReader<?> reader : new ArrayList<>(readers)) {
            reader.close(); // its close hook removes it and retracts it
        }
        runCloseHooks();
    }

    public boolean isClosed() {
        return closed;
    }

    @Override
    public String toString() {
        return "Subscriber[domain=" + domainId + "]";
    }
}