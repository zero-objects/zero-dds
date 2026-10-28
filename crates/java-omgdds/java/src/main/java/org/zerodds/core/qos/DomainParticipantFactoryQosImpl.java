// SPDX-License-Identifier: Apache-2.0
package org.zerodds.core.qos;

import org.omg.dds.core.ServiceEnvironment;
import org.omg.dds.core.policy.QosPolicy;
import org.omg.dds.core.policy.EntityFactory;
import org.omg.dds.domain.DomainParticipantFactoryQos;
import org.zerodds.core.policy.PolicyFactoryImpl;

import java.util.LinkedHashMap;
import java.util.Map;

/**
 * ZeroDDS DomainParticipantFactoryQos (DDS-Java-PSM 1.0 §7.3.5.2).
 *
 * <p>The QoS of a DomainParticipantFactory (DDS-Java-PSM 1.0 §7.4.1): only ENTITY_FACTORY applies.
 */
public final class DomainParticipantFactoryQosImpl
extends AbstractEntityQos<QosPolicy.ForDomainParticipantFactory, DomainParticipantFactoryQos>
implements DomainParticipantFactoryQos {

    private static final long serialVersionUID = 1L;

    private DomainParticipantFactoryQosImpl(ServiceEnvironment environment,
                        Map<Class<? extends QosPolicy.ForDomainParticipantFactory>, QosPolicy.ForDomainParticipantFactory> policies) {
        super(environment, policies);
    }

    /** The QoS of DDS 1.2 §7.1.3: every policy at its default value. */
    public static DomainParticipantFactoryQos defaults(ServiceEnvironment environment) {
        PolicyFactoryImpl factory =
                (PolicyFactoryImpl) environment.getSPI().getPolicyFactory();
        Map<Class<? extends QosPolicy.ForDomainParticipantFactory>, QosPolicy.ForDomainParticipantFactory> policies =
                new LinkedHashMap<>();
        policies.put(EntityFactory.class, factory.EntityFactory());
        return new DomainParticipantFactoryQosImpl(environment, policies);
    }

    @Override
    public EntityFactory getEntityFactory() {
        return get(EntityFactory.class);
    }

    @Override
    protected DomainParticipantFactoryQos with(QosPolicy.ForDomainParticipantFactory policy) {
        Map<Class<? extends QosPolicy.ForDomainParticipantFactory>, QosPolicy.ForDomainParticipantFactory> policies = new LinkedHashMap<>();
        policies.putAll(this);
        policies.put(policyKey(policy), policy);
        return new DomainParticipantFactoryQosImpl(getEnvironment(), policies);
    }

    /**
     * The OMG source declares this overload with {@code ForDataWriter}, which no
     * factory-QoS policy implements; it can therefore never be called with an
     * argument. It is implemented here so the interface is satisfied, and it
     * rejects every policy, as §7.3.5.2 requires of an inapplicable policy.
     */
    @Override
    public DomainParticipantFactoryQos withPolicy(QosPolicy.ForDataWriter policy) {
        throw new IllegalArgumentException(
                "a DataWriter policy does not apply to a DomainParticipantFactory");
    }

    @Override
    public DomainParticipantFactoryQos withPolicies(QosPolicy.ForDataWriter... policy) {
        throw new IllegalArgumentException(
                "a DataWriter policy does not apply to a DomainParticipantFactory");
    }

    @Override
    protected DomainParticipantFactoryQos self() {
        return this;
    }

    @Override
    protected Class<? extends QosPolicy.ForDomainParticipantFactory> policyClass() {
        return QosPolicy.ForDomainParticipantFactory.class;
    }
}
