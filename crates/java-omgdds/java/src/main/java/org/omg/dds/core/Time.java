// SPDX-License-Identifier: Apache-2.0
package org.omg.dds.core;

import java.util.Objects;

/**
 * OMG DDS Java-PSM Time_t — Spec §7.2.5.
 *
 * <p>Time as `(seconds, nanoseconds)`. Semantically immutable.
 */
public final class Time {
    public static final Time INFINITE = new Time(Integer.MAX_VALUE, 0x7FFFFFFFL);
    public static final Time INVALID = new Time(-1, 0xFFFFFFFFL);
    public static final Time ZERO = new Time(0, 0);

    private final long sec;
    private final long nanosec;

    private Time(long sec, long nanosec) {
        this.sec = sec;
        this.nanosec = nanosec;
    }

    public static Time of(long sec, long nanosec) {
        return new Time(sec, nanosec);
    }

    public static Time fromMillis(long millis) {
        return new Time(millis / 1000, (millis % 1000) * 1_000_000L);
    }

    public long sec() {
        return sec;
    }

    public long nanosec() {
        return nanosec;
    }

    public long toMillis() {
        return sec * 1000 + nanosec / 1_000_000;
    }

    @Override
    public boolean equals(Object o) {
        if (this == o) return true;
        if (!(o instanceof Time other)) return false;
        return sec == other.sec && nanosec == other.nanosec;
    }

    @Override
    public int hashCode() {
        return Objects.hash(sec, nanosec);
    }

    @Override
    public String toString() {
        return "Time(" + sec + "s, " + nanosec + "ns)";
    }
}
