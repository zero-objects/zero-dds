// SPDX-License-Identifier: Apache-2.0
package org.zerodds.core.policy;

import org.omg.dds.core.ServiceEnvironment;
import org.zerodds.ServiceEnvironmentImpl;
import org.omg.dds.core.policy.RequestedOffered;
import org.omg.dds.core.policy.Deadline;
import org.omg.dds.core.Duration;
import java.util.concurrent.TimeUnit;

/**
 * ZeroDDS Deadline (DDS-Java-PSM 1.0 §7.3.5.1).
 *
 * <p>The maximum interval between two samples of one instance (DDS 1.2 §7.1.3.4). Requested/offered: the offered period must not exceed the requested one. Default: infinite, which never misses a deadline.
 */
public final class DeadlineImpl implements Deadline, RequestedOffered<Deadline>, Cloneable {

    private static final long serialVersionUID = 1L;

    private transient ServiceEnvironment environment;
    private final org.omg.dds.core.Duration period;

    public DeadlineImpl(ServiceEnvironment environment,
            org.omg.dds.core.Duration period) {
        this.environment = environment;
        this.period = period;
    }

    @Override
    public ServiceEnvironment getEnvironment() {
        return environment;
    }

    @Override
    public org.omg.dds.core.Duration getPeriod() {
        return period;
    }

    @Override
    public Deadline withPeriod(Duration period) {
        return new DeadlineImpl(environment, period);
    }

    @Override
    public Deadline withPeriod(long period, TimeUnit unit) {
        return withPeriod(org.zerodds.core.DurationImpl.of(environment, period, unit));
    }

    @Override
    public int compareTo(Deadline other) {
        return period.compareTo(other.getPeriod());
    }

    @Override
    public Comparable<Deadline> requestedOfferedContract() {
        // DDS 1.4 §2.2.3.7: the offered period must be no longer than the requested one,
        // so a shorter period is the greater level of service.
        return other -> other.getPeriod().compareTo(period);
    }

    @Override
    public boolean equals(Object o) {
        if (this == o) return true;
        if (!(o instanceof Deadline)) return false;
        Deadline other = (Deadline) o;
        return java.util.Objects.equals(period, other.getPeriod());
    }

    @Override
    public int hashCode() {
        return java.util.Objects.hashCode(period);
    }

    @Override
    public String toString() {
        return "Deadline[period=" + period + "]";
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
    public DeadlineImpl clone() {
        try {
            return (DeadlineImpl) super.clone();
        } catch (CloneNotSupportedException e) {
            throw new AssertionError(e);
        }
    }
}
