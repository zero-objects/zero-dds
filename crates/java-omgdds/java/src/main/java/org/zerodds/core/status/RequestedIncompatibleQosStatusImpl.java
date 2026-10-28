// SPDX-License-Identifier: Apache-2.0
package org.zerodds.core.status;

import org.omg.dds.core.ServiceEnvironment;
import org.zerodds.ServiceEnvironmentImpl;
import org.omg.dds.core.status.RequestedIncompatibleQosStatus;
import java.util.Collections;
import java.util.LinkedHashSet;
import java.util.Set;
import org.omg.dds.core.policy.QosPolicy;
import org.omg.dds.core.policy.QosPolicyCount;

/**
 * ZeroDDS RequestedIncompatibleQosStatus.
 *
 * <p>REQUESTED_INCOMPATIBLE_QOS: a writer offered QoS this reader cannot accept (DDS 1.2 §7.1.4.1).
 *
 * <p>Immutable: §7.3.7.1 lets an implementation pool and reuse a status
 * object, so an application that keeps one has to copy it. ZeroDDS never
 * changes a status it handed out, so the {@code _change} counts are fixed
 * at construction.
 */
public final class RequestedIncompatibleQosStatusImpl extends RequestedIncompatibleQosStatus implements Cloneable {

    private static final long serialVersionUID = 1L;

    private transient ServiceEnvironment environment;
    private final int totalCount;
    private final int totalCountChange;
    private final Class<? extends QosPolicy> lastPolicyClass;
    private final LinkedHashSet<QosPolicyCount> policies;

    public RequestedIncompatibleQosStatusImpl(ServiceEnvironment environment,
            int totalCount,
            int totalCountChange,
            Class<? extends QosPolicy> lastPolicyClass,
            Set<QosPolicyCount> policies) {
        this.environment = environment;
        this.totalCount = totalCount;
        this.totalCountChange = totalCountChange;
        this.lastPolicyClass = lastPolicyClass;
        this.policies = new LinkedHashSet<>(policies);
    }

    @Override
    public ServiceEnvironment getEnvironment() {
        return environment;
    }

    @Override
    public int getTotalCount() {
        return totalCount;
    }

    @Override
    public int getTotalCountChange() {
        return totalCountChange;
    }

    @Override
    public Class<? extends QosPolicy> getLastPolicyClass() {
        return lastPolicyClass;
    }

    @Override
    public Set<QosPolicyCount> getPolicies() {
        return Collections.unmodifiableSet(policies);
    }

    @Override
    public boolean equals(Object o) {
        if (this == o) {
            return true;
        }
        if (!(o instanceof RequestedIncompatibleQosStatusImpl)) {
            return false;
        }
        RequestedIncompatibleQosStatusImpl that = (RequestedIncompatibleQosStatusImpl) o;
        return totalCount == that.totalCount && totalCountChange == that.totalCountChange && java.util.Objects.equals(lastPolicyClass, that.lastPolicyClass) && java.util.Objects.equals(policies, that.policies);
    }

    @Override
    public int hashCode() {
        return java.util.Objects.hash(totalCount, totalCountChange, lastPolicyClass, policies);
    }

    @Override
    public String toString() {
        return "RequestedIncompatibleQosStatus[" + "totalCount=" + totalCount + "," + "totalCountChange=" + totalCountChange + "," + "lastPolicyClass=" + lastPolicyClass + "," + "policies=" + policies + "]";
    }

    /**
     * Java serialization (DDS-Java-PSM 1.0 §7.3.3) carries the value, not its
     * {@link ServiceEnvironment}: an environment is a runtime object and is not
     * serializable. A deserialized object belongs to the default environment of
     * this implementation.
     */
    private void readObject(java.io.ObjectInputStream in)
            throws java.io.IOException, ClassNotFoundException {
        in.defaultReadObject();
        environment = ServiceEnvironmentImpl.getDefaultEnvironment();
    }

    /** A copy by value (DDS-Java-PSM 1.0 7.3.3). */
    @Override
    public RequestedIncompatibleQosStatusImpl clone() {
        try {
            return (RequestedIncompatibleQosStatusImpl) super.clone();
        } catch (CloneNotSupportedException e) {
            throw new AssertionError(e);
        }
    }
}
