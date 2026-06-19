// SPDX-License-Identifier: Apache-2.0
package org.omg.dds.core.policy;

import org.omg.dds.core.Duration;

import java.util.Objects;

/**
 * OMG DDS Java-PSM ReliabilityQosPolicy — Spec §7.2.4.
 */
public final class Reliability {
    public enum Kind {
        BEST_EFFORT,
        RELIABLE,
    }

    public static final Reliability BEST_EFFORT_DEFAULT =
            new Reliability(Kind.BEST_EFFORT, Duration.fromMillis(100));
    public static final Reliability RELIABLE_DEFAULT =
            new Reliability(Kind.RELIABLE, Duration.fromMillis(100));

    private final Kind kind;
    private final Duration maxBlockingTime;

    public Reliability(Kind kind, Duration maxBlockingTime) {
        this.kind = Objects.requireNonNull(kind);
        this.maxBlockingTime = Objects.requireNonNull(maxBlockingTime);
    }

    public Kind kind() {
        return kind;
    }

    public Duration maxBlockingTime() {
        return maxBlockingTime;
    }

    @Override
    public boolean equals(Object o) {
        if (this == o) return true;
        if (!(o instanceof Reliability)) return false;
        Reliability other = (Reliability) o;
        return kind == other.kind && maxBlockingTime.equals(other.maxBlockingTime);
    }

    @Override
    public int hashCode() {
        return Objects.hash(kind, maxBlockingTime);
    }
}
