// SPDX-License-Identifier: Apache-2.0
package org.zerodds.core.policy;

import org.omg.dds.core.ServiceEnvironment;
import org.zerodds.ServiceEnvironmentImpl;
import org.omg.dds.core.policy.TimeBasedFilter;
import org.omg.dds.core.Duration;
import java.util.concurrent.TimeUnit;

/**
 * ZeroDDS TimeBasedFilter (DDS-Java-PSM 1.0 §7.3.5.1).
 *
 * <p>The minimum interval between two samples of one instance (DDS 1.2 §7.1.3.6). Default: zero, which passes every sample.
 */
public final class TimeBasedFilterImpl implements TimeBasedFilter, Cloneable {

    private static final long serialVersionUID = 1L;

    private transient ServiceEnvironment environment;
    private final org.omg.dds.core.Duration minimumSeparation;

    public TimeBasedFilterImpl(ServiceEnvironment environment,
            org.omg.dds.core.Duration minimumSeparation) {
        this.environment = environment;
        this.minimumSeparation = minimumSeparation;
    }

    @Override
    public ServiceEnvironment getEnvironment() {
        return environment;
    }

    @Override
    public org.omg.dds.core.Duration getMinimumSeparation() {
        return minimumSeparation;
    }

    @Override
    public TimeBasedFilter withMinimumSeparation(Duration minimumSeparation) {
        return new TimeBasedFilterImpl(environment, minimumSeparation);
    }

    @Override
    public TimeBasedFilter withMinimumSeparation(long minimumSeparation, TimeUnit unit) {
        return withMinimumSeparation(org.zerodds.core.DurationImpl.of(environment, minimumSeparation, unit));
    }

    @Override
    public boolean equals(Object o) {
        if (this == o) return true;
        if (!(o instanceof TimeBasedFilter)) return false;
        TimeBasedFilter other = (TimeBasedFilter) o;
        return java.util.Objects.equals(minimumSeparation, other.getMinimumSeparation());
    }

    @Override
    public int hashCode() {
        return java.util.Objects.hashCode(minimumSeparation);
    }

    @Override
    public String toString() {
        return "TimeBasedFilter[minimumSeparation=" + minimumSeparation + "]";
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
    public TimeBasedFilterImpl clone() {
        try {
            return (TimeBasedFilterImpl) super.clone();
        } catch (CloneNotSupportedException e) {
            throw new AssertionError(e);
        }
    }
}
