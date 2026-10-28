// SPDX-License-Identifier: Apache-2.0
package org.zerodds.core;

import org.omg.dds.core.Duration;
import org.omg.dds.core.ModifiableTime;
import org.omg.dds.core.ServiceEnvironment;
import org.zerodds.ServiceEnvironmentImpl;
import org.omg.dds.core.Time;

import java.util.concurrent.TimeUnit;

/**
 * ZeroDDS {@link ModifiableTime} (DDS-Java-PSM 1.0 §7.3.4).
 */
public final class ModifiableTimeImpl extends ModifiableTime implements Cloneable {

    private static final long serialVersionUID = -881589038981141796L;

    private transient ServiceEnvironment environment;
    private long seconds;
    private long nanoseconds;

    public ModifiableTimeImpl(ServiceEnvironment environment, long seconds, long nanoseconds) {
        this.environment = environment;
        this.seconds = seconds;
        this.nanoseconds = nanoseconds;
    }

    @Override
    public ServiceEnvironment getEnvironment() {
        return environment;
    }

    @Override
    public long getTime(TimeUnit inThisUnit) {
        if (!isValid()) {
            return -1;
        }
        return inThisUnit.convert(seconds, TimeUnit.SECONDS)
                + inThisUnit.convert(nanoseconds, TimeUnit.NANOSECONDS);
    }

    @Override
    public long getRemainder(TimeUnit primaryUnit, TimeUnit remainderUnit) {
        if (!isValid()) {
            return -1;
        }
        long whole = primaryUnit.convert(seconds, TimeUnit.SECONDS)
                + primaryUnit.convert(nanoseconds, TimeUnit.NANOSECONDS);
        long nanos = TimeUnit.NANOSECONDS.convert(seconds, TimeUnit.SECONDS)
                + nanoseconds - TimeUnit.NANOSECONDS.convert(whole, primaryUnit);
        return remainderUnit.convert(Math.max(0, nanos), TimeUnit.NANOSECONDS);
    }

    @Override
    public boolean isValid() {
        return seconds >= 0;
    }

    @Override
    public void copyFrom(Time src) {
        if (src == null) {
            throw new IllegalArgumentException("copyFrom: the source Time is null");
        }
        // an invalid source (getTime == -1) is copied as an invalid time
        long[] parts = src.isValid()
                ? TimeImpl.split(src.getTime(TimeUnit.NANOSECONDS), TimeUnit.NANOSECONDS)
                : new long[]{-1, 0};
        this.seconds = parts[0];
        this.nanoseconds = parts[1];
    }

    @Override
    public Time immutableCopy() {
        return new TimeImpl(environment, seconds, nanoseconds);
    }

    @Override
    public void setTime(long time, TimeUnit unit) {
        long[] parts = TimeImpl.split(time, unit);
        this.seconds = parts[0];
        this.nanoseconds = parts[1];
    }

    @Override
    public void add(Duration duration) {
        add(duration.getDuration(TimeUnit.NANOSECONDS), TimeUnit.NANOSECONDS);
    }

    @Override
    public void add(long duration, TimeUnit unit) {
        long[] parts = TimeImpl.shift(seconds, nanoseconds, unit.toNanos(duration));
        this.seconds = parts[0];
        this.nanoseconds = parts[1];
    }

    @Override
    public void subtract(Duration duration) {
        subtract(duration.getDuration(TimeUnit.NANOSECONDS), TimeUnit.NANOSECONDS);
    }

    @Override
    public void subtract(long duration, TimeUnit unit) {
        long[] parts = TimeImpl.shift(seconds, nanoseconds, -unit.toNanos(duration));
        this.seconds = parts[0];
        this.nanoseconds = parts[1];
    }

    @Override
    public ModifiableTime modifiableCopy() {
        return new ModifiableTimeImpl(environment, seconds, nanoseconds);
    }

    @Override
    public int compareTo(Time other) {
        return Long.compare(getTime(TimeUnit.NANOSECONDS), other.getTime(TimeUnit.NANOSECONDS));
    }

    @Override
    public boolean equals(Object o) {
        if (this == o) return true;
        if (!(o instanceof Time)) return false;
        Time other = (Time) o;
        return getTime(TimeUnit.NANOSECONDS) == other.getTime(TimeUnit.NANOSECONDS);
    }

    @Override
    public int hashCode() {
        return Long.hashCode(getTime(TimeUnit.NANOSECONDS));
    }

    @Override
    public String toString() {
        return isValid() ? seconds + "s" + nanoseconds + "ns" : "INVALID";
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
    public ModifiableTimeImpl clone() {
        try {
            return (ModifiableTimeImpl) super.clone();
        } catch (CloneNotSupportedException e) {
            throw new AssertionError(e);
        }
    }
}
