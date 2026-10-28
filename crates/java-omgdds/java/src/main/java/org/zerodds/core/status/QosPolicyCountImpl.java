// SPDX-License-Identifier: Apache-2.0
package org.zerodds.core.status;

import org.omg.dds.core.ServiceEnvironment;
import org.zerodds.ServiceEnvironmentImpl;
import org.omg.dds.core.policy.QosPolicy;
import org.omg.dds.core.policy.QosPolicyCount;

/**
 * ZeroDDS {@link QosPolicyCount} (DDS 1.2 §7.1.4.1): how often one policy
 * refused a match.
 */
public final class QosPolicyCountImpl implements QosPolicyCount, Cloneable {

    private static final long serialVersionUID = 1L;

    private transient ServiceEnvironment environment;
    private final Class<? extends QosPolicy> policyClass;
    private final int count;

    public QosPolicyCountImpl(ServiceEnvironment environment,
            Class<? extends QosPolicy> policyClass, int count) {
        this.environment = environment;
        this.policyClass = policyClass;
        this.count = count;
    }

    @Override
    public ServiceEnvironment getEnvironment() {
        return environment;
    }

    @Override
    public Class<? extends QosPolicy> getPolicyClass() {
        return policyClass;
    }

    @Override
    public int getCount() {
        return count;
    }

    @Override
    public boolean equals(Object o) {
        if (!(o instanceof QosPolicyCount)) {
            return false;
        }
        QosPolicyCount other = (QosPolicyCount) o;
        return policyClass.equals(other.getPolicyClass()) && count == other.getCount();
    }

    @Override
    public int hashCode() {
        return policyClass.hashCode() * 31 + count;
    }

    @Override
    public String toString() {
        return policyClass.getSimpleName() + " x" + count;
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
    public QosPolicyCountImpl clone() {
        try {
            return (QosPolicyCountImpl) super.clone();
        } catch (CloneNotSupportedException e) {
            throw new AssertionError(e);
        }
    }
}