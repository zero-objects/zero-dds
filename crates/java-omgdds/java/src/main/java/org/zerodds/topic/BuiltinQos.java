// SPDX-License-Identifier: Apache-2.0
package org.zerodds.topic;

import org.omg.dds.core.Duration;
import org.omg.dds.core.ServiceEnvironment;
import org.omg.dds.core.policy.DataRepresentation;
import org.omg.dds.core.policy.DestinationOrder;
import org.omg.dds.core.policy.Durability;
import org.omg.dds.core.policy.DurabilityService;
import org.omg.dds.core.policy.History;
import org.omg.dds.core.policy.Liveliness;
import org.omg.dds.core.policy.Ownership;
import org.omg.dds.core.policy.Presentation;
import org.omg.dds.core.policy.Reliability;
import org.omg.dds.core.policy.TypeConsistencyEnforcement;
import org.omg.dds.topic.ParticipantBuiltinTopicData;
import org.omg.dds.topic.PublicationBuiltinTopicData;
import org.omg.dds.topic.SubscriptionBuiltinTopicData;
import org.omg.dds.topic.TopicBuiltinTopicData;
import org.zerodds.core.policy.PolicyFactoryImpl;
import org.zerodds.core.qos.DataReaderQosImpl;
import org.zerodds.core.qos.DataWriterQosImpl;
import org.zerodds.core.qos.DomainParticipantQosImpl;
import org.zerodds.core.qos.PublisherQosImpl;
import org.zerodds.core.qos.SubscriberQosImpl;
import org.zerodds.core.qos.TopicQosImpl;
import org.zerodds.rtps.QosParameters;
import org.zerodds.rtps.QosParameters.Dur;

import java.util.ArrayList;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.concurrent.TimeUnit;

/**
 * The QoS policies of the built-in topic data (DDS 1.4 §2.2.5) between their
 * Java objects and {@link QosParameters}, the form DDSI-RTPS 2.5 §9.6.2
 * (Table 9.13) puts on the wire.
 *
 * <p>Reading a {@link QosParameters} fills a policy that has no parameter
 * with the default of the entity kind (DDS 1.4 §2.2.3, Table of defaults):
 * RELIABLE for the writer of a publication, BEST_EFFORT for the reader of a
 * subscription and for a topic. This is what §9.6.3.1 means by "a policy
 * absent from an announcement is that policy at its default".
 */
public final class BuiltinQos {

    private BuiltinQos() {}

    // --- Java -> wire --------------------------------------------------------

    public static QosParameters of(ParticipantBuiltinTopicData data) {
        QosParameters q = new QosParameters();
        q.userData = data.getUserData() == null ? null : data.getUserData().getValue();
        return q;
    }

    public static QosParameters of(TopicBuiltinTopicData data) {
        QosParameters q = new QosParameters();
        q.durability = durability(data.getDurability());
        q.durabilityService = durabilityService(data.getDurabilityService());
        q.deadline = data.getDeadline() == null ? null : dur(data.getDeadline().getPeriod());
        q.latencyBudget = data.getLatencyBudget() == null
                ? null : dur(data.getLatencyBudget().getDuration());
        q.liveliness = liveliness(data.getLiveliness());
        q.reliability = reliability(data.getReliability());
        q.transportPriority = data.getTransportPriority() == null
                ? null : data.getTransportPriority().getValue();
        q.lifespan = data.getLifespan() == null ? null : dur(data.getLifespan().getDuration());
        q.destinationOrder = data.getDestinationOrder() == null
                ? null : data.getDestinationOrder().getKind().ordinal();
        q.history = history(data.getHistory());
        q.resourceLimits = resourceLimits(data.getResourceLimits());
        q.ownership = ownership(data.getOwnership());
        q.topicData = data.getTopicData() == null ? null : data.getTopicData().getValue();
        q.dataRepresentation = representation(data.getRepresentation());
        q.typeConsistency = typeConsistency(data.getTypeConsistency());
        typeOf(data, q);
        return q;
    }

    public static QosParameters of(PublicationBuiltinTopicData data) {
        QosParameters q = new QosParameters();
        q.durability = durability(data.getDurability());
        q.durabilityService = durabilityService(data.getDurabilityService());
        q.deadline = data.getDeadline() == null ? null : dur(data.getDeadline().getPeriod());
        q.latencyBudget = data.getLatencyBudget() == null
                ? null : dur(data.getLatencyBudget().getDuration());
        q.liveliness = liveliness(data.getLiveliness());
        q.reliability = reliability(data.getReliability());
        q.lifespan = data.getLifespan() == null ? null : dur(data.getLifespan().getDuration());
        q.userData = data.getUserData() == null ? null : data.getUserData().getValue();
        q.ownership = ownership(data.getOwnership());
        q.ownershipStrength = data.getOwnershipStrength() == null
                ? null : data.getOwnershipStrength().getValue();
        q.destinationOrder = data.getDestinationOrder() == null
                ? null : data.getDestinationOrder().getKind().ordinal();
        q.presentation = presentation(data.getPresentation());
        q.partition = data.getPartition() == null
                ? null : new ArrayList<>(data.getPartition().getName());
        q.topicData = data.getTopicData() == null ? null : data.getTopicData().getValue();
        q.groupData = data.getGroupData() == null ? null : data.getGroupData().getValue();
        q.dataRepresentation = representation(data.getRepresentation());
        q.typeConsistency = typeConsistency(data.getTypeConsistency());
        typeOf(data, q);
        return q;
    }

    public static QosParameters of(SubscriptionBuiltinTopicData data) {
        QosParameters q = new QosParameters();
        q.durability = durability(data.getDurability());
        q.deadline = data.getDeadline() == null ? null : dur(data.getDeadline().getPeriod());
        q.latencyBudget = data.getLatencyBudget() == null
                ? null : dur(data.getLatencyBudget().getDuration());
        q.liveliness = liveliness(data.getLiveliness());
        q.reliability = reliability(data.getReliability());
        q.ownership = ownership(data.getOwnership());
        q.destinationOrder = data.getDestinationOrder() == null
                ? null : data.getDestinationOrder().getKind().ordinal();
        q.userData = data.getUserData() == null ? null : data.getUserData().getValue();
        q.timeBasedFilter = data.getTimeBasedFilter() == null
                ? null : dur(data.getTimeBasedFilter().getMinimumSeparation());
        q.presentation = presentation(data.getPresentation());
        q.partition = data.getPartition() == null
                ? null : new ArrayList<>(data.getPartition().getName());
        q.topicData = data.getTopicData() == null ? null : data.getTopicData().getValue();
        q.groupData = data.getGroupData() == null ? null : data.getGroupData().getValue();
        q.dataRepresentation = representation(data.getRepresentation());
        q.typeConsistency = typeConsistency(data.getTypeConsistency());
        typeOf(data, q);
        return q;
    }

    /** The type members of the built-in topic data (XTypes 1.3 §7.6.3.2.2). */
    private static void typeOf(Object data, QosParameters q) {
        java.util.List<String> equivalent;
        java.util.List<String> base;
        if (data instanceof TopicBuiltinTopicData) {
            equivalent = ((TopicBuiltinTopicData) data).getEquivalentTypeName();
            base = ((TopicBuiltinTopicData) data).getBaseTypeName();
        } else if (data instanceof PublicationBuiltinTopicData) {
            equivalent = ((PublicationBuiltinTopicData) data).getEquivalentTypeName();
            base = ((PublicationBuiltinTopicData) data).getBaseTypeName();
        } else {
            equivalent = ((SubscriptionBuiltinTopicData) data).getEquivalentTypeName();
            base = ((SubscriptionBuiltinTopicData) data).getBaseTypeName();
        }
        q.equivalentTypeName = equivalent == null ? null : new ArrayList<>(equivalent);
        q.baseTypeName = base == null ? null : new ArrayList<>(base);
        if (data instanceof TopicBuiltinTopicDataImpl) {
            q.typeInformation = ((TopicBuiltinTopicDataImpl) data).getTypeInformation();
            q.typeObject = ((TopicBuiltinTopicDataImpl) data).getTypeObjectBytes();
        } else if (data instanceof PublicationBuiltinTopicDataImpl) {
            q.typeInformation = ((PublicationBuiltinTopicDataImpl) data).getTypeInformation();
            q.typeObject = ((PublicationBuiltinTopicDataImpl) data).getTypeObjectBytes();
        } else if (data instanceof SubscriptionBuiltinTopicDataImpl) {
            q.typeInformation = ((SubscriptionBuiltinTopicDataImpl) data).getTypeInformation();
            q.typeObject = ((SubscriptionBuiltinTopicDataImpl) data).getTypeObjectBytes();
        }
    }

    private static Integer durability(Durability d) {
        return d == null ? null : d.getKind().ordinal();
    }

    private static QosParameters.DurabilityService durabilityService(DurabilityService d) {
        if (d == null) {
            return null;
        }
        QosParameters.DurabilityService s = new QosParameters.DurabilityService();
        s.cleanupDelay = dur(d.getServiceCleanupDelay());
        s.historyKind = d.getHistoryKind().ordinal();
        s.historyDepth = d.getHistoryDepth();
        s.maxSamples = d.getMaxSamples();
        s.maxInstances = d.getMaxInstances();
        s.maxSamplesPerInstance = d.getMaxSamplesPerInstance();
        return s;
    }

    private static QosParameters.Liveliness liveliness(Liveliness l) {
        if (l == null) {
            return null;
        }
        QosParameters.Liveliness out = new QosParameters.Liveliness();
        out.kind = l.getKind().ordinal();
        out.lease = dur(l.getLeaseDuration());
        return out;
    }

    /** RTPS numbers the reliability kinds BEST_EFFORT = 1, RELIABLE = 2. */
    private static QosParameters.Reliability reliability(Reliability r) {
        if (r == null) {
            return null;
        }
        QosParameters.Reliability out = new QosParameters.Reliability();
        out.kind = r.getKind() == Reliability.Kind.RELIABLE ? 2 : 1;
        out.maxBlockingTime = dur(r.getMaxBlockingTime());
        return out;
    }

    private static QosParameters.History history(History h) {
        if (h == null) {
            return null;
        }
        QosParameters.History out = new QosParameters.History();
        out.kind = h.getKind().ordinal();
        out.depth = h.getDepth();
        return out;
    }

    private static QosParameters.ResourceLimits resourceLimits(
            org.omg.dds.core.policy.ResourceLimits r) {
        if (r == null) {
            return null;
        }
        QosParameters.ResourceLimits out = new QosParameters.ResourceLimits();
        out.maxSamples = r.getMaxSamples();
        out.maxInstances = r.getMaxInstances();
        out.maxSamplesPerInstance = r.getMaxSamplesPerInstance();
        return out;
    }

    private static Integer ownership(Ownership o) {
        return o == null ? null : o.getKind().ordinal();
    }

    private static QosParameters.Presentation presentation(Presentation p) {
        if (p == null) {
            return null;
        }
        QosParameters.Presentation out = new QosParameters.Presentation();
        out.accessScope = p.getAccessScope().ordinal();
        out.coherentAccess = p.isCoherentAccess();
        out.orderedAccess = p.isOrderedAccess();
        return out;
    }

    private static int[] representation(DataRepresentation r) {
        if (r == null || r.getValue() == null) {
            return null;
        }
        List<Short> ids = r.getValue();
        int[] out = new int[ids.size()];
        for (int i = 0; i < out.length; i++) {
            out[i] = ids.get(i);
        }
        return out;
    }

    /**
     * XTypes 1.3 §7.6.3.4 has two kinds, DISALLOW_TYPE_COERCION (0) and
     * ALLOW_TYPE_COERCION (1); the PSM 1.0 enumeration has four. The exact
     * and declared kinds forbid coercion, the assignable kind allows it. The
     * five flags of XTypes 1.3 are not part of the PSM 1.0 API and are sent
     * at their specified defaults.
     */
    private static QosParameters.TypeConsistency typeConsistency(
            TypeConsistencyEnforcement t) {
        if (t == null) {
            return null;
        }
        QosParameters.TypeConsistency out = new QosParameters.TypeConsistency();
        // XTypes 1.3 7.6.2.3: ALLOW_TYPE_COERCION (1) for a reader that takes a writer of another
        // type (ASSIGNABLE, and DECLARED, which is the narrower test of the same permission),
        // DISALLOW_TYPE_COERCION (0) for the exact kinds
        out.kind = t.getKind() == TypeConsistencyEnforcement.Kind.ASSIGNABLE_TYPE_CONSISTENCY
                || t.getKind() == TypeConsistencyEnforcement.Kind.DECLARED_TYPE_CONSISTENCY
                ? 1 : 0;
        return out;
    }

    private static Dur dur(Duration d) {
        if (d == null || d.isInfinite()) {
            return d == null ? null : Dur.INFINITE;
        }
        return Dur.ofNanos(d.getDuration(TimeUnit.SECONDS),
                d.getRemainder(TimeUnit.SECONDS, TimeUnit.NANOSECONDS));
    }

    // --- wire -> Java --------------------------------------------------------

    public static void applyTo(ParticipantBuiltinTopicDataImpl data, QosParameters q,
            ServiceEnvironment env) {
        PolicyFactoryImpl pf = factory(env);
        byte[] user = q.userData != null ? q.userData
                : DomainParticipantQosImpl.defaults(env).getUserData().getValue();
        data.setUserData(pf.UserData().withValue(user, 0, user.length));
    }

    public static void applyTo(TopicBuiltinTopicDataImpl data, QosParameters q,
            ServiceEnvironment env) {
        data.setEquivalentTypeName(q.equivalentTypeName);
        data.setBaseTypeName(q.baseTypeName);
        data.setTypeInformation(q.typeInformation);
        data.setTypeObjectBytes(q.typeObject);
        PolicyFactoryImpl pf = factory(env);
        org.omg.dds.topic.TopicQos d = TopicQosImpl.defaults(env);
        data.setDurability(q.durability != null ? durability(pf, q.durability) : d.getDurability());
        data.setDurabilityService(q.durabilityService != null
                ? durabilityService(env, q.durabilityService) : d.getDurabilityService());
        data.setDeadline(q.deadline != null
                ? pf.Deadline().withPeriod(duration(env, q.deadline)) : d.getDeadline());
        data.setLatencyBudget(q.latencyBudget != null
                ? pf.LatencyBudget().withDuration(duration(env, q.latencyBudget))
                : d.getLatencyBudget());
        data.setLiveliness(q.liveliness != null
                ? liveliness(env, q.liveliness) : d.getLiveliness());
        data.setReliability(q.reliability != null
                ? reliability(env, q.reliability) : d.getReliability());
        data.setTransportPriority(q.transportPriority != null
                ? pf.TransportPriority().withValue(q.transportPriority)
                : d.getTransportPriority());
        data.setLifespan(q.lifespan != null
                ? pf.Lifespan().withDuration(duration(env, q.lifespan)) : d.getLifespan());
        data.setDestinationOrder(q.destinationOrder != null
                ? destinationOrder(pf, q.destinationOrder) : d.getDestinationOrder());
        data.setHistory(q.history != null ? history(pf, q.history) : d.getHistory());
        data.setResourceLimits(q.resourceLimits != null
                ? pf.ResourceLimits().withMaxSamples(q.resourceLimits.maxSamples)
                        .withMaxInstances(q.resourceLimits.maxInstances)
                        .withMaxSamplesPerInstance(q.resourceLimits.maxSamplesPerInstance)
                : d.getResourceLimits());
        data.setOwnership(q.ownership != null ? ownership(pf, q.ownership) : d.getOwnership());
        data.setTopicData(q.topicData != null
                ? pf.TopicData().withValue(q.topicData, 0, q.topicData.length)
                : d.getTopicData());
        data.setRepresentation(q.dataRepresentation != null
                ? representation(pf, q.dataRepresentation) : d.getRepresentation());
        data.setTypeConsistency(q.typeConsistency != null
                ? typeConsistency(pf, q.typeConsistency) : d.getTypeConsistency());
    }

    public static void applyTo(PublicationBuiltinTopicDataImpl data, QosParameters q,
            ServiceEnvironment env) {
        data.setEquivalentTypeName(q.equivalentTypeName);
        data.setBaseTypeName(q.baseTypeName);
        data.setTypeInformation(q.typeInformation);
        data.setTypeObjectBytes(q.typeObject);
        PolicyFactoryImpl pf = factory(env);
        org.omg.dds.pub.DataWriterQos d = DataWriterQosImpl.defaults(env);
        org.omg.dds.pub.PublisherQos p = PublisherQosImpl.defaults(env);
        data.setDurability(q.durability != null ? durability(pf, q.durability) : d.getDurability());
        data.setDurabilityService(q.durabilityService != null
                ? durabilityService(env, q.durabilityService) : d.getDurabilityService());
        data.setDeadline(q.deadline != null
                ? pf.Deadline().withPeriod(duration(env, q.deadline)) : d.getDeadline());
        data.setLatencyBudget(q.latencyBudget != null
                ? pf.LatencyBudget().withDuration(duration(env, q.latencyBudget))
                : d.getLatencyBudget());
        data.setLiveliness(q.liveliness != null
                ? liveliness(env, q.liveliness) : d.getLiveliness());
        data.setReliability(q.reliability != null
                ? reliability(env, q.reliability) : d.getReliability());
        data.setLifespan(q.lifespan != null
                ? pf.Lifespan().withDuration(duration(env, q.lifespan)) : d.getLifespan());
        data.setUserData(q.userData != null
                ? pf.UserData().withValue(q.userData, 0, q.userData.length) : d.getUserData());
        data.setOwnership(q.ownership != null ? ownership(pf, q.ownership) : d.getOwnership());
        data.setOwnershipStrength(q.ownershipStrength != null
                ? pf.OwnershipStrength().withValue(q.ownershipStrength)
                : d.getOwnershipStrength());
        data.setDestinationOrder(q.destinationOrder != null
                ? destinationOrder(pf, q.destinationOrder) : d.getDestinationOrder());
        data.setPresentation(q.presentation != null
                ? presentation(pf, q.presentation) : p.getPresentation());
        data.setPartition(q.partition != null && !q.partition.isEmpty()
                ? pf.Partition().withName(new LinkedHashSet<>(q.partition)) : p.getPartition());
        data.setTopicData(q.topicData != null
                ? pf.TopicData().withValue(q.topicData, 0, q.topicData.length)
                : pf.TopicData());
        data.setGroupData(q.groupData != null
                ? pf.GroupData().withValue(q.groupData, 0, q.groupData.length)
                : p.getGroupData());
        data.setRepresentation(q.dataRepresentation != null
                ? representation(pf, q.dataRepresentation) : d.getRepresentation());
        data.setTypeConsistency(q.typeConsistency != null
                ? typeConsistency(pf, q.typeConsistency) : d.getTypeConsistency());
    }

    public static void applyTo(SubscriptionBuiltinTopicDataImpl data, QosParameters q,
            ServiceEnvironment env) {
        data.setEquivalentTypeName(q.equivalentTypeName);
        data.setBaseTypeName(q.baseTypeName);
        data.setTypeInformation(q.typeInformation);
        data.setTypeObjectBytes(q.typeObject);
        PolicyFactoryImpl pf = factory(env);
        org.omg.dds.sub.DataReaderQos d = DataReaderQosImpl.defaults(env);
        org.omg.dds.sub.SubscriberQos s = SubscriberQosImpl.defaults(env);
        data.setDurability(q.durability != null ? durability(pf, q.durability) : d.getDurability());
        data.setDeadline(q.deadline != null
                ? pf.Deadline().withPeriod(duration(env, q.deadline)) : d.getDeadline());
        data.setLatencyBudget(q.latencyBudget != null
                ? pf.LatencyBudget().withDuration(duration(env, q.latencyBudget))
                : d.getLatencyBudget());
        data.setLiveliness(q.liveliness != null
                ? liveliness(env, q.liveliness) : d.getLiveliness());
        data.setReliability(q.reliability != null
                ? reliability(env, q.reliability) : d.get(Reliability.class));
        data.setOwnership(q.ownership != null ? ownership(pf, q.ownership) : d.getOwnership());
        data.setDestinationOrder(q.destinationOrder != null
                ? destinationOrder(pf, q.destinationOrder) : d.getDestinationOrder());
        data.setUserData(q.userData != null
                ? pf.UserData().withValue(q.userData, 0, q.userData.length) : d.getUserData());
        data.setTimeBasedFilter(q.timeBasedFilter != null
                ? pf.TimeBasedFilter().withMinimumSeparation(duration(env, q.timeBasedFilter))
                : d.getTimeBasedFilter());
        data.setPresentation(q.presentation != null
                ? presentation(pf, q.presentation) : s.getPresentation());
        data.setPartition(q.partition != null && !q.partition.isEmpty()
                ? pf.Partition().withName(new LinkedHashSet<>(q.partition)) : s.getPartition());
        data.setTopicData(q.topicData != null
                ? pf.TopicData().withValue(q.topicData, 0, q.topicData.length)
                : pf.TopicData());
        data.setGroupData(q.groupData != null
                ? pf.GroupData().withValue(q.groupData, 0, q.groupData.length)
                : s.getGroupData());
        data.setRepresentation(q.dataRepresentation != null
                ? representation(pf, q.dataRepresentation) : d.getRepresentation());
        data.setTypeConsistency(q.typeConsistency != null
                ? typeConsistency(pf, q.typeConsistency) : d.getTypeConsistency());
    }

    private static PolicyFactoryImpl factory(ServiceEnvironment env) {
        return (PolicyFactoryImpl) env.getSPI().getPolicyFactory();
    }

    private static <E extends Enum<E>> E kind(E[] values, int wire, String what) {
        if (wire < 0 || wire >= values.length) {
            throw new IllegalArgumentException(wire + " is not a " + what);
        }
        return values[wire];
    }

    private static Durability durability(PolicyFactoryImpl pf, int wire) {
        return pf.Durability().withKind(kind(Durability.Kind.values(), wire, "durability kind"));
    }

    private static DurabilityService durabilityService(ServiceEnvironment env,
            QosParameters.DurabilityService s) {
        return factory(env).DurabilityService()
                .withServiceCleanupDelay(duration(env, s.cleanupDelay))
                .withHistoryKind(kind(History.Kind.values(), s.historyKind, "history kind"))
                .withHistoryDepth(s.historyDepth)
                .withMaxSamples(s.maxSamples)
                .withMaxInstances(s.maxInstances)
                .withMaxSamplesPerInstance(s.maxSamplesPerInstance);
    }

    private static Liveliness liveliness(ServiceEnvironment env, QosParameters.Liveliness l) {
        return factory(env).Liveliness()
                .withKind(kind(Liveliness.Kind.values(), l.kind, "liveliness kind"))
                .withLeaseDuration(duration(env, l.lease));
    }

    private static Reliability reliability(ServiceEnvironment env, QosParameters.Reliability r) {
        if (r.kind != 1 && r.kind != 2) {
            throw new IllegalArgumentException(r.kind + " is not a reliability kind");
        }
        return factory(env).Reliability()
                .withKind(r.kind == 2 ? Reliability.Kind.RELIABLE : Reliability.Kind.BEST_EFFORT)
                .withMaxBlockingTime(duration(env, r.maxBlockingTime));
    }

    private static DestinationOrder destinationOrder(PolicyFactoryImpl pf, int wire) {
        return pf.DestinationOrder().withKind(
                kind(DestinationOrder.Kind.values(), wire, "destination order kind"));
    }

    private static History history(PolicyFactoryImpl pf, QosParameters.History h) {
        return pf.History().withKind(kind(History.Kind.values(), h.kind, "history kind"))
                .withDepth(h.depth);
    }

    private static Ownership ownership(PolicyFactoryImpl pf, int wire) {
        return pf.Ownership().withKind(kind(Ownership.Kind.values(), wire, "ownership kind"));
    }

    private static Presentation presentation(PolicyFactoryImpl pf, QosParameters.Presentation p) {
        return pf.Presentation().withAccessScope(
                kind(Presentation.AccessScopeKind.values(), p.accessScope, "access scope"))
                .withCoherentAccess(p.coherentAccess)
                .withOrderedAccess(p.orderedAccess);
    }

    private static DataRepresentation representation(PolicyFactoryImpl pf, int[] ids) {
        short[] shorts = new short[ids.length];
        for (int i = 0; i < ids.length; i++) {
            shorts[i] = (short) ids[i];
        }
        return pf.Representation().withValue(shorts);
    }

    private static TypeConsistencyEnforcement typeConsistency(PolicyFactoryImpl pf,
            QosParameters.TypeConsistency t) {
        return pf.TypeConsistency().withKind(t.kind == 0
                ? TypeConsistencyEnforcement.Kind.EXACT_TYPE_TYPE_CONSISTENCY
                : TypeConsistencyEnforcement.Kind.ASSIGNABLE_TYPE_CONSISTENCY);
    }

    private static Duration duration(ServiceEnvironment env, Dur d) {
        if (d.isInfinite()) {
            return env.getSPI().infiniteDuration();
        }
        return env.getSPI().newDuration(
                d.seconds * 1_000_000_000L + d.nanos(), TimeUnit.NANOSECONDS);
    }
}
