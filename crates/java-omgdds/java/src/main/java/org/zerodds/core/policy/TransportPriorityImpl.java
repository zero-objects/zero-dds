// SPDX-License-Identifier: Apache-2.0
package org.zerodds.core.policy;

import org.omg.dds.core.ServiceEnvironment;
import org.zerodds.ServiceEnvironmentImpl;
import org.omg.dds.core.policy.TransportPriority;

/**
 * ZeroDDS TransportPriority (DDS-Java-PSM 1.0 §7.3.5.1).
 *
 * <p>The hint that orders transport resources between writers of one topic (DDS 1.2 §7.1.3.8). Default: 0.
 */
public final class TransportPriorityImpl implements TransportPriority, Cloneable {

    private static final long serialVersionUID = 1L;

    private transient ServiceEnvironment environment;
    private final int value;

    public TransportPriorityImpl(ServiceEnvironment environment,
            int value) {
        this.environment = environment;
        this.value = value;
    }

    @Override
    public ServiceEnvironment getEnvironment() {
        return environment;
    }

    @Override
    public int getValue() {
        return value;
    }

    @Override
    public TransportPriority withValue(int value) {
        return new TransportPriorityImpl(environment, value);
    }

    @Override
    public boolean equals(Object o) {
        if (this == o) return true;
        if (!(o instanceof TransportPriority)) return false;
        TransportPriority other = (TransportPriority) o;
        return value == other.getValue();
    }

    @Override
    public int hashCode() {
        return value;
    }

    @Override
    public String toString() {
        return "TransportPriority[value=" + value + "]";
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
    public TransportPriorityImpl clone() {
        try {
            return (TransportPriorityImpl) super.clone();
        } catch (CloneNotSupportedException e) {
            throw new AssertionError(e);
        }
    }
}
