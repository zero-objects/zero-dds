// SPDX-License-Identifier: Apache-2.0
package org.omg.dds.core.policy;

import org.omg.dds.core.Duration;

import java.util.Objects;

/**
 * OMG DDS LivelinessQosPolicy — DDS-DCPS 1.4 §2.2.3.11.
 *
 * <p>{@code kind} determines how a DataWriter asserts that it is still
 * "alive": {@code AUTOMATIC} (the infrastructure renews on the writer's
 * behalf), {@code MANUAL_BY_PARTICIPANT}, or {@code MANUAL_BY_TOPIC} (the
 * application must call
 * {@link org.omg.dds.pub.DataWriter#assertLiveliness()} within
 * {@code leaseDuration}).
 *
 * <p>RxO (§2.2.3, compatibility table): compatible iff
 * {@code offered.kind >= requested.kind} in the strength ordering
 * {@code AUTOMATIC < MANUAL_BY_PARTICIPANT < MANUAL_BY_TOPIC} AND
 * {@code offered.leaseDuration <= requested.leaseDuration}.
 *
 * <p>If a writer fails to renew within its lease, the reader's matched
 * publication transitions {@code alive -> not_alive} and the
 * {@code LIVELINESS_CHANGED} status (§2.2.4.1) is updated.
 */
public final class Liveliness {
    public enum Kind {
        AUTOMATIC,
        MANUAL_BY_PARTICIPANT,
        MANUAL_BY_TOPIC,
    }

    /** Spec default: AUTOMATIC, infinite lease. */
    public static final Liveliness AUTOMATIC_DEFAULT =
            new Liveliness(Kind.AUTOMATIC, Duration.INFINITE);

    private final Kind kind;
    private final Duration leaseDuration;

    public Liveliness(Kind kind, Duration leaseDuration) {
        this.kind = Objects.requireNonNull(kind, "kind");
        this.leaseDuration = Objects.requireNonNull(leaseDuration, "leaseDuration");
    }

    public Kind kind() {
        return kind;
    }

    public Duration leaseDuration() {
        return leaseDuration;
    }

    /**
     * RxO compatibility (§2.2.3 compatibility table). {@code this} is the
     * requested (reader) policy, {@code offered} the writer policy.
     */
    public boolean isCompatibleWith(Liveliness offered) {
        if (offered.kind.ordinal() < this.kind.ordinal()) {
            return false;
        }
        if (this.leaseDuration.isInfinite()) {
            return true;
        }
        if (offered.leaseDuration.isInfinite()) {
            return false;
        }
        return offered.leaseDuration.toMillis() <= this.leaseDuration.toMillis();
    }

    @Override
    public boolean equals(Object o) {
        if (this == o) return true;
        if (!(o instanceof Liveliness)) return false;
        Liveliness other = (Liveliness) o;
        return kind == other.kind && leaseDuration.equals(other.leaseDuration);
    }

    @Override
    public int hashCode() {
        return Objects.hash(kind, leaseDuration);
    }

    @Override
    public String toString() {
        return "Liveliness(" + kind + ", " + leaseDuration + ")";
    }
}
