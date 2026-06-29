// SPDX-License-Identifier: Apache-2.0
package org.omg.dds.core.policy;

/**
 * Composite QoS profile applied to DataReader/DataWriter (and, for the
 * PARTITION policy, the owning Publisher/Subscriber).
 *
 * <p>Carries the policies needed for RxO compatibility-matching and
 * per-instance behavior (DDS-DCPS 1.4 §2.2.3): RELIABILITY, DURABILITY,
 * HISTORY, DEADLINE, LIVELINESS, OWNERSHIP (+ OWNERSHIP_STRENGTH) and
 * PARTITION.
 *
 * <p>Immutable; use the {@code with*} copy-methods to derive a profile with
 * one policy changed. The 3-arg constructor and {@link #DEFAULT} are kept
 * for backward compatibility.
 */
public final class QosProfile {
    public static final QosProfile DEFAULT = new QosProfile(
            Reliability.RELIABLE_DEFAULT, Durability.VOLATILE, History.KEEP_LAST_10);

    private final Reliability reliability;
    private final Durability durability;
    private final History history;
    private final Deadline deadline;
    private final Liveliness liveliness;
    private final Ownership ownership;
    private final OwnershipStrength ownershipStrength;
    private final Partition partition;

    public QosProfile(Reliability reliability, Durability durability, History history) {
        this(reliability, durability, history,
                Deadline.INFINITE, Liveliness.AUTOMATIC_DEFAULT,
                Ownership.SHARED, OwnershipStrength.DEFAULT, Partition.DEFAULT);
    }

    public QosProfile(Reliability reliability, Durability durability, History history,
                      Deadline deadline, Liveliness liveliness, Ownership ownership,
                      OwnershipStrength ownershipStrength, Partition partition) {
        this.reliability = reliability;
        this.durability = durability;
        this.history = history;
        this.deadline = deadline;
        this.liveliness = liveliness;
        this.ownership = ownership;
        this.ownershipStrength = ownershipStrength;
        this.partition = partition;
    }

    public Reliability reliability() {
        return reliability;
    }

    public Durability durability() {
        return durability;
    }

    public History history() {
        return history;
    }

    public Deadline deadline() {
        return deadline;
    }

    public Liveliness liveliness() {
        return liveliness;
    }

    public Ownership ownership() {
        return ownership;
    }

    public OwnershipStrength ownershipStrength() {
        return ownershipStrength;
    }

    public Partition partition() {
        return partition;
    }

    public QosProfile withDeadline(Deadline d) {
        return new QosProfile(reliability, durability, history, d, liveliness,
                ownership, ownershipStrength, partition);
    }

    public QosProfile withLiveliness(Liveliness l) {
        return new QosProfile(reliability, durability, history, deadline, l,
                ownership, ownershipStrength, partition);
    }

    public QosProfile withOwnership(Ownership o) {
        return new QosProfile(reliability, durability, history, deadline, liveliness,
                o, ownershipStrength, partition);
    }

    public QosProfile withOwnershipStrength(OwnershipStrength s) {
        return new QosProfile(reliability, durability, history, deadline, liveliness,
                ownership, s, partition);
    }

    public QosProfile withPartition(Partition p) {
        return new QosProfile(reliability, durability, history, deadline, liveliness,
                ownership, ownershipStrength, p);
    }

    public QosProfile withHistory(History h) {
        return new QosProfile(reliability, durability, h, deadline, liveliness,
                ownership, ownershipStrength, partition);
    }

    public QosProfile withReliability(Reliability r) {
        return new QosProfile(r, durability, history, deadline, liveliness,
                ownership, ownershipStrength, partition);
    }

    public QosProfile withDurability(Durability d) {
        return new QosProfile(reliability, d, history, deadline, liveliness,
                ownership, ownershipStrength, partition);
    }

    /**
     * DDS-DCPS 1.4 §2.2.3 RxO-Compatibility — DataReader (this, requested)
     * vs. DataWriter (offered). Returns {@code true} iff the connection is
     * compatible across all RxO policies.
     */
    public boolean isCompatibleWith(QosProfile offered) {
        if (this.reliability.kind() == Reliability.Kind.RELIABLE
                && offered.reliability.kind() != Reliability.Kind.RELIABLE) {
            return false;
        }
        if (this.durability.ordinal() > offered.durability.ordinal()) {
            return false;
        }
        if (!this.deadline.isCompatibleWith(offered.deadline)) {
            return false;
        }
        if (!this.liveliness.isCompatibleWith(offered.liveliness)) {
            return false;
        }
        return this.ownership.isCompatibleWith(offered.ownership);
    }
}
