// SPDX-License-Identifier: Apache-2.0
package org.zerodds.topic;

import org.omg.dds.core.QosProvider;
import org.omg.dds.core.ServiceEnvironment;
import org.omg.dds.core.StatusCondition;
import org.omg.dds.core.status.InconsistentTopicStatus;
import org.omg.dds.core.status.Status;
import org.omg.dds.domain.DomainParticipant;
import org.omg.dds.topic.Topic;
import org.omg.dds.topic.TopicDescription;
import org.omg.dds.topic.TopicListener;
import org.omg.dds.topic.TopicQos;
import org.omg.dds.type.TypeSupport;
import org.zerodds.cdr.TypeSupportResolver;
import org.zerodds.core.AlreadyClosedException;
import org.zerodds.core.EntityBase;
import org.zerodds.core.Values;
import org.zerodds.core.status.InconsistentTopicStatusImpl;

/**
 * ZeroDDS {@link Topic} (DDS-Java-PSM 1.0 §7.5.1).
 *
 * <p>§7.5.1 gives {@code createTopic} a type or a {@link TypeSupport}; the
 * topic keeps the resulting {@code TypeSupport}, so its {@code typeName} is
 * the name the type is registered under and the readers and writers created
 * from it share one codec (§7.2.2 — a value type of another implementation is
 * accepted as a parameter).
 */
public final class TopicImpl<TYPE> extends EntityBase<TopicListener<TYPE>, TopicQos>
implements Topic<TYPE> {

    private static final Class<? extends Status> INCONSISTENT_TOPIC =
            InconsistentTopicStatus.class;

    private final String name;
    private final TypeSupport<TYPE> typeSupport;
    private final DomainParticipant parent;
    private volatile TopicQos qos;

    // INCONSISTENT_TOPIC tracking (DDS 1.2 §7.1.4.1).
    private int inconsistentTotal = 0;
    private int inconsistentReadBaseline = 0;

    private volatile boolean closed = false;
    private volatile boolean enabled = false;

    public TopicImpl(ServiceEnvironment environment, DomainParticipant parent,
            String name, Class<TYPE> type, TopicQos qos) {
        super(environment);
        org.zerodds.core.qos.QosRules.checkConsistent(qos);
        this.parent = parent;
        this.name = name;
        this.qos = qos;
        this.typeSupport = typeSupportFor(environment, type);
    }

    /** The topic of an explicit {@link TypeSupport} (§7.5.1). */
    public TopicImpl(ServiceEnvironment environment, DomainParticipant parent,
            String name, TypeSupport<TYPE> typeSupport, TopicQos qos) {
        super(environment);
        org.zerodds.core.qos.QosRules.checkConsistent(qos);
        this.parent = parent;
        this.name = name;
        this.qos = qos;
        this.typeSupport = typeSupport;
    }

    @SuppressWarnings("unchecked")
    private static <TYPE> TypeSupport<TYPE> typeSupportFor(
            ServiceEnvironment environment, Class<TYPE> type) {
        org.zerodds.cdr.TopicTypeSupport<TYPE> codec = TypeSupportResolver.resolve(type);
        // through the SPI (§7.3.1), which also knows how to create samples of the abstract built-in types
        return environment.getSPI().newTypeSupport(type, codec.getTypeName());
    }

    // --- TopicDescription (§7.5.1) -----------------------------------------

    @Override
    public TypeSupport<TYPE> getTypeSupport() {
        return typeSupport;
    }

    @SuppressWarnings("unchecked")
    @Override
    public <OTHER> TopicDescription<OTHER> cast() {
        return (TopicDescription<OTHER>) this;
    }

    @Override
    public String getTypeName() {
        return typeSupport.getTypeName();
    }

    @Override
    public String getName() {
        return name;
    }

    @Override
    public DomainParticipant getParent() {
        return parent;
    }

    // --- Topic --------------------------------------------------------------

    @Override
    public InconsistentTopicStatus getInconsistentTopicStatus() {
        clearStatus(INCONSISTENT_TOPIC);
        synchronized (this) {
            int change = inconsistentTotal - inconsistentReadBaseline;
            inconsistentReadBaseline = inconsistentTotal;
            return new InconsistentTopicStatusImpl(env(), inconsistentTotal, change);
        }
    }

    /**
     * DDS 1.4 §2.2.4.1 INCONSISTENT_TOPIC: a topic of the same name with another type exists
     * in the domain. The status changes and the listener is told, outside this topic's monitor:
     * the topic's own TopicListener, else the DomainParticipantListener (§2.2.4.3.1).
     */
    public void onInconsistentTopic() {
        TopicListener<TYPE> listener;
        InconsistentTopicStatus status = null;
        synchronized (this) {
            inconsistentTotal++;
            triggerStatus(INCONSISTENT_TOPIC);
            listener = listenerFor();
            if (listener != null) {
                int change = inconsistentTotal - inconsistentReadBaseline;
                inconsistentReadBaseline = inconsistentTotal;
                status = new InconsistentTopicStatusImpl(env(), inconsistentTotal, change);
            }
        }
        if (listener != null) {
            listener.onInconsistentTopic(
                    new org.zerodds.core.event.InconsistentTopicEventImpl<>(env(), this, status));
        }
    }

    private TopicListener<TYPE> listenerFor() {
        TopicListener<TYPE> own = ownListener(INCONSISTENT_TOPIC);
        if (own != null) {
            return own;
        }
        if (parent instanceof org.zerodds.domain.DomainParticipantImpl) {
            org.omg.dds.domain.DomainParticipantListener up =
                    ((org.zerodds.domain.DomainParticipantImpl) parent).ownListener(INCONSISTENT_TOPIC);
            if (up != null) {
                return org.zerodds.core.ListenerUp.forTopic(up);
            }
        }
        return null;
    }

    @SuppressWarnings("unchecked")
    @Override
    public StatusCondition<Topic<TYPE>> getStatusCondition() {
        return (StatusCondition<Topic<TYPE>>) super.getStatusCondition();
    }

    // --- Entity (§7.3.6) ----------------------------------------------------

    @Override
    public TopicQos getQos() {
        return qos;
    }

    /**
     * DDS 1.4 §2.2.2.1.1.1: an enabled topic refuses a change of an immutable policy
     * (IMMUTABLE_POLICY) and an inconsistent set (INCONSISTENT_POLICY); TOPIC_DATA, DEADLINE,
     * LATENCY_BUDGET, TRANSPORT_PRIORITY and LIFESPAN can change, and "DCPSTopic" tells the
     * domain.
     */
    @Override
    public void setQos(TopicQos qos) {
        synchronized (this) {
            org.zerodds.core.qos.QosRules.checkChange(enabled, this.qos, qos);
            this.qos = qos;
        }
        // "DCPSTopic" carries the QoS of the topic (DDS 1.4 2.2.5): the change is a new sample
        if (parent instanceof org.zerodds.domain.DomainParticipantImpl) {
            ((org.zerodds.domain.DomainParticipantImpl) parent).topicQosChanged(this);
        }
    }

    @Override
    protected TopicQos profileQos(QosProvider provider) {
        return provider.getTopicQos();
    }

    @Override
    public void enable() {
        if (closed) {
            throw new AlreadyClosedException("Topic is closed");
        }
        enabled = true;
    }

    public boolean isEnabled() {
        return enabled;
    }

    @Override
    public void close() {
        if (closed) {
            return;
        }
        // DDS 1.4 §2.2.2.3.2 delete_topic: PRECONDITION_NOT_MET while a DataWriter or DataReader
        // still refers to the Topic.
        if (users.get() > 0) {
            throw new org.zerodds.core.PreconditionNotMetException("the Topic " + name
                    + " is still used by " + users.get() + " DataWriter or DataReader");
        }
        closed = true;
        runCloseHooks();
    }

    private final java.util.concurrent.atomic.AtomicInteger users =
            new java.util.concurrent.atomic.AtomicInteger();

    /** A DataWriter or DataReader refers to this Topic. */
    public void use() {
        users.incrementAndGet();
    }

    /** That DataWriter or DataReader was closed. */
    public void release() {
        users.decrementAndGet();
    }

    public boolean isClosed() {
        return closed;
    }

    /**
     * Whether {@code topic} is a ZeroDDS topic that has been closed. A topic
     * of another implementation is not one this binding can answer for.
     */
    public static boolean isClosed(Topic<?> topic) {
        return topic instanceof TopicImpl && ((TopicImpl<?>) topic).isClosed();
    }

    @Override
    public String toString() {
        return "Topic[" + name + "," + getTypeName() + "]";
    }
}