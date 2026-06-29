// SPDX-License-Identifier: Apache-2.0
package org.omg.dds.core.policy;

import org.omg.dds.core.Duration;

import java.util.Objects;

/**
 * OMG DDS DeadlineQosPolicy — DDS-DCPS 1.4 §2.2.3.7.
 *
 * <p>The {@code period} expresses the maximum duration within which a new
 * sample is expected on a per-instance basis. On the writer side it is the
 * <em>offered</em> deadline; on the reader side the <em>requested</em>
 * deadline. RxO (§2.2.3, table "QoS compatibility"): a connection is
 * compatible iff {@code offered.period <= requested.period}.
 *
 * <p>When the requested deadline elapses on the reader without a fresh
 * sample for an instance, the {@code REQUESTED_DEADLINE_MISSED} status is
 * incremented (§2.2.4.1) — surfaced via
 * {@link org.omg.dds.sub.DataReader#getRequestedDeadlineMissedStatus()}.
 */
public final class Deadline {
    /** The default (and only spec default) is an infinite period — no deadline. */
    public static final Deadline INFINITE = new Deadline(Duration.INFINITE);

    private final Duration period;

    public Deadline(Duration period) {
        this.period = Objects.requireNonNull(period, "period");
    }

    public static Deadline of(Duration period) {
        return new Deadline(period);
    }

    public Duration period() {
        return period;
    }

    /**
     * RxO compatibility (DDS-DCPS 1.4 §2.2.3, compatibility table): the
     * offered (writer) deadline must be {@code <=} the requested (reader)
     * deadline. {@code this} is the requested (reader) policy.
     */
    public boolean isCompatibleWith(Deadline offered) {
        if (this.period.isInfinite()) {
            return true; // requested = infinite accepts any offered period.
        }
        if (offered.period.isInfinite()) {
            return false; // offered infinite cannot satisfy a finite request.
        }
        return offered.period.toMillis() <= this.period.toMillis();
    }

    @Override
    public boolean equals(Object o) {
        if (this == o) return true;
        if (!(o instanceof Deadline)) return false;
        return period.equals(((Deadline) o).period);
    }

    @Override
    public int hashCode() {
        return Objects.hash(period);
    }

    @Override
    public String toString() {
        return "Deadline(" + period + ")";
    }
}
