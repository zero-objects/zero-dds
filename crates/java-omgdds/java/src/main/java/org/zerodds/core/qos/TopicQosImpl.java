// SPDX-License-Identifier: Apache-2.0
package org.zerodds.core.qos;

import org.omg.dds.core.ServiceEnvironment;
import org.omg.dds.core.policy.QosPolicy;
import org.omg.dds.core.policy.DataRepresentation;
import org.omg.dds.core.policy.Deadline;
import org.omg.dds.core.policy.DestinationOrder;
import org.omg.dds.core.policy.Durability;
import org.omg.dds.core.policy.DurabilityService;
import org.omg.dds.core.policy.History;
import org.omg.dds.core.policy.LatencyBudget;
import org.omg.dds.core.policy.Lifespan;
import org.omg.dds.core.policy.Liveliness;
import org.omg.dds.core.policy.Ownership;
import org.omg.dds.core.policy.Reliability;
import org.omg.dds.core.policy.ResourceLimits;
import org.omg.dds.core.policy.TopicData;
import org.omg.dds.core.policy.TransportPriority;
import org.omg.dds.core.policy.TypeConsistencyEnforcement;
import org.omg.dds.topic.TopicQos;
import org.zerodds.core.policy.PolicyFactoryImpl;

import java.util.LinkedHashMap;
import java.util.Map;

/**
 * ZeroDDS TopicQos (DDS-Java-PSM 1.0 §7.3.5.2).
 *
 * <p>The QoS of a Topic (DDS-Java-PSM 1.0 §7.5.2). RELIABILITY defaults to BEST_EFFORT for a topic (DDS 1.2 §7.1.3.6).
 */
public final class TopicQosImpl
extends AbstractEntityQos<QosPolicy.ForTopic, TopicQos>
implements TopicQos {

    private static final long serialVersionUID = 1L;

    private TopicQosImpl(ServiceEnvironment environment,
                        Map<Class<? extends QosPolicy.ForTopic>, QosPolicy.ForTopic> policies) {
        super(environment, policies);
    }

    /** The QoS of DDS 1.2 §7.1.3: every policy at its default value. */
    public static TopicQos defaults(ServiceEnvironment environment) {
        PolicyFactoryImpl factory =
                (PolicyFactoryImpl) environment.getSPI().getPolicyFactory();
        // TOPIC_DATA is not among the PolicyFactory operations of Table 7.2.
        Map<Class<? extends QosPolicy.ForTopic>, QosPolicy.ForTopic> policies =
                new LinkedHashMap<>();
        policies.put(TopicData.class, factory.TopicData());
        policies.put(Durability.class, factory.Durability());
        policies.put(DurabilityService.class, factory.DurabilityService());
        policies.put(Deadline.class, factory.Deadline());
        policies.put(LatencyBudget.class, factory.LatencyBudget());
        policies.put(Liveliness.class, factory.Liveliness());
        policies.put(Reliability.class, factory.DataReaderReliability());
        policies.put(DestinationOrder.class, factory.DestinationOrder());
        policies.put(History.class, factory.DataReaderHistory());
        policies.put(ResourceLimits.class, factory.ResourceLimits());
        policies.put(TransportPriority.class, factory.TransportPriority());
        policies.put(Lifespan.class, factory.Lifespan());
        policies.put(Ownership.class, factory.Ownership());
        policies.put(DataRepresentation.class, factory.Representation());
        policies.put(TypeConsistencyEnforcement.class, factory.TypeConsistency());
        return new TopicQosImpl(environment, policies);
    }

    @Override
    public TopicData getTopicData() {
        return get(TopicData.class);
    }

    @Override
    public Durability getDurability() {
        return get(Durability.class);
    }

    @Override
    public DurabilityService getDurabilityService() {
        return get(DurabilityService.class);
    }

    @Override
    public Deadline getDeadline() {
        return get(Deadline.class);
    }

    @Override
    public LatencyBudget getLatencyBudget() {
        return get(LatencyBudget.class);
    }

    @Override
    public Liveliness getLiveliness() {
        return get(Liveliness.class);
    }

    @Override
    public Reliability getReliability() {
        return get(Reliability.class);
    }

    @Override
    public DestinationOrder getDestinationOrder() {
        return get(DestinationOrder.class);
    }

    @Override
    public History getHistory() {
        return get(History.class);
    }

    @Override
    public ResourceLimits getResourceLimits() {
        return get(ResourceLimits.class);
    }

    @Override
    public TransportPriority getTransportPriority() {
        return get(TransportPriority.class);
    }

    @Override
    public Lifespan getLifespan() {
        return get(Lifespan.class);
    }

    @Override
    public Ownership getOwnership() {
        return get(Ownership.class);
    }

    @Override
    public DataRepresentation getRepresentation() {
        return get(DataRepresentation.class);
    }

    @Override
    public TypeConsistencyEnforcement getTypeConsistency() {
        return get(TypeConsistencyEnforcement.class);
    }

    @Override
    protected TopicQos with(QosPolicy.ForTopic policy) {
        Map<Class<? extends QosPolicy.ForTopic>, QosPolicy.ForTopic> policies = new LinkedHashMap<>();
        policies.putAll(this);
        policies.put(policyKey(policy), policy);
        return new TopicQosImpl(getEnvironment(), policies);
    }

    @Override
    protected TopicQos self() {
        return this;
    }

    @Override
    protected Class<? extends QosPolicy.ForTopic> policyClass() {
        return QosPolicy.ForTopic.class;
    }
}
