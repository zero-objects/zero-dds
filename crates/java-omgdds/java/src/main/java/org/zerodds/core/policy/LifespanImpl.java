// SPDX-License-Identifier: Apache-2.0
package org.zerodds.core.policy;

import org.omg.dds.core.ServiceEnvironment;
import org.zerodds.ServiceEnvironmentImpl;
import org.omg.dds.core.policy.Lifespan;
import org.omg.dds.core.Duration;
import java.util.concurrent.TimeUnit;

/**
 * ZeroDDS Lifespan (DDS-Java-PSM 1.0 §7.3.5.1).
 *
 * <p>How long a sample stays valid in the writer history (DDS 1.2 §7.1.3.7). Default: infinite.
 */
public final class LifespanImpl implements Lifespan, Cloneable {

    private static final long serialVersionUID = 1L;

    private transient ServiceEnvironment environment;
    private final org.omg.dds.core.Duration duration;

    public LifespanImpl(ServiceEnvironment environment,
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
    public Lifespan withDuration(Duration duration) {
        return new LifespanImpl(environment, duration);
    }

    @Override
    public Lifespan withDuration(long duration, TimeUnit unit) {
        return withDuration(org.zerodds.core.DurationImpl.of(environment, duration, unit));
    }

    @Override
    public boolean equals(Object o) {
        if (this == o) return true;
        if (!(o instanceof Lifespan)) return false;
        Lifespan other = (Lifespan) o;
        return java.util.Objects.equals(duration, other.getDuration());
    }

    @Override
    public int hashCode() {
        return java.util.Objects.hashCode(duration);
    }

    @Override
    public String toString() {
        return "Lifespan[duration=" + duration + "]";
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
    public LifespanImpl clone() {
        try {
            return (LifespanImpl) super.clone();
        } catch (CloneNotSupportedException e) {
            throw new AssertionError(e);
        }
    }
}
