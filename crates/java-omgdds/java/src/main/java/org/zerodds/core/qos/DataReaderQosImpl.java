// SPDX-License-Identifier: Apache-2.0
package org.zerodds.core.qos;

import org.omg.dds.core.ServiceEnvironment;
import org.omg.dds.core.policy.QosPolicy;
import org.omg.dds.core.policy.DataRepresentation;
import org.omg.dds.core.policy.Deadline;
import org.omg.dds.core.policy.DestinationOrder;
import org.omg.dds.core.policy.Durability;
import org.omg.dds.core.policy.History;
import org.omg.dds.core.policy.LatencyBudget;
import org.omg.dds.core.policy.Liveliness;
import org.omg.dds.core.policy.Ownership;
import org.omg.dds.core.policy.ReaderDataLifecycle;
import org.omg.dds.core.policy.Reliability;
import org.omg.dds.core.policy.ResourceLimits;
import org.omg.dds.core.policy.TimeBasedFilter;
import org.omg.dds.core.policy.TypeConsistencyEnforcement;
import org.omg.dds.core.policy.UserData;
import org.omg.dds.sub.DataReaderQos;
import org.zerodds.core.policy.PolicyFactoryImpl;

import java.util.LinkedHashMap;
import java.util.Map;

/**
 * ZeroDDS DataReaderQos (DDS-Java-PSM 1.0 §7.3.5.2).
 *
 * <p>The QoS of a DataReader (DDS-Java-PSM 1.0 §7.7.3). RELIABILITY is not a reader policy; a reader inherits it from its Topic.
 */
public final class DataReaderQosImpl
extends AbstractEntityQos<QosPolicy.ForDataReader, DataReaderQos>
implements DataReaderQos {

    private static final long serialVersionUID = 1L;

    private DataReaderQosImpl(ServiceEnvironment environment,
                        Map<Class<? extends QosPolicy.ForDataReader>, QosPolicy.ForDataReader> policies) {
        super(environment, policies);
    }

    /** The QoS of DDS 1.2 §7.1.3: every policy at its default value. */
    public static DataReaderQos defaults(ServiceEnvironment environment) {
        PolicyFactoryImpl factory =
                (PolicyFactoryImpl) environment.getSPI().getPolicyFactory();
        Map<Class<? extends QosPolicy.ForDataReader>, QosPolicy.ForDataReader> policies =
                new LinkedHashMap<>();
        policies.put(Durability.class, factory.Durability());
        policies.put(Deadline.class, factory.Deadline());
        policies.put(LatencyBudget.class, factory.LatencyBudget());
        policies.put(Liveliness.class, factory.Liveliness());
        // §2.2.3.14: a DataReader requests BEST_EFFORT unless it says otherwise.
        policies.put(Reliability.class, factory.DataReaderReliability());
        policies.put(DestinationOrder.class, factory.DestinationOrder());
        policies.put(History.class, factory.DataReaderHistory());
        policies.put(ResourceLimits.class, factory.ResourceLimits());
        policies.put(UserData.class, factory.UserData());
        policies.put(Ownership.class, factory.Ownership());
        policies.put(TimeBasedFilter.class, factory.TimeBasedFilter());
        policies.put(ReaderDataLifecycle.class, factory.ReaderDataLifecycle());
        policies.put(DataRepresentation.class, factory.Representation());
        policies.put(TypeConsistencyEnforcement.class, factory.TypeConsistency());
        return new DataReaderQosImpl(environment, policies);
    }

    @Override
    public Durability getDurability() {
        return get(Durability.class);
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
    public UserData getUserData() {
        return get(UserData.class);
    }

    @Override
    public Ownership getOwnership() {
        return get(Ownership.class);
    }

    @Override
    public TimeBasedFilter getTimeBasedFilter() {
        return get(TimeBasedFilter.class);
    }

    @Override
    public ReaderDataLifecycle getReaderDataLifecycle() {
        return get(ReaderDataLifecycle.class);
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
    protected DataReaderQos with(QosPolicy.ForDataReader policy) {
        Map<Class<? extends QosPolicy.ForDataReader>, QosPolicy.ForDataReader> policies = new LinkedHashMap<>();
        policies.putAll(this);
        policies.put(policyKey(policy), policy);
        return new DataReaderQosImpl(getEnvironment(), policies);
    }

    @Override
    protected DataReaderQos self() {
        return this;
    }

    @Override
    protected Class<? extends QosPolicy.ForDataReader> policyClass() {
        return QosPolicy.ForDataReader.class;
    }
}
