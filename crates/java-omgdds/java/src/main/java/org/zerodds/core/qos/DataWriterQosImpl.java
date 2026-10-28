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
import org.omg.dds.core.policy.OwnershipStrength;
import org.omg.dds.core.policy.Reliability;
import org.omg.dds.core.policy.ResourceLimits;
import org.omg.dds.core.policy.TransportPriority;
import org.omg.dds.core.policy.TypeConsistencyEnforcement;
import org.omg.dds.core.policy.UserData;
import org.omg.dds.core.policy.WriterDataLifecycle;
import org.omg.dds.pub.DataWriterQos;
import org.zerodds.core.policy.PolicyFactoryImpl;

import java.util.LinkedHashMap;
import java.util.Map;

/**
 * ZeroDDS DataWriterQos (DDS-Java-PSM 1.0 §7.3.5.2).
 *
 * <p>The QoS of a DataWriter (DDS-Java-PSM 1.0 §7.6.2). RELIABILITY defaults to RELIABLE for a writer (DDS 1.2 §7.1.3.6).
 */
public final class DataWriterQosImpl
extends AbstractEntityQos<QosPolicy.ForDataWriter, DataWriterQos>
implements DataWriterQos {

    private static final long serialVersionUID = 1L;

    private DataWriterQosImpl(ServiceEnvironment environment,
                        Map<Class<? extends QosPolicy.ForDataWriter>, QosPolicy.ForDataWriter> policies) {
        super(environment, policies);
    }

    /** The QoS of DDS 1.2 §7.1.3: every policy at its default value. */
    public static DataWriterQos defaults(ServiceEnvironment environment) {
        PolicyFactoryImpl factory =
                (PolicyFactoryImpl) environment.getSPI().getPolicyFactory();
        Map<Class<? extends QosPolicy.ForDataWriter>, QosPolicy.ForDataWriter> policies =
                new LinkedHashMap<>();
        policies.put(Durability.class, factory.Durability());
        policies.put(DurabilityService.class, factory.DurabilityService());
        policies.put(Deadline.class, factory.Deadline());
        policies.put(LatencyBudget.class, factory.LatencyBudget());
        policies.put(Liveliness.class, factory.Liveliness());
        policies.put(Reliability.class, factory.DataWriterReliability());
        policies.put(DestinationOrder.class, factory.DestinationOrder());
        policies.put(History.class, factory.DataReaderHistory());
        policies.put(ResourceLimits.class, factory.ResourceLimits());
        policies.put(TransportPriority.class, factory.TransportPriority());
        policies.put(Lifespan.class, factory.Lifespan());
        policies.put(UserData.class, factory.UserData());
        policies.put(Ownership.class, factory.Ownership());
        policies.put(OwnershipStrength.class, factory.OwnershipStrength());
        policies.put(WriterDataLifecycle.class, factory.WriterDataLifecycle());
        policies.put(DataRepresentation.class, factory.Representation());
        policies.put(TypeConsistencyEnforcement.class, factory.TypeConsistency());
        return new DataWriterQosImpl(environment, policies);
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
    public UserData getUserData() {
        return get(UserData.class);
    }

    @Override
    public Ownership getOwnership() {
        return get(Ownership.class);
    }

    @Override
    public OwnershipStrength getOwnershipStrength() {
        return get(OwnershipStrength.class);
    }

    @Override
    public WriterDataLifecycle getWriterDataLifecycle() {
        return get(WriterDataLifecycle.class);
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
    protected DataWriterQos with(QosPolicy.ForDataWriter policy) {
        Map<Class<? extends QosPolicy.ForDataWriter>, QosPolicy.ForDataWriter> policies = new LinkedHashMap<>();
        policies.putAll(this);
        policies.put(policyKey(policy), policy);
        return new DataWriterQosImpl(getEnvironment(), policies);
    }

    @Override
    protected DataWriterQos self() {
        return this;
    }

    @Override
    protected Class<? extends QosPolicy.ForDataWriter> policyClass() {
        return QosPolicy.ForDataWriter.class;
    }
}
