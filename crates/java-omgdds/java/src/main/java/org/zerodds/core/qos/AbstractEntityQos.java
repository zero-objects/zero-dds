// SPDX-License-Identifier: Apache-2.0
package org.zerodds.core.qos;

import org.omg.dds.core.EntityQos;
import org.omg.dds.core.ServiceEnvironment;
import org.omg.dds.core.policy.PolicyFactory;
import org.omg.dds.core.policy.QosPolicy;

import java.util.AbstractMap;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Set;

/**
 * The storage of an {@link EntityQos} (DDS-Java-PSM 1.0 §7.3.5.2).
 *
 * <p>§7.3.5.2 makes an {@code EntityQos} a {@code java.util.Map} from
 * {@code Class<? extends QosPolicy>} to the policy, which this class uses
 * verbatim: the entry key is the policy interface's own {@code Class} object
 * as the application names it, and {@link #getPolicyFactory()} hands out the
 * default-initialized policies of the environment.
 *
 * @param <POLICY>    the marker interface of the policies this QoS accepts.
 * @param <SELF>      the concrete QoS interface, so that {@code withPolicy}
 *                    returns the concrete type (DDS-Java-PSM 1.0 §7.3.5.3).
 */
public abstract class AbstractEntityQos<POLICY extends QosPolicy, SELF extends EntityQos<POLICY>>
extends AbstractMap<Class<? extends POLICY>, POLICY> implements Cloneable {

    private static final long serialVersionUID = 1L;

    private final ServiceEnvironment environment;
    private final Map<Class<? extends POLICY>, POLICY> policies;

    protected AbstractEntityQos(ServiceEnvironment environment,
                                Map<Class<? extends POLICY>, POLICY> policies) {
        this.environment = environment;
        this.policies = new LinkedHashMap<>(policies);
    }

    public ServiceEnvironment getEnvironment() {
        return environment;
    }

    @Override
    public Set<Entry<Class<? extends POLICY>, POLICY>> entrySet() {
        return Collections.unmodifiableMap(policies).entrySet();
    }

    @Override
    public POLICY get(Object key) {
        return policies.get(key);
    }

    @SuppressWarnings("unchecked")
    public <P extends POLICY> P get(Class<P> id) {
        return (P) policies.get(id);
    }

    public PolicyFactory getPolicyFactory() {
        return environment.getSPI().getPolicyFactory();
    }

    /** A copy of this QoS with {@code policy} set, the other policies unchanged. */
    protected abstract SELF with(POLICY policy);

    /**
     * §7.3.5.3: copy this QoS and override one policy. The concrete QoS
     * interfaces narrow this to their own return type.
     */
    public SELF withPolicy(POLICY policy) {
        return with(policy);
    }

    @SafeVarargs
    @SuppressWarnings("unchecked")
    public final SELF withPolicies(POLICY... policy) {
        SELF result = self();
        for (POLICY p : policy) {
            result = (SELF) result.withPolicy(p);
        }
        return result;
    }

    /**
     * A copy by value (DDS-Java-PSM 1.0 7.3.3). The policies are immutable and the map of them is
     * never changed after construction, so the copy may share it.
     */
    @Override
    public AbstractEntityQos<POLICY, SELF> clone() {
        try {
            @SuppressWarnings("unchecked")
            AbstractEntityQos<POLICY, SELF> copy = (AbstractEntityQos<POLICY, SELF>) super.clone();
            return copy;
        } catch (CloneNotSupportedException e) {
            throw new AssertionError(e);
        }
    }

    /** This object, typed as its own QoS interface. */
    protected abstract SELF self();

    /** The marker interface of the policies this QoS accepts, e.g. {@code ForDataWriter}. */
    protected abstract Class<? extends POLICY> policyClass();

    /**
     * The map key of {@code policy}: the policy interface of {@code Table 7.2}
     * (DDS-Java-PSM 1.0 §7.3.5.1), for example {@code Reliability.class}.
     *
     * @throws IllegalArgumentException if the policy does not apply to this QoS
     *     (§7.3.5.2).
     */
    @SuppressWarnings("unchecked")
    protected Class<? extends POLICY> policyKey(QosPolicy policy) {
        Class<? extends POLICY> marker = policyClass();
        for (Class<?> candidate : policy.getClass().getInterfaces()) {
            if (candidate.isInterface() && !marker.equals(candidate)
                    && marker.isAssignableFrom(candidate)) {
                return (Class<? extends POLICY>) candidate;
            }
        }
        throw new IllegalArgumentException(
                policy + " does not apply to " + getClass().getSimpleName());
    }

    @Override
    public boolean equals(Object o) {
        if (this == o) return true;
        if (o == null || !getClass().equals(o.getClass())) return false;
        AbstractEntityQos<?, ?> other = (AbstractEntityQos<?, ?>) o;
        return policies.equals(other.policies);
    }

    @Override
    public int hashCode() {
        return policies.hashCode();
    }

    @Override
    public String toString() {
        return getClass().getSimpleName() + policies;
    }
}