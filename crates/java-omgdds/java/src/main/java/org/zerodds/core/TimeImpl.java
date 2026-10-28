// SPDX-License-Identifier: Apache-2.0
package org.zerodds.core;

import org.omg.dds.core.ModifiableTime;
import org.omg.dds.core.ServiceEnvironment;
import org.zerodds.ServiceEnvironmentImpl;
import org.omg.dds.core.Time;

import java.util.concurrent.TimeUnit;

/**
 * ZeroDDS {@link Time} (DDS-Java-PSM 1.0 §7.3.4).
 *
 * <p>Instances of this class are immutable; {@link #modifiableCopy()} of an
 * ordinary time returns a {@link ModifiableTimeImpl} holding the same instant.
 * A negative instant is invalid, as §7.3.4 requires.
 */
public class TimeImpl extends Time implements Cloneable {

    private static final long serialVersionUID = -132361141453190372L;

    transient ServiceEnvironment environment;
    long seconds;
    long nanoseconds;

    public TimeImpl(ServiceEnvironment environment, long seconds, long nanoseconds) {
        this.environment = environment;
        this.seconds = seconds;
        this.nanoseconds = nanoseconds;
    }

    /** The invalid instant of {@code environment} (DDS-Java-PSM 1.0 §7.3.4). */
    public static Time invalid(ServiceEnvironment environment) {
        return new TimeImpl(environment, -1, 0);
    }

    /**
     * Splits the instant {@code time} given in {@code unit} into seconds and
     * nanoseconds. A negative instant is the invalid time of §7.3.4 and is
     * returned as {@code {-1, 0}}, whatever the unit (a value of -5 ms has
     * zero whole seconds and must not pass for a valid instant).
     */
    public static long[] split(long time, TimeUnit unit) {
        if (time < 0) {
            return new long[]{-1, 0};
        }
        long seconds = unit.toSeconds(time);
        long nanos = unit.toNanos(time - unit.convert(seconds, TimeUnit.SECONDS));
        return new long[]{seconds, nanos};
    }

    /**
     * The instant {@code seconds}/{@code nanos} moved by {@code deltaNanos};
     * the result is invalid ({@code {-1, 0}}) when it lies before the epoch,
     * and an invalid instant stays invalid.
     */
    static long[] shift(long seconds, long nanos, long deltaNanos) {
        if (seconds < 0) {
            return new long[]{-1, 0};
        }
        long sec = seconds + deltaNanos / 1_000_000_000L;
        long ns = nanos + deltaNanos % 1_000_000_000L;
        sec += Math.floorDiv(ns, 1_000_000_000L);
        ns = Math.floorMod(ns, 1_000_000_000L);
        if (sec < 0) {
            return new long[]{-1, 0};
        }
        return new long[]{sec, ns};
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
        // (this - getTime(primaryUnit)) expressed in remainderUnit (§7.3.4).
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
    public ModifiableTime modifiableCopy() {
        return new ModifiableTimeImpl(environment, seconds, nanoseconds);
    }

    TimeImpl copy() {
        return new TimeImpl(environment, seconds, nanoseconds);
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
    public TimeImpl clone() {
        try {
            return (TimeImpl) super.clone();
        } catch (CloneNotSupportedException e) {
            throw new AssertionError(e);
        }
    }
}