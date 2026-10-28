// SPDX-License-Identifier: Apache-2.0
package org.zerodds.domain;

import java.util.ArrayList;
import java.util.Collection;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.CopyOnWriteArrayList;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.TimeoutException;
import java.util.concurrent.atomic.AtomicInteger;

import org.omg.dds.core.Duration;
import org.omg.dds.core.InstanceHandle;
import org.omg.dds.core.ModifiableTime;
import org.omg.dds.core.QosProvider;
import org.omg.dds.core.ServiceEnvironment;
import org.omg.dds.core.StatusCondition;
import org.omg.dds.core.Time;
import org.omg.dds.core.status.Status;
import org.omg.dds.domain.DomainParticipant;
import org.omg.dds.domain.DomainParticipantListener;
import org.omg.dds.domain.DomainParticipantQos;
import org.omg.dds.pub.Publisher;
import org.omg.dds.pub.PublisherListener;
import org.omg.dds.pub.PublisherQos;
import org.omg.dds.sub.DataReaderQos;
import org.omg.dds.sub.Subscriber;
import org.omg.dds.sub.SubscriberListener;
import org.omg.dds.sub.SubscriberQos;
import org.omg.dds.topic.ContentFilteredTopic;
import org.omg.dds.topic.MultiTopic;
import org.omg.dds.topic.ParticipantBuiltinTopicData;
import org.omg.dds.topic.Topic;
import org.omg.dds.topic.TopicBuiltinTopicData;
import org.omg.dds.topic.TopicDescription;
import org.omg.dds.topic.TopicListener;
import org.omg.dds.topic.TopicQos;
import org.omg.dds.type.TypeSupport;
import org.omg.dds.type.dynamic.DynamicType;
import org.zerodds.cdr.TypeSupportResolver;
import org.zerodds.core.AlreadyClosedException;
import org.zerodds.core.EntityBase;
import org.zerodds.core.PreconditionNotMetException;
import org.zerodds.core.Values;
import org.zerodds.core.qos.DataReaderQosImpl;
import org.zerodds.core.qos.PublisherQosImpl;
import org.zerodds.core.qos.SubscriberQosImpl;
import org.zerodds.core.qos.TopicQosImpl;
import org.zerodds.internal.Ignores;
import org.zerodds.internal.InProcessBus;
import org.zerodds.pub.DataWriterImpl;
import org.zerodds.pub.PublisherImpl;
import org.zerodds.sub.ContentFilter;
import org.zerodds.sub.SubscriberImpl;
import org.zerodds.topic.BuiltinData;
import org.zerodds.topic.BuiltinQos;
import org.zerodds.topic.BuiltinTopicKeyImpl;
import org.zerodds.topic.BuiltinTopics;
import org.zerodds.topic.ContentFilteredTopicImpl;
import org.zerodds.topic.MultiTopicImpl;
import org.zerodds.topic.ParticipantBuiltinTopicDataImpl;
import org.zerodds.topic.TopicBuiltinTopicDataImpl;
import org.zerodds.topic.TopicImpl;

/**
 * ZeroDDS {@link DomainParticipant} (DDS-Java-PSM 1.0 §7.4.2).
 *
 * <p>The participant is the container of every other entity of an
 * application and the factory of its publishers, subscribers and topics
 * (§7.4.2).
 */
public final class DomainParticipantImpl
extends EntityBase<DomainParticipantListener, DomainParticipantQos>
implements DomainParticipant {

    /** How often {@code findTopic} re-reads the topic list while waiting. */
    private static final long POLL_MILLIS = 50L;

    private final DomainParticipantFactoryImpl factory;
    private final int domainId;
    private volatile DomainParticipantQos qos;
    private volatile PublisherQos defaultPublisherQos;
    private volatile SubscriberQos defaultSubscriberQos;
    private volatile TopicQos defaultTopicQos;

    private final List<Publisher> publishers = new CopyOnWriteArrayList<>();
    private final List<Subscriber> subscribers = new CopyOnWriteArrayList<>();
    private final List<Topic<?>> topics = new CopyOnWriteArrayList<>();
    private final List<TopicDescription<?>> filteredTopics =
            new CopyOnWriteArrayList<>();
    private final List<MultiTopic<?>> multiTopics = new CopyOnWriteArrayList<>();
    /** The topic handles the application ignored; the ones that name a topic are also in {@link Ignores}. */
    private final Set<InstanceHandle> ignoredTopics = ConcurrentHashMap.newKeySet();

    // What the RTPS discovery of other processes announced (DDSI-RTPS 2.5 §8.5).
    private final Map<InstanceHandle, org.zerodds.rtps.DiscoveryData.Participant> remoteParticipants =
            new ConcurrentHashMap<>();
    private final Map<InstanceHandle, org.zerodds.rtps.DiscoveryData.Endpoint> remoteEndpoints =
            new ConcurrentHashMap<>();
    private final Map<InstanceHandle, TopicBuiltinTopicDataImpl> remoteTopics =
            new ConcurrentHashMap<>();
    private final Map<InstanceHandle, Integer> remoteTopicEndpoints = new ConcurrentHashMap<>();

    /** The writers of this participant with LIVELINESS MANUAL_BY_PARTICIPANT (DDS 1.4 2.2.3.11). */
    private final AtomicInteger manualWriters = new AtomicInteger();

    private volatile Subscriber builtinSubscriber;
    volatile boolean closed = false;
    private volatile boolean enabled = false;

    /** The built-in topics and the writers that publish their data (§7.4.2). */
    private Builtin builtin;

    /** Set while the built-in entities are created, to keep them out of the
     *  data they themselves publish. */
    boolean creatingBuiltin = false;

    public DomainParticipantImpl(ServiceEnvironment environment,
            DomainParticipantFactoryImpl factory, int domainId,
            DomainParticipantQos qos, boolean autoEnable) {
        super(environment);
        this.factory = factory;
        this.domainId = domainId;
        this.qos = qos;
        this.defaultPublisherQos = PublisherQosImpl.defaults(environment);
        this.defaultSubscriberQos = SubscriberQosImpl.defaults(environment);
        this.defaultTopicQos = TopicQosImpl.defaults(environment);
        this.builtin = new Builtin(this, environment);
        this.builtin.announceParticipant();
        if (org.zerodds.rtps.DdsWireBridge.enabled()) {
            // The participants and endpoints of other processes reach the
            // built-in topics through SPDP and SEDP, with their QoS.
            org.zerodds.rtps.RtpsParticipant rtps = org.zerodds.rtps.RtpsParticipant.get(domainId);
            rtps.setUserData(qos.getUserData().getValue());
            this.builtin.attachRemote(rtps);
        }
        if (autoEnable) {
            this.enabled = true;
        }
    }

    // --- Create Publisher (§7.4.2) -----------------------------------------

    @Override
    public Publisher createPublisher() {
        return createPublisher(defaultPublisherQos);
    }

    @Override
    public Publisher createPublisher(PublisherQos qos) {
        org.zerodds.core.Args.require(qos, "qos");
        return newPublisher(qos, null, null);
    }

    @Override
    public Publisher createPublisher(PublisherQos qos, PublisherListener listener,
            Collection<Class<? extends Status>> statuses) {
        org.zerodds.core.Args.require(qos, "qos");
        return newPublisher(qos, listener, statuses);
    }

    /** The listener is set before the Publisher is enabled (DDS 1.4 §2.2.4.3). */
    private Publisher newPublisher(PublisherQos qos, PublisherListener listener,
            Collection<Class<? extends Status>> statuses) {
        ensureOpen();
        org.zerodds.core.qos.QosRules.checkConsistent(qos);
        PublisherImpl publisher = new PublisherImpl(
                getEnvironment(), domainId, this, qos);
        if (listener != null) {
            publisher.setListener(listener, statuses);
        }
        publishers.add(publisher);
        publisher.addCloseHook(() -> publishers.remove(publisher));
        enableIfFactorySays(publisher);
        return publisher;
    }

    // --- Create Subscriber (§7.4.2) ----------------------------------------

    @Override
    public Subscriber createSubscriber() {
        return createSubscriber(defaultSubscriberQos);
    }

    @Override
    public Subscriber createSubscriber(SubscriberQos qos) {
        org.zerodds.core.Args.require(qos, "qos");
        return newSubscriber(qos, null, null);
    }

    @Override
    public Subscriber createSubscriber(SubscriberQos qos,
            SubscriberListener listener,
            Collection<Class<? extends Status>> statuses) {
        org.zerodds.core.Args.require(qos, "qos");
        return newSubscriber(qos, listener, statuses);
    }

    private Subscriber newSubscriber(SubscriberQos qos, SubscriberListener listener,
            Collection<Class<? extends Status>> statuses) {
        ensureOpen();
        org.zerodds.core.qos.QosRules.checkConsistent(qos);
        SubscriberImpl subscriber = new SubscriberImpl(
                getEnvironment(), domainId, this, qos);
        if (listener != null) {
            subscriber.setListener(listener, statuses);
        }
        subscribers.add(subscriber);
        subscriber.addCloseHook(() -> subscribers.remove(subscriber));
        enableIfFactorySays(subscriber);
        return subscriber;
    }

    /**
     * DDS 1.4 §2.2.2.2.1.13 and §2.2.5: the built-in subscriber holds one DataReader for each
     * built-in topic, found with {@code lookupDataReader(topicName)}. They request
     * TRANSIENT_LOCAL and RELIABLE with HISTORY KEEP_LAST 1, and the built-in writers offer
     * TRANSIENT_LOCAL, so a reader gets what the domain already contains. The subscriber is
     * created once and is not one of the participant's application-visible subscribers.
     */
    @Override
    public Subscriber getBuiltinSubscriber() {
        Subscriber subscriber = builtinSubscriber;
        if (subscriber == null) {
            synchronized (this) {
                subscriber = builtinSubscriber;
                if (subscriber == null) {
                    ensureOpen();
                    SubscriberImpl created = new SubscriberImpl(
                            getEnvironment(), domainId, this,
                            SubscriberQosImpl.defaults(getEnvironment()));
                    created.enableBuiltin();
                    creatingBuiltin = true;
                    try {
                        DataReaderQos readerQos = DataReaderQosImpl.defaults(getEnvironment())
                                .withPolicy(getEnvironment().getSPI().getPolicyFactory()
                                        .Durability().withTransientLocal())
                                .withPolicy(getEnvironment().getSPI().getPolicyFactory()
                                        .Reliability().withReliable());
                        for (String name : new String[] {BuiltinTopics.PARTICIPANT,
                                BuiltinTopics.PUBLICATION, BuiltinTopics.SUBSCRIPTION,
                                BuiltinTopics.TOPIC}) {
                            created.createDataReader(lookupTopic(name), readerQos);
                        }
                    } finally {
                        creatingBuiltin = false;
                    }
                    builtinSubscriber = created;
                    subscriber = created;
                }
            }
        }
        return subscriber;
    }

    // --- Create Topic (§7.4.2) ---------------------------------------------

    @Override
    public <TYPE> Topic<TYPE> createTopic(String topicName, Class<TYPE> type) {
        org.zerodds.core.Args.require(topicName, "topicName");
        org.zerodds.core.Args.require(type, "type");
        return createTopic(topicName, type, defaultTopicQos);
    }

    @Override
    public <TYPE> Topic<TYPE> createTopic(String topicName, Class<TYPE> type,
            TopicQos qos, TopicListener<TYPE> listener,
            Collection<Class<? extends Status>> statuses) {
        org.zerodds.core.Args.require(topicName, "topicName");
        org.zerodds.core.Args.require(type, "type");
        org.zerodds.core.Args.require(qos, "qos");
        return newTopic(topicName, type, null, qos, listener, statuses);
    }

    @Override
    public <TYPE> Topic<TYPE> createTopic(String topicName,
            TypeSupport<TYPE> type) {
        org.zerodds.core.Args.require(topicName, "topicName");
        org.zerodds.core.Args.require(type, "type");
        return createTopic(topicName, type, defaultTopicQos);
    }

    @Override
    public <TYPE> Topic<TYPE> createTopic(String topicName,
            TypeSupport<TYPE> type, TopicQos qos, TopicListener<TYPE> listener,
            Collection<Class<? extends Status>> statuses) {
        org.zerodds.core.Args.require(topicName, "topicName");
        org.zerodds.core.Args.require(type, "type");
        org.zerodds.core.Args.require(qos, "qos");
        return newTopic(topicName, null, type, qos, listener, statuses);
    }

    /** The ZeroDDS form of {@code createTopic(String, TypeSupport, TopicQos)}. */
    public <TYPE> Topic<TYPE> createTopic(String topicName,
            TypeSupport<TYPE> type, TopicQos qos) {
        org.zerodds.core.Args.require(topicName, "topicName");
        org.zerodds.core.Args.require(type, "type");
        org.zerodds.core.Args.require(qos, "qos");
        return newTopic(topicName, null, type, qos, null, null);
    }

    /** The ZeroDDS form of {@code createTopic(String, Class, TopicQos)}. */
    public <TYPE> Topic<TYPE> createTopic(String topicName, Class<TYPE> type,
            TopicQos qos) {
        org.zerodds.core.Args.require(topicName, "topicName");
        org.zerodds.core.Args.require(type, "type");
        org.zerodds.core.Args.require(qos, "qos");
        return newTopic(topicName, type, null, qos, null, null);
    }

    /**
     * Creates a topic of a class or of a {@link TypeSupport} (exactly one is given). The
     * listener is set before the topic is enabled and announced (DDS 1.4 §2.2.4.3).
     */
    private <TYPE> Topic<TYPE> newTopic(String topicName, Class<TYPE> type,
            TypeSupport<TYPE> support, TopicQos qos, TopicListener<TYPE> listener,
            Collection<Class<? extends Status>> statuses) {
        ensureOpen();
        org.zerodds.core.qos.QosRules.checkConsistent(qos);
        if (type != null) {
            if (BuiltinTopics.isBuiltin(topicName)) {
                // §2.2.1.3.1: the name is taken by the built-in topic, whose data is
                // the structure of §2.2.5.
                if (!BuiltinTopics.isDataType(topicName, type)) {
                    throw new IllegalArgumentException(topicName
                            + " is the built-in topic of type " + BuiltinTopics.typeNameOf(topicName)
                            + ", not " + type.getName());
                }
                return builtinTopic(topicName, BuiltinTopics.typeNameOf(topicName));
            }
            return created(new TopicImpl<>(getEnvironment(), this, topicName, type, qos),
                    listener, statuses);
        }
        Topic<TYPE> builtinTopic = builtinTopic(topicName, support.getTypeName());
        if (builtinTopic != null) {
            return builtinTopic;
        }
        return created(new TopicImpl<>(getEnvironment(), this, topicName, support, qos),
                listener, statuses);
    }

    /**
     * §2.2.1.3.1: the built-in topics are created by the participant itself;
     * {@code create_topic} with their name returns the built-in topic when
     * the type is the built-in one.
     *
     * @return the built-in topic, or {@code null} if {@code topicName} is not
     *         a built-in topic at all.
     */
    @SuppressWarnings("unchecked")
    private <T> Topic<T> builtinTopic(String topicName, String typeName) {
        if (!BuiltinTopics.isBuiltin(topicName)) {
            return null;
        }
        Topic<?> existing = lookupTopic(topicName);
        if (existing == null || !existing.getTypeName().equals(typeName)) {
            throw new IllegalArgumentException(topicName
                    + " is the built-in topic of type "
                    + BuiltinTopics.typeNameOf(topicName));
        }
        return (Topic<T>) existing;
    }

    /**
     * §7.5.1 without a {@link TypeSupport}: the Service locates the type
     * support from the {@link DynamicType} (here, the XCDR2 codec of
     * {@link org.zerodds.cdr.DynamicDataTypeSupport}). PSM 1.0 types the
     * result {@code Topic<DynamicType>}; its samples are
     * {@link org.omg.dds.type.dynamic.DynamicData}. Use
     * {@link DynamicTopics#dataTopic} for the {@code Topic<DynamicData>} that
     * readers and writers are created on, or {@link #createDynamicTopic}.
     */
    @Override
    public Topic<DynamicType> createTopic(String topicName, DynamicType type) {
        org.zerodds.core.Args.require(topicName, "topicName");
        org.zerodds.core.Args.require(type, "type");
        return DynamicTopics.asDynamicType(createDynamicTopic(topicName, type, defaultTopicQos));
    }

    @Override
    public Topic<DynamicType> createTopic(String topicName, DynamicType type,
            TopicQos qos, TopicListener<DynamicType> listener,
            Collection<Class<? extends Status>> statuses) {
        org.zerodds.core.Args.require(topicName, "topicName");
        org.zerodds.core.Args.require(type, "type");
        org.zerodds.core.Args.require(qos, "qos");
        Topic<DynamicType> topic = DynamicTopics.asDynamicType(
                createDynamicTopic(topicName, type, qos));
        topic.setListener(listener, statuses);
        return topic;
    }

    /** The topic of the samples of {@code type}, typed as its samples are. */
    public Topic<org.omg.dds.type.dynamic.DynamicData> createDynamicTopic(String topicName,
            DynamicType type) {
        return createDynamicTopic(topicName, type, defaultTopicQos);
    }

    /** As above, with the QoS of the topic. */
    public Topic<org.omg.dds.type.dynamic.DynamicData> createDynamicTopic(String topicName,
            DynamicType type, TopicQos qos) {
        if (type == null) {
            throw new IllegalArgumentException("the DynamicType of a topic must not be null");
        }
        org.zerodds.cdr.DynamicDataTypeSupport codec =
                new org.zerodds.cdr.DynamicDataTypeSupport(getEnvironment(), type);
        TypeSupport<org.omg.dds.type.dynamic.DynamicData> support = new org.zerodds.cdr.TypeSupportImpl<>(
                getEnvironment(), type.getName(), org.omg.dds.type.dynamic.DynamicData.class,
                codec, codec::newData);
        return createTopic(topicName, support, qos);
    }

    @Override
    public Topic<DynamicType> createTopic(String topicName, DynamicType type,
            TypeSupport<DynamicType> typeSupport) {
        org.zerodds.core.Args.require(topicName, "topicName");
        org.zerodds.core.Args.require(type, "type");
        if (typeSupport == null) {
            // §7.5.1: the TypeSupport is what the topic serializes with.
            throw new IllegalArgumentException(
                    "the TypeSupport of a Topic must not be null");
        }
        return createTopic(topicName, typeSupport, defaultTopicQos);
    }

    @Override
    public Topic<DynamicType> createTopic(String topicName, DynamicType type,
            TypeSupport<DynamicType> typeSupport, TopicQos qos,
            TopicListener<DynamicType> listener,
            Collection<Class<? extends Status>> statuses) {
        org.zerodds.core.Args.require(topicName, "topicName");
        org.zerodds.core.Args.require(type, "type");
        org.zerodds.core.Args.require(qos, "qos");
        return newTopic(topicName, null, typeSupport, qos, listener, statuses);
    }

    /**
     * §7.3.6: the ENTITY_FACTORY of this participant decides whether what it
     * creates is enabled right away (DDS 1.2 §7.1.3.7, default true); what is
     * created by a participant that is not enabled is not enabled whatever it says
     * (DDS 1.4 §2.2.2.1.1.7).
     */
    private void enableIfFactorySays(org.omg.dds.core.Entity<?, ?> entity) {
        if (enabled && autoEnableCreated()) {
            entity.enable();
        }
    }

    private boolean autoEnableCreated() {
        return qos.getEntityFactory() == null
                || qos.getEntityFactory().isAutoEnableCreatedEntities();
    }

    /** A topic the application created: listener, enabled, contained, announced. */
    private <TYPE> Topic<TYPE> created(TopicImpl<TYPE> topic, TopicListener<TYPE> listener,
            Collection<Class<? extends Status>> statuses) {
        if (!BuiltinTopics.isBuiltin(topic.getName())) {
            // DDS-XTypes 1.3 §7.6.4.1: several topics of one name are allowed "provided
            // that all of them represent type-based subsets of 'the same' network topic",
            // and a network topic has one QoS. A topic with other policies than the one
            // of its name is refused, and the topic that was there is told.
            for (Topic<?> existing : topics) {
                if (existing.getName().equals(topic.getName())
                        && !existing.getQos().equals(topic.getQos())) {
                    if (existing instanceof TopicImpl) {
                        ((TopicImpl<?>) existing).onInconsistentTopic();
                    }
                    throw new org.zerodds.core.InconsistentPolicyException(getEnvironment(),
                            "a topic named " + topic.getName()
                                    + " exists with other policies (DDS-XTypes 1.3 §7.6.4.1)");
                }
            }
        }
        if (listener != null) {
            topic.setListener(listener, statuses);
        }
        enableIfFactorySays(topic);
        addTopic(topic);
        if (!BuiltinTopics.isBuiltin(topic.getName())) {
            builtin.announceTopic(topic);
            checkTopicConsistency(topic);
        }
        return topic;
    }

    /**
     * DDS 1.4 §2.2.4.1 INCONSISTENT_TOPIC: another participant of the domain has a topic of
     * the same name with another type; the status of both topics changes.
     */
    private void checkTopicConsistency(TopicImpl<?> topic) {
        for (DomainParticipantImpl other : DomainParticipantFactoryImpl.participantsInDomain(domainId)) {
            if (other == this) {
                continue;
            }
            for (Topic<?> theirs : other.containedTopics()) {
                if (theirs.getName().equals(topic.getName())
                        && !BuiltinTopics.isBuiltin(theirs.getName())
                        && !theirs.getTypeName().equals(topic.getTypeName())
                        && theirs instanceof TopicImpl) {
                    topic.onInconsistentTopic();
                    ((TopicImpl<?>) theirs).onInconsistentTopic();
                }
            }
        }
    }

    // --- Find and look up (§7.4.2) -----------------------------------------

    @SuppressWarnings("unchecked")
    @Override
    public <TYPE> Topic<TYPE> findTopic(String topicName, Duration timeout)
            throws TimeoutException {
        org.zerodds.core.Args.require(topicName, "topicName");
        org.zerodds.core.Args.require(timeout, "timeout");
        Topic<?> topic = lookupTopic(topicName);
        if (topic != null) {
            return (Topic<TYPE>) topic;
        }
        // The topics of this participant are the ones it can find: the wait is kept
        // because find_topic is specified to block for the timeout, and a topic another
        // thread of the application creates in the meantime is found
        // (DDS 1.2 §2.2.1.2.4).
        long millis = timeout == null
                ? 0L : timeout.getDuration(TimeUnit.MILLISECONDS);
        Topic<?> found = millis == Long.MAX_VALUE
                ? awaitTopic(topicName, Long.MAX_VALUE)
                : awaitTopic(topicName, System.nanoTime() + millis * 1_000_000L);
        if (found != null) {
            return (Topic<TYPE>) found;
        }
        throw new TimeoutException("no Topic named " + topicName);
    }

    /**
     * Waits until {@code topicName} is created or {@code deadlineNanos} has
     * passed ({@code Long.MAX_VALUE} waits forever). The participant's own
     * monitor is not held, so a {@code createTopic} on another thread is seen.
     *
     * @return the topic, or {@code null} if the deadline passed.
     */
    private Topic<?> awaitTopic(String topicName, long deadlineNanos) {
        while (deadlineNanos == Long.MAX_VALUE
                || System.nanoTime() < deadlineNanos) {
            try {
                Thread.sleep(POLL_MILLIS);
            } catch (InterruptedException interrupted) {
                Thread.currentThread().interrupt();
                return null;
            }
            Topic<?> topic = lookupTopic(topicName);
            if (topic != null) {
                return topic;
            }
        }
        return null;
    }

    @Override
    public <TYPE> Topic<TYPE> findTopic(String topicName, long timeout,
            TimeUnit unit) throws TimeoutException {
        org.zerodds.core.Args.require(topicName, "topicName");
        org.zerodds.core.Args.require(unit, "unit");
        return findTopic(topicName, org.zerodds.core.Values.durationOfMillis(
                getEnvironment(), unit.toMillis(timeout)));
    }

    @SuppressWarnings("unchecked")
    @Override
    public <TYPE> TopicDescription<TYPE> lookupTopicDescription(String name) {
        org.zerodds.core.Args.require(name, "name");
        for (TopicDescription<?> description : allTopicDescriptions()) {
            if (description.getName().equals(name)) {
                return (TopicDescription<TYPE>) description;
            }
        }
        return null;
    }

    /**
     * DDS-XTypes 1.3 §7.6.4.2: "The lookup_topicdescription operation shall
     * accept an optional in unsigned long argument called index. [...] Values
     * of the index from 0 to one less than the number of different
     * TopicDescriptions [...] shall return a TopicDescription and values of
     * the index outside the range shall return nil." The topic descriptions of
     * one name are taken in the order they were created, the topics first.
     * The interface of the OMG Java API has the operation without the index; a
     * negative {@code index} is outside the range.
     */
    @SuppressWarnings("unchecked")
    public <TYPE> TopicDescription<TYPE> lookupTopicDescription(String name, int index) {
        if (index < 0) {
            return null;
        }
        int seen = 0;
        for (TopicDescription<?> description : allTopicDescriptions()) {
            if (description.getName().equals(name)) {
                if (seen == index) {
                    return (TopicDescription<TYPE>) description;
                }
                seen++;
            }
        }
        return null;
    }

    /** The topic of {@code topicName} that this participant created, if any. */
    public Topic<?> lookupTopic(String topicName) {
        for (Topic<?> topic : topics) {
            if (topic.getName().equals(topicName)) {
                return topic;
            }
        }
        return null;
    }

    /** Adds a topic the participant created itself (the built-in ones). */
    void addTopic(Topic<?> topic) {
        topics.add(topic);
        if (topic instanceof org.zerodds.topic.TopicImpl) {
            ((org.zerodds.topic.TopicImpl<?>) topic).addCloseHook(() -> {
                topics.remove(topic);
                if (!BuiltinTopics.isBuiltin(topic.getName())) {
                    builtin.retractTopic(topic);
                }
            });
        }
    }

    /** The QoS of a topic changed (TOPIC_DATA, DEADLINE, ...): "DCPSTopic" tells the domain. */
    public void topicQosChanged(Topic<?> topic) {
        if (!closed && topics.contains(topic) && !BuiltinTopics.isBuiltin(topic.getName())) {
            builtin.announceTopic(topic);
        }
    }

    List<Publisher> containedPublishers() {
        return new ArrayList<>(publishers);
    }

    List<Subscriber> containedSubscribers() {
        return new ArrayList<>(subscribers);
    }

    List<Topic<?>> containedTopics() {
        return new ArrayList<>(topics);
    }

    /** Every topic description of this participant, filtered ones included. */
    public List<TopicDescription<?>> allTopicDescriptions() {
        List<TopicDescription<?>> all = new ArrayList<>(topics);
        all.addAll(filteredTopics);
        all.addAll(multiTopics);
        return all;
    }

    /** The topics this participant has created (DDS-DCPS 1.4 §2.2.1.2.2). */
    public List<Topic<?>> getTopics() {
        return new ArrayList<>(topics);
    }

    // --- Content filter (§7.4.2) -------------------------------------------

    @Override
    public <TYPE> ContentFilteredTopic<TYPE> createContentFilteredTopic(
            String name, Topic<? extends TYPE> relatedTopic,
            String filterExpression, List<String> expressionParameters) {
        org.zerodds.core.Args.require(name, "name");
        org.zerodds.core.Args.require(relatedTopic, "relatedTopic");
        org.zerodds.core.Args.require(filterExpression, "filterExpression");
        ensureOpen();
        if (relatedTopic == null) {
            throw new IllegalArgumentException("relatedTopic must not be null");
        }
        if (TopicImpl.isClosed(relatedTopic)) {
            throw new AlreadyClosedException("the related Topic is closed");
        }
        java.util.function.Predicate<TYPE> matches =
                ContentFilter.compile(filterExpression, expressionParameters);
        ContentFilteredTopicImpl<TYPE> filtered =
                new ContentFilteredTopicImpl<>(getEnvironment(), name,
                        relatedTopic, filterExpression, expressionParameters,
                        matches);
        filteredTopics.add(filtered);
        filtered.addCloseHook(() -> filteredTopics.remove(filtered));
        return filtered;
    }

    @Override
    public <TYPE> MultiTopic<TYPE> createMultiTopic(String name,
            Class<TYPE> type, String subscriptionExpression,
            List<String> expressionParameters) {
        org.zerodds.core.Args.require(name, "name");
        org.zerodds.core.Args.require(type, "type");
        org.zerodds.core.Args.require(subscriptionExpression, "subscriptionExpression");
        return createMultiTopic(name, newTypeSupport(type), subscriptionExpression,
                expressionParameters);
    }

    @Override
    public <TYPE> MultiTopic<TYPE> createMultiTopic(String name,
            TypeSupport<TYPE> type, String subscriptionExpression,
            List<String> expressionParameters) {
        org.zerodds.core.Args.require(name, "name");
        org.zerodds.core.Args.require(type, "type");
        org.zerodds.core.Args.require(subscriptionExpression, "subscriptionExpression");
        ensureOpen();
        if (type == null) {
            throw new IllegalArgumentException("the resulting type must not be null");
        }
        MultiTopicImpl<TYPE> multiTopic = MultiTopicImpl.of(getEnvironment(),
                name, type, this, subscriptionExpression, expressionParameters);
        // Annex B: the FROM clause names topics of this participant.
        List<Topic<?>> sources = new ArrayList<>();
        for (String topicName : multiTopic.sourceNames()) {
            Topic<?> source = lookupTopic(topicName);
            if (source == null) {
                throw new IllegalArgumentException("the subscription expression '"
                        + subscriptionExpression + "' reads the topic '" + topicName
                        + "', which this participant has not created");
            }
            sources.add(source);
        }
        multiTopic.bind(sources);
        multiTopics.add(multiTopic);
        multiTopic.addCloseHook(() -> multiTopics.remove(multiTopic));
        return multiTopic;
    }

    /** The TypeSupport of a data class, resolved as §7.5.1 describes. */
    private <TYPE> TypeSupport<TYPE> newTypeSupport(Class<TYPE> type) {
        org.zerodds.cdr.TopicTypeSupport<TYPE> codec =
                TypeSupportResolver.resolve(type);
        return new org.zerodds.cdr.TypeSupportImpl<>(getEnvironment(),
                codec.getTypeName(), type, codec);
    }

    // --- Contained entities (§7.4.2) ---------------------------------------

    @Override
    public void closeContainedEntities() {
        for (Publisher publisher : new ArrayList<>(publishers)) {
            publisher.close();
        }
        for (Subscriber subscriber : new ArrayList<>(subscribers)) {
            subscriber.close();
        }
        for (TopicDescription<?> filtered : new ArrayList<>(filteredTopics)) {
            filtered.close();
        }
        for (MultiTopic<?> multiTopic : new ArrayList<>(multiTopics)) {
            multiTopic.close();
        }
        // The topics of the application leave the built-in topic as they close (their hook);
        // the built-in topics are used by the built-in entities, which close next.
        for (Topic<?> topic : new ArrayList<>(topics)) {
            if (!BuiltinTopics.isBuiltin(topic.getName())) {
                topic.close();
            }
        }
        Subscriber builtinSubscriber = this.builtinSubscriber;
        if (builtinSubscriber != null) {
            builtinSubscriber.close();
        }
        // After the retractions above, which still need a writer to write on.
        builtin.close();
        for (Topic<?> topic : new ArrayList<>(topics)) {
            topic.close();
        }
    }

    // --- The built-in topics (§2.2.1.3.1) ----------------------------------

    // The built-in topics are created by the Builtin constructor through the
    // very publishers and subscribers that report back here, so the field is
    // still null (or the entities are the built-in ones) during that step.

    /** "DCPSPublication" announced for a DataWriter this participant owns. */
    void onWriterCreated(org.omg.dds.pub.DataWriter<?> writer,
            Publisher parent) {
        Builtin topics = builtin;
        if (topics != null) {
            topics.announceWriter(writer, parent);
        }
    }

    /** "DCPSSubscription" announced for a DataReader this participant owns. */
    void onReaderCreated(org.omg.dds.sub.DataReader<?> reader,
            SubscriberQos parentQos) {
        Builtin topics = builtin;
        if (topics != null) {
            topics.announceReader(reader, parentQos);
        }
    }

    /** "DCPSPublication" withdrawn for a DataWriter that goes away. */
    void onWriterClosed(org.omg.dds.pub.DataWriter<?> writer) {
        Builtin topics = builtin;
        if (topics != null) {
            topics.retractWriter(writer);
        }
    }

    /** "DCPSSubscription" withdrawn for a DataReader that goes away. */
    void onReaderClosed(org.omg.dds.sub.DataReader<?> reader) {
        Builtin topics = builtin;
        if (topics != null) {
            topics.retractReader(reader);
        }
    }

    // --- Ignore (§7.4.2) ---------------------------------------------------

    /**
     * DDS 1.4 §2.2.2.2.1.14: from now on this participant behaves as if the participant of
     * {@code handle} did not exist: none of its topics, publications and subscriptions is
     * matched with an endpoint of this participant, and it is not listed as discovered.
     * Not reversible.
     */
    @Override
    public void ignoreParticipant(InstanceHandle handle) {
        ensureOpen();
        checkHandle(handle);
        Ignores.of(getInstanceHandle()).ignoreParticipant(handle);
        if (org.zerodds.rtps.DdsWireBridge.enabled()) {
            org.zerodds.rtps.RtpsParticipant.get(domainId).ignoreParticipant(
                    java.util.Arrays.copyOf(org.zerodds.core.InstanceHandleImpl.bytesOf(handle), 12));
        }
        InProcessBus.instance().rematchIgnored();
    }

    /**
     * DDS 1.4 §2.2.2.2.1.15: no publication or subscription of the topic is matched any more
     * with an endpoint of this participant, and the topic is not listed as discovered.
     */
    @Override
    public void ignoreTopic(InstanceHandle handle) {
        ensureOpen();
        checkHandle(handle);
        ignoredTopics.add(handle);
        String name = topicNameOf(handle);
        if (name != null) {
            Ignores.of(getInstanceHandle()).ignoreTopic(name);
            if (org.zerodds.rtps.DdsWireBridge.enabled()) {
                org.zerodds.rtps.RtpsParticipant.get(domainId).ignoreTopic(name);
            }
        }
        InProcessBus.instance().rematchIgnored();
    }

    /**
     * DDS 1.4 §2.2.2.2.1.16: the DataWriter of {@code handle} is ignored: none of its data
     * reaches a reader of this participant, and no reader of it is matched with it.
     */
    @Override
    public void ignorePublication(InstanceHandle handle) {
        ensureOpen();
        checkHandle(handle);
        Ignores.of(getInstanceHandle()).ignorePublication(handle);
        if (org.zerodds.rtps.DdsWireBridge.enabled()) {
            org.zerodds.rtps.RtpsParticipant.get(domainId).ignoreEndpoint(
                    org.zerodds.core.InstanceHandleImpl.bytesOf(handle).clone());
        }
        InProcessBus.instance().rematchIgnored();
    }

    /**
     * DDS 1.4 §2.2.2.2.1.17: the DataReader of {@code handle} is ignored: no writer of this
     * participant is matched with it or sends it data.
     */
    @Override
    public void ignoreSubscription(InstanceHandle handle) {
        ensureOpen();
        checkHandle(handle);
        Ignores.of(getInstanceHandle()).ignoreSubscription(handle);
        if (org.zerodds.rtps.DdsWireBridge.enabled()) {
            org.zerodds.rtps.RtpsParticipant.get(domainId).ignoreEndpoint(
                    org.zerodds.core.InstanceHandleImpl.bytesOf(handle).clone());
        }
        InProcessBus.instance().rematchIgnored();
    }

    private static void checkHandle(InstanceHandle handle) {
        if (handle == null) {
            throw new IllegalArgumentException("the instance handle must not be null");
        }
    }

    /** The name of the topic a topic handle names: a topic of this domain, local or remote. */
    private String topicNameOf(InstanceHandle handle) {
        for (DomainParticipantImpl participant : DomainParticipantFactoryImpl.participantsInDomain(domainId)) {
            for (Topic<?> topic : participant.containedTopics()) {
                if (topic.getInstanceHandle().equals(handle)
                        && !BuiltinTopics.isBuiltin(topic.getName())) {
                    return topic.getName();
                }
            }
        }
        TopicBuiltinTopicDataImpl remote = remoteTopics.get(handle);
        return remote == null ? null : remote.getName();
    }

    // --- The participant itself (§7.4.2) -----------------------------------

    @Override
    public int getDomainId() {
        return domainId;
    }

    /**
     * §7.4.2: assert_liveliness affects only the DataWriters of this
     * participant whose LIVELINESS is MANUAL_BY_PARTICIPANT (DDS 1.4
     * §2.2.2.2.1.19). A write of any writer of the participant asserts it too.
     */
    @Override
    public void assertLiveliness() {
        ensureOpen();
        renewManualByParticipant();
    }

    /**
     * Renews the liveliness of every DataWriter of this participant whose LIVELINESS is
     * MANUAL_BY_PARTICIPANT (DDS 1.4 §2.2.3.11, §2.2.2.2.1.7).
     */
    /** The liveliness of the participant was asserted: the MANUAL_BY_PARTICIPANT writers renew. */
    public void livelinessAsserted() {
        if (manualWriters.get() == 0) {
            return;
        }
        renewManualByParticipant();
    }

    /** A writer with LIVELINESS MANUAL_BY_PARTICIPANT was enabled in this participant. */
    public void manualWriterAdded() {
        manualWriters.incrementAndGet();
    }

    /** That writer was closed. */
    public void manualWriterRemoved() {
        manualWriters.decrementAndGet();
    }

    public void renewManualByParticipant() {
        for (Publisher publisher : new ArrayList<>(publishers)) {
            if (!(publisher instanceof PublisherImpl)) {
                continue;
            }
            for (org.omg.dds.pub.DataWriter<?> writer : ((PublisherImpl) publisher).getDataWriters()) {
                if (writer instanceof org.zerodds.pub.DataWriterImpl
                        && writer.getQos().getLiveliness().getKind()
                                == org.omg.dds.core.policy.Liveliness.Kind.MANUAL_BY_PARTICIPANT
                        && !((org.zerodds.pub.DataWriterImpl<?>) writer).isClosed()) {
                    ((org.zerodds.pub.DataWriterImpl<?>) writer).renewLiveliness();
                    ((org.zerodds.pub.DataWriterImpl<?>) writer).assertOnWire();
                }
            }
        }
    }

    @Override
    public PublisherQos getDefaultPublisherQos() {
        return defaultPublisherQos;
    }

    @Override
    public void setDefaultPublisherQos(PublisherQos qos) {
        org.zerodds.core.Args.require(qos, "qos");
        org.zerodds.core.qos.QosRules.checkConsistent(qos);
        this.defaultPublisherQos = qos;
    }

    @Override
    public SubscriberQos getDefaultSubscriberQos() {
        return defaultSubscriberQos;
    }

    @Override
    public void setDefaultSubscriberQos(SubscriberQos qos) {
        org.zerodds.core.Args.require(qos, "qos");
        org.zerodds.core.qos.QosRules.checkConsistent(qos);
        this.defaultSubscriberQos = qos;
    }

    @Override
    public TopicQos getDefaultTopicQos() {
        return defaultTopicQos;
    }

    @Override
    public void setDefaultTopicQos(TopicQos qos) {
        org.zerodds.core.Args.require(qos, "qos");
        org.zerodds.core.qos.QosRules.checkConsistent(qos);
        this.defaultTopicQos = qos;
    }

    // --- Discovery (§7.4.2) ------------------------------------------------

    /** The handle of a participant another process announced through SPDP. */
    private InstanceHandle handleOf(org.zerodds.rtps.DiscoveryData.Participant participant) {
        return Values.of(getEnvironment(), BuiltinData.participantGuid(participant.guidPrefix));
    }

    /** SPDP: a participant of another process appeared. */
    void remoteParticipantSeen(org.zerodds.rtps.DiscoveryData.Participant participant) {
        remoteParticipants.put(handleOf(participant), participant);
    }

    /** SPDP: a participant of another process is gone. */
    void remoteParticipantLost(org.zerodds.rtps.DiscoveryData.Participant participant) {
        remoteParticipants.remove(handleOf(participant));
    }

    /** The handle of the topic of that name and type, which DDSI-RTPS 2.5 does not give one. */
    private InstanceHandle remoteTopicHandle(String name, String typeName) {
        try {
            java.security.MessageDigest md5 = java.security.MessageDigest.getInstance("MD5");
            md5.update(name.getBytes(java.nio.charset.StandardCharsets.UTF_8));
            md5.update((byte) 0);
            md5.update(typeName.getBytes(java.nio.charset.StandardCharsets.UTF_8));
            return Values.of(getEnvironment(), md5.digest());
        } catch (java.security.NoSuchAlgorithmException e) {
            throw new IllegalStateException(e);
        }
    }

    /**
     * SEDP: a publication or subscription of another process appeared. Its topic is a
     * discovered topic, and a topic of this participant of the same name with another type
     * is inconsistent with it (DDS 1.4 §2.2.4.1 INCONSISTENT_TOPIC).
     */
    void remoteEndpointSeen(org.zerodds.rtps.DiscoveryData.Endpoint endpoint) {
        remoteEndpoints.put(Values.of(getEnvironment(), endpoint.endpointGuid), endpoint);
        if (endpoint.topicName == null || endpoint.typeName == null
                || BuiltinTopics.isBuiltin(endpoint.topicName)) {
            return;
        }
        InstanceHandle topic = remoteTopicHandle(endpoint.topicName, endpoint.typeName);
        if (remoteTopicEndpoints.merge(topic, 1, Integer::sum) == 1) {
            TopicBuiltinTopicDataImpl data = new TopicBuiltinTopicDataImpl(getEnvironment());
            data.setKey(BuiltinTopicKeyImpl.of(getEnvironment(), topic));
            data.setName(endpoint.topicName);
            data.setTypeName(endpoint.typeName);
            BuiltinQos.applyTo(data, endpoint.qos, getEnvironment());
            remoteTopics.put(topic, data);
        }
        for (Topic<?> mine : topics) {
            if (mine.getName().equals(endpoint.topicName)
                    && !mine.getTypeName().equals(endpoint.typeName)
                    && mine instanceof TopicImpl) {
                ((TopicImpl<?>) mine).onInconsistentTopic();
            }
        }
    }

    /** SEDP: a publication or subscription of another process is gone. */
    void remoteEndpointLost(org.zerodds.rtps.DiscoveryData.Endpoint endpoint) {
        remoteEndpoints.remove(Values.of(getEnvironment(), endpoint.endpointGuid));
        if (endpoint.topicName == null || endpoint.typeName == null) {
            return;
        }
        InstanceHandle topic = remoteTopicHandle(endpoint.topicName, endpoint.typeName);
        Integer left = remoteTopicEndpoints.computeIfPresent(topic, (k, n) -> n > 1 ? n - 1 : null);
        if (left == null) {
            remoteTopics.remove(topic);
        }
    }

    /**
     * DDS 1.4 §2.2.2.2.1.27: the participants of the domain, those of this process (this one
     * included) and the ones SPDP announced, without the ones this participant ignores.
     */
    @Override
    public Set<InstanceHandle> getDiscoveredParticipants() {
        Ignores.Rules ignored = Ignores.of(getInstanceHandle());
        Set<InstanceHandle> handles = ConcurrentHashMap.newKeySet();
        for (DomainParticipantImpl participant
                : DomainParticipantFactoryImpl.participantsInDomain(domainId)) {
            InstanceHandle handle = participant.getInstanceHandle();
            if (!ignored.ignoresParticipant(handle)) {
                handles.add(handle);
            }
        }
        for (InstanceHandle handle : remoteParticipants.keySet()) {
            if (!ignored.ignoresParticipant(handle)) {
                handles.add(handle);
            }
        }
        return handles;
    }

    /**
     * DDS 1.4 §2.2.2.2.1.28: PRECONDITION_NOT_MET for a handle that is not a discovered
     * participant of this domain or is one this participant ignores.
     */
    @Override
    public ParticipantBuiltinTopicData getDiscoveredParticipantData(
            InstanceHandle participantHandle) {
        checkHandle(participantHandle);
        if (Ignores.of(getInstanceHandle()).ignoresParticipant(participantHandle)) {
            throw new PreconditionNotMetException(
                    participantHandle + " is a DomainParticipant this participant ignores");
        }
        for (DomainParticipantImpl participant
                : DomainParticipantFactoryImpl.participantsInDomain(domainId)) {
            if (participant.getInstanceHandle().equals(participantHandle)) {
                return new ParticipantBuiltinTopicDataImpl(
                        getEnvironment(),
                        BuiltinTopicKeyImpl.of(getEnvironment(), participantHandle),
                        participant.getQos().getUserData());
            }
        }
        org.zerodds.rtps.DiscoveryData.Participant remote = remoteParticipants.get(participantHandle);
        if (remote != null) {
            return BuiltinData.participant(getEnvironment(), remote);
        }
        throw new PreconditionNotMetException(
                participantHandle + " is not a discovered DomainParticipant");
    }

    /**
     * DDS 1.4 §2.2.2.2.1.29: the topics of the domain: those the participants of this process
     * created, and the ones the publications and subscriptions of other processes name, without
     * the ones this participant ignores and the ones of participants it ignores.
     */
    @Override
    public Set<InstanceHandle> getDiscoveredTopics() {
        Ignores.Rules ignored = Ignores.of(getInstanceHandle());
        Set<InstanceHandle> handles = ConcurrentHashMap.newKeySet();
        for (DomainParticipantImpl participant
                : DomainParticipantFactoryImpl.participantsInDomain(domainId)) {
            if (ignored.ignoresParticipant(participant.getInstanceHandle())
                    && participant != this) {
                continue;
            }
            for (Topic<?> topic : participant.containedTopics()) {
                if (!BuiltinTopics.isBuiltin(topic.getName())
                        && !ignoredTopics.contains(topic.getInstanceHandle())) {
                    handles.add(topic.getInstanceHandle());
                }
            }
        }
        for (InstanceHandle handle : remoteTopics.keySet()) {
            if (!ignoredTopics.contains(handle)) {
                handles.add(handle);
            }
        }
        return handles;
    }

    /**
     * DDS 1.4 §2.2.2.2.1.30: PRECONDITION_NOT_MET for a handle that is not a topic of this
     * domain or is one this participant ignores.
     */
    @Override
    public TopicBuiltinTopicData getDiscoveredTopicData(
            InstanceHandle topicHandle) {
        checkHandle(topicHandle);
        if (ignoredTopics.contains(topicHandle)) {
            throw new PreconditionNotMetException(
                    topicHandle + " is a Topic this participant ignores");
        }
        Ignores.Rules ignored = Ignores.of(getInstanceHandle());
        for (DomainParticipantImpl participant
                : DomainParticipantFactoryImpl.participantsInDomain(domainId)) {
            if (ignored.ignoresParticipant(participant.getInstanceHandle())
                    && participant != this) {
                continue;
            }
            for (Topic<?> topic : participant.containedTopics()) {
                if (topic.getInstanceHandle().equals(topicHandle)
                        && !BuiltinTopics.isBuiltin(topic.getName())) {
                    return BuiltinData.topic(getEnvironment(), topic);
                }
            }
        }
        TopicBuiltinTopicDataImpl remote = remoteTopics.get(topicHandle);
        if (remote != null) {
            return remote;
        }
        throw new PreconditionNotMetException(
                topicHandle + " is not a discovered Topic");
    }

    @Override
    public boolean containsEntity(InstanceHandle handle) {
        org.zerodds.core.Args.require(handle, "handle");
        for (Publisher publisher : publishers) {
            if (publisher.getInstanceHandle().equals(handle)) {
                return true;
            }
        }
        for (Subscriber subscriber : subscribers) {
            if (subscriber.getInstanceHandle().equals(handle)) {
                return true;
            }
            for (org.omg.dds.sub.DataReader<?> reader
                    : ((SubscriberImpl) subscriber).getDataReaders()) {
                if (reader.getInstanceHandle().equals(handle)) {
                    return true;
                }
            }
        }
        for (Publisher publisher : publishers) {
            for (org.omg.dds.pub.DataWriter<?> writer
                    : ((PublisherImpl) publisher).getDataWriters()) {
                if (writer.getInstanceHandle().equals(handle)) {
                    return true;
                }
            }
        }
        for (Topic<?> topic : topics) {
            if (topic.getInstanceHandle().equals(handle)) {
                return true;
            }
        }
        Subscriber builtinSubscriber = this.builtinSubscriber;
        if (builtinSubscriber == null) {
            return false;
        }
        if (builtinSubscriber.getInstanceHandle().equals(handle)) {
            return true;
        }
        for (org.omg.dds.sub.DataReader<?> reader
                : ((SubscriberImpl) builtinSubscriber).getDataReaders()) {
            if (reader.getInstanceHandle().equals(handle)) {
                return true;
            }
        }
        return false;
    }

    // --- Time (§7.4.2) -----------------------------------------------------

    @Override
    public ModifiableTime getCurrentTime(ModifiableTime currentTime) {
        long now = System.currentTimeMillis();
        if (currentTime == null) {
            currentTime = getEnvironment().getSPI()
                    .newTime(now, TimeUnit.MILLISECONDS);
        } else {
            currentTime.setTime(now, TimeUnit.MILLISECONDS);
        }
        return currentTime;
    }

    @Override
    public Time getCurrentTime() {
        return getEnvironment().getSPI()
                .newTime(System.currentTimeMillis(), TimeUnit.MILLISECONDS)
                .immutableCopy();
    }

    // --- Entity (§7.3.6) ---------------------------------------------------

    @Override
    public DomainParticipantQos getQos() {
        return qos;
    }

    /**
     * DDS 1.4 §2.2.2.1.1.1: the policies of a participant (USER_DATA, ENTITY_FACTORY) are
     * all changeable. The new USER_DATA reaches the other participants through
     * "DCPSParticipant" and SPDP.
     */
    @Override
    public void setQos(DomainParticipantQos qos) {
        org.zerodds.core.qos.QosRules.checkChange(enabled, this.qos, qos);
        this.qos = qos;
        if (!closed) {
            builtin.announceParticipant();
            if (org.zerodds.rtps.DdsWireBridge.enabled()) {
                org.zerodds.rtps.RtpsParticipant.get(domainId).setUserData(
                        qos.getUserData().getValue());
            }
        }
    }

    @Override
    protected DomainParticipantQos profileQos(QosProvider provider) {
        return provider.getDomainParticipantQos();
    }

    @SuppressWarnings("unchecked")
    @Override
    public StatusCondition<DomainParticipant> getStatusCondition() {
        return (StatusCondition<DomainParticipant>) super.getStatusCondition();
    }

    /**
     * DDS 1.4 §2.2.2.1.1.7: with ENTITY_FACTORY autoenable_created_entities the publishers,
     * subscribers and topics created so far are enabled too.
     */
    @Override
    public void enable() {
        if (closed) {
            throw new AlreadyClosedException("DomainParticipant is closed");
        }
        boolean first = !enabled;
        enabled = true;
        if (first && autoEnableCreated()) {
            for (Publisher publisher : new ArrayList<>(publishers)) {
                publisher.enable();
            }
            for (Subscriber subscriber : new ArrayList<>(subscribers)) {
                subscriber.enable();
            }
            for (Topic<?> topic : new ArrayList<>(topics)) {
                topic.enable();
            }
        }
    }

    public boolean isEnabled() {
        return enabled;
    }

    @Override
    public void close() {
        // §7.2.3: closing an Entity implicitly closes all contained objects.
        // The flag goes up first, so a concurrent create_* call cannot slip a new
        // contained entity in behind the close (DDS-Java-PSM 1.0 §7.2.4).
        closed = true;
        // §2.2.1.3.1: the entities of this participant leave the built-in
        // topics before its readers in other participants are closed.
        builtin.retractAll();
        closeContainedEntities();
        Ignores.forgetParticipant(getInstanceHandle());
        factory.forget(this);
        // DDS 1.2 §7.1.3.4: TRANSIENT data survives the DataWriter but not the
        // DomainParticipant; PERSISTENT data outlives it. The data of a domain goes with the
        // last participant of the domain in this process.
        if (DomainParticipantFactoryImpl.participantsInDomain(domainId).isEmpty()) {
            org.zerodds.internal.DurabilityService.instance().clearDomain(domainId);
        }
    }

    public boolean isClosed() {
        return closed;
    }

    private void ensureOpen() {
        if (closed) {
            throw new AlreadyClosedException("DomainParticipant is closed");
        }
    }

    @Override
    public String toString() {
        return "DomainParticipant[domain=" + domainId + "]";
    }
}
