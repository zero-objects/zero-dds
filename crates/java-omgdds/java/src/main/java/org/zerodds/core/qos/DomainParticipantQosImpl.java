// SPDX-License-Identifier: Apache-2.0
package org.zerodds.core.qos;

import org.omg.dds.core.ServiceEnvironment;
import org.omg.dds.core.policy.QosPolicy;
import org.omg.dds.core.policy.EntityFactory;
import org.omg.dds.core.policy.UserData;
import org.omg.dds.domain.DomainParticipantQos;
import org.zerodds.core.policy.PolicyFactoryImpl;

import java.util.LinkedHashMap;
import java.util.Map;

/**
 * ZeroDDS DomainParticipantQos (DDS-Java-PSM 1.0 §7.3.5.2).
 *
 * <p>The QoS of a DomainParticipant (DDS-Java-PSM 1.0 §7.4.2): USER_DATA and ENTITY_FACTORY apply (DDS 1.2 §7.1.3.1).
 */
public final class DomainParticipantQosImpl
extends AbstractEntityQos<QosPolicy.ForDomainParticipant, DomainParticipantQos>
implements DomainParticipantQos {

    private static final long serialVersionUID = 1L;

    private DomainParticipantQosImpl(ServiceEnvironment environment,
                        Map<Class<? extends QosPolicy.ForDomainParticipant>, QosPolicy.ForDomainParticipant> policies) {
        super(environment, policies);
    }

    /** The QoS of DDS 1.2 §7.1.3: every policy at its default value. */
    public static DomainParticipantQos defaults(ServiceEnvironment environment) {
        PolicyFactoryImpl factory =
                (PolicyFactoryImpl) environment.getSPI().getPolicyFactory();
        Map<Class<? extends QosPolicy.ForDomainParticipant>, QosPolicy.ForDomainParticipant> policies =
                new LinkedHashMap<>();
        policies.put(UserData.class, factory.UserData());
        policies.put(EntityFactory.class, factory.EntityFactory());
        return new DomainParticipantQosImpl(environment, policies);
    }

    @Override
    public UserData getUserData() {
        return get(UserData.class);
    }

    @Override
    public EntityFactory getEntityFactory() {
        return get(EntityFactory.class);
    }

    @Override
    protected DomainParticipantQos with(QosPolicy.ForDomainParticipant policy) {
        Map<Class<? extends QosPolicy.ForDomainParticipant>, QosPolicy.ForDomainParticipant> policies = new LinkedHashMap<>();
        policies.putAll(this);
        policies.put(policyKey(policy), policy);
        return new DomainParticipantQosImpl(getEnvironment(), policies);
    }

    @Override
    protected DomainParticipantQos self() {
        return this;
    }

    @Override
    protected Class<? extends QosPolicy.ForDomainParticipant> policyClass() {
        return QosPolicy.ForDomainParticipant.class;
    }
}
