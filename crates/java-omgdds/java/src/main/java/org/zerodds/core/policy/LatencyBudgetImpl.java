// SPDX-License-Identifier: Apache-2.0
package org.zerodds.core.policy;

import org.omg.dds.core.ServiceEnvironment;
import org.zerodds.ServiceEnvironmentImpl;
import org.omg.dds.core.policy.RequestedOffered;
import org.omg.dds.core.policy.LatencyBudget;
import org.omg.dds.core.Duration;
import java.util.concurrent.TimeUnit;

/**
 * ZeroDDS LatencyBudget (DDS-Java-PSM 1.0 §7.3.5.1).
 *
 * <p>The additional delay the service may add to deliver a sample (DDS 1.2 §7.1.3.9). Requested/offered: the offered budget must not exceed the requested one. Default: zero.
 */
public final class LatencyBudgetImpl implements LatencyBudget, RequestedOffered<LatencyBudget>, Cloneable {

    private static final long serialVersionUID = 1L;

    private transient ServiceEnvironment environment;
    private final org.omg.dds.core.Duration duration;

    public LatencyBudgetImpl(ServiceEnvironment environment,
            org.omg.dds.core.Duration duration) {
        this.environment = environment;
        this.duration = duration;
    }

    @Override
    public ServiceEnvironment getEnvironment() {
        return environment;
    }

    @Override
    public org.omg.dds.core.Duration getDuration() {
        return duration;
    }

    @Override
    public LatencyBudget withDuration(Duration duration) {
        return new LatencyBudgetImpl(environment, duration);
    }

    @Override
    public LatencyBudget withDuration(long duration, TimeUnit unit) {
        return withDuration(org.zerodds.core.DurationImpl.of(environment, duration, unit));
    }

    @Override
    public int compareTo(LatencyBudget other) {
        return duration.compareTo(other.getDuration());
    }

    @Override
    public Comparable<LatencyBudget> requestedOfferedContract() {
        // DDS 1.4 §2.2.3.8: the offered duration must be no longer than the requested one.
        return other -> other.getDuration().compareTo(duration);
    }

    @Override
    public boolean equals(Object o) {
        if (this == o) return true;
        if (!(o instanceof LatencyBudget)) return false;
        LatencyBudget other = (LatencyBudget) o;
        return java.util.Objects.equals(duration, other.getDuration());
    }

    @Override
    public int hashCode() {
        return java.util.Objects.hashCode(duration);
    }

    @Override
    public String toString() {
        return "LatencyBudget[duration=" + duration + "]";
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
    public LatencyBudgetImpl clone() {
        try {
            return (LatencyBudgetImpl) super.clone();
        } catch (CloneNotSupportedException e) {
            throw new AssertionError(e);
        }
    }
}
