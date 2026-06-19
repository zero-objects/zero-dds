// SPDX-License-Identifier: Apache-2.0
package org.omg.dds.core;

import java.util.Objects;

/**
 * OMG DDS Java-PSM Duration_t — Spec §7.2.5.
 */
public final class Duration {
    public static final Duration INFINITE = new Duration(Integer.MAX_VALUE, 0x7FFFFFFFL);
    public static final Duration ZERO = new Duration(0, 0);

    private final long sec;
    private final long nanosec;

    private Duration(long sec, long nanosec) {
        this.sec = sec;
        this.nanosec = nanosec;
    }

    public static Duration of(long sec, long nanosec) {
        return new Duration(sec, nanosec);
    }

    public static Duration fromMillis(long millis) {
        return new Duration(millis / 1000, (millis % 1000) * 1_000_000L);
    }

    public static Duration fromSeconds(long seconds) {
        return new Duration(seconds, 0);
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

    public boolean isInfinite() {
        return sec == Integer.MAX_VALUE && nanosec == 0x7FFFFFFFL;
    }

    @Override
    public boolean equals(Object o) {
        if (this == o) return true;
        if (!(o instanceof Duration)) return false;
        Duration other = (Duration) o;
        return sec == other.sec && nanosec == other.nanosec;
    }

    @Override
    public int hashCode() {
        return Objects.hash(sec, nanosec);
    }

    @Override
    public String toString() {
        return isInfinite() ? "INFINITE" : "Duration(" + sec + "s, " + nanosec + "ns)";
    }
}
