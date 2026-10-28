// SPDX-License-Identifier: Apache-2.0
package org.zerodds.core;

import org.omg.dds.core.Duration;
import org.omg.dds.core.ServiceEnvironment;
import org.zerodds.ServiceEnvironmentImpl;

import java.util.concurrent.TimeUnit;

/**
 * ZeroDDS {@link Duration} (DDS-Java-PSM 1.0 §7.3.4).
 *
 * <p>Instances are immutable; the {@code with}-style mutation of
 * {@link org.omg.dds.core.policy.Durability} and friends calls the copy
 * constructors of this class. A duration of magnitude {@link Long#MAX_VALUE}
 * in any unit is infinite, as §7.3.4 requires.
 */
public final class DurationImpl extends Duration implements Cloneable {

    private static final long serialVersionUID = 6926514364942353575L;

    private transient ServiceEnvironment environment;
    private final long seconds;
    private final long nanoseconds;

    public DurationImpl(ServiceEnvironment environment, long seconds, long nanoseconds) {
        this.environment = environment;
        this.seconds = seconds;
        this.nanoseconds = nanoseconds;
    }

    /** A duration of {@code duration} {@code unit}, built by {@code env}'s service. */
    public static Duration of(ServiceEnvironment environment, long duration, TimeUnit unit) {
        if (duration == Long.MAX_VALUE) {
            return infinite(environment); // §7.3.4: magnitude Long.MAX_VALUE in any unit
        }
        long[] parts = split(duration, unit);
        return new DurationImpl(environment, parts[0], parts[1]);
    }

    /** The infinite duration of {@code environment} (DDS 1.2 §7.1.3, infinite defaults). */
    public static Duration infinite(ServiceEnvironment environment) {
        return new DurationImpl(environment, Long.MAX_VALUE, 0);
    }

    /** The zero duration of {@code environment}. */
    public static Duration zero(ServiceEnvironment environment) {
        return new DurationImpl(environment, 0, 0);
    }

    /** Splits {@code duration} of {@code unit} into the second/nanosecond pair of this class. */
    public static long[] split(long duration, TimeUnit unit) {
        long seconds = unit.toSeconds(duration);
        long nanos = unit.toNanos(duration - unit.convert(seconds, TimeUnit.SECONDS));
        return new long[]{seconds, nanos};
    }

    @Override
    public ServiceEnvironment getEnvironment() {
        return environment;
    }

    @Override
    public long getDuration(TimeUnit inThisUnit) {
        if (isInfinite()) {
            return Long.MAX_VALUE;
        }
        return inThisUnit.convert(seconds, TimeUnit.SECONDS)
                + inThisUnit.convert(nanoseconds, TimeUnit.NANOSECONDS);
    }

    @Override
    public long getRemainder(TimeUnit primaryUnit, TimeUnit remainderUnit) {
        if (isInfinite()) {
            return Long.MAX_VALUE;
        }
        return subtract(primaryUnit.convert(seconds, TimeUnit.SECONDS)
                + primaryUnit.convert(nanoseconds, TimeUnit.NANOSECONDS), primaryUnit)
                .getDuration(remainderUnit);
    }

    @Override
    public boolean isZero() {
        return seconds == 0 && nanoseconds == 0;
    }

    @Override
    public boolean isInfinite() {
        return seconds == Long.MAX_VALUE;
    }

    @Override
    public Duration add(Duration duration) {
        return add(duration.getDuration(TimeUnit.NANOSECONDS), TimeUnit.NANOSECONDS);
    }

    @Override
    public Duration add(long duration, TimeUnit unit) {
        if (isInfinite()) {
            return this;
        }
        long[] parts = split(unit.toNanos(duration), TimeUnit.NANOSECONDS);
        return new DurationImpl(environment, seconds + parts[0], nanoseconds + parts[1]);
    }

    @Override
    public Duration subtract(Duration duration) {
        return subtract(duration.getDuration(TimeUnit.NANOSECONDS), TimeUnit.NANOSECONDS);
    }

    @Override
    public Duration subtract(long duration, TimeUnit unit) {
        if (isInfinite()) {
            return this;
        }
        long[] parts = split(unit.toNanos(duration), TimeUnit.NANOSECONDS);
        return new DurationImpl(environment, seconds - parts[0], nanoseconds - parts[1]);
    }

    @Override
    public int compareTo(Duration other) {
        return Long.compare(getDuration(TimeUnit.NANOSECONDS), other.getDuration(TimeUnit.NANOSECONDS));
    }

    @Override
    public boolean equals(Object o) {
        if (this == o) return true;
        if (!(o instanceof Duration)) return false;
        Duration other = (Duration) o;
        return getDuration(TimeUnit.NANOSECONDS) == other.getDuration(TimeUnit.NANOSECONDS);
    }

    @Override
    public int hashCode() {
        return Long.hashCode(getDuration(TimeUnit.NANOSECONDS));
    }

    @Override
    public String toString() {
        return isInfinite() ? "INFINITE" : seconds + "s" + nanoseconds + "ns";
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
    public DurationImpl clone() {
        try {
            return (DurationImpl) super.clone();
        } catch (CloneNotSupportedException e) {
            throw new AssertionError(e);
        }
    }
}