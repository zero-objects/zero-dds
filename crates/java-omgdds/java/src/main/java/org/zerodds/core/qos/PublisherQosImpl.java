// SPDX-License-Identifier: Apache-2.0
package org.zerodds.core.qos;

import org.omg.dds.core.ServiceEnvironment;
import org.omg.dds.core.policy.QosPolicy;
import org.omg.dds.core.policy.EntityFactory;
import org.omg.dds.core.policy.GroupData;
import org.omg.dds.core.policy.Partition;
import org.omg.dds.core.policy.Presentation;
import org.omg.dds.pub.PublisherQos;
import org.zerodds.core.policy.PolicyFactoryImpl;

import java.util.LinkedHashMap;
import java.util.Map;

/**
 * ZeroDDS PublisherQos (DDS-Java-PSM 1.0 §7.3.5.2).
 *
 * <p>The QoS of a Publisher (DDS-Java-PSM 1.0 §7.6.1): PRESENTATION, PARTITION, GROUP_DATA and ENTITY_FACTORY apply.
 */
public final class PublisherQosImpl
extends AbstractEntityQos<QosPolicy.ForPublisher, PublisherQos>
implements PublisherQos {

    private static final long serialVersionUID = 1L;

    private PublisherQosImpl(ServiceEnvironment environment,
                        Map<Class<? extends QosPolicy.ForPublisher>, QosPolicy.ForPublisher> policies) {
        super(environment, policies);
    }

    /** The QoS of DDS 1.2 §7.1.3: every policy at its default value. */
    public static PublisherQos defaults(ServiceEnvironment environment) {
        PolicyFactoryImpl factory =
                (PolicyFactoryImpl) environment.getSPI().getPolicyFactory();
        Map<Class<? extends QosPolicy.ForPublisher>, QosPolicy.ForPublisher> policies =
                new LinkedHashMap<>();
        policies.put(Presentation.class, factory.Presentation());
        policies.put(Partition.class, factory.Partition());
        policies.put(GroupData.class, factory.GroupData());
        policies.put(EntityFactory.class, factory.EntityFactory());
        return new PublisherQosImpl(environment, policies);
    }

    @Override
    public Presentation getPresentation() {
        return get(Presentation.class);
    }

    @Override
    public Partition getPartition() {
        return get(Partition.class);
    }

    @Override
    public GroupData getGroupData() {
        return get(GroupData.class);
    }

    @Override
    public EntityFactory getEntityFactory() {
        return get(EntityFactory.class);
    }

    @Override
    protected PublisherQos with(QosPolicy.ForPublisher policy) {
        Map<Class<? extends QosPolicy.ForPublisher>, QosPolicy.ForPublisher> policies = new LinkedHashMap<>();
        policies.putAll(this);
        policies.put(policyKey(policy), policy);
        return new PublisherQosImpl(getEnvironment(), policies);
    }

    @Override
    protected PublisherQos self() {
        return this;
    }

    @Override
    protected Class<? extends QosPolicy.ForPublisher> policyClass() {
        return QosPolicy.ForPublisher.class;
    }
}
