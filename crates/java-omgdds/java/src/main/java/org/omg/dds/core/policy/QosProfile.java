// SPDX-License-Identifier: Apache-2.0
package org.omg.dds.core.policy;

/**
 * Composite QoS profile applied to DataReader/DataWriter.
 *
 * <p>Represents the subset of policies needed for compatibility-matching
 * (Spec §2.2.4 RxO-Compatibility).
 */
public final class QosProfile {
    public static final QosProfile DEFAULT = new QosProfile(
            Reliability.RELIABLE_DEFAULT, Durability.VOLATILE, History.KEEP_LAST_10);

    private final Reliability reliability;
    private final Durability durability;
    private final History history;

    public QosProfile(Reliability reliability, Durability durability, History history) {
        this.reliability = reliability;
        this.durability = durability;
        this.history = history;
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

    /**
     * Spec §2.2.4 RxO-Compatibility — DataReader (this) vs. DataWriter (offered).
     * Reader-Reliability = RELIABLE requires Writer-Reliability = RELIABLE.
     * Reader-Durability {@code <=} Writer-Durability (in spec ordering).
     */
    public boolean isCompatibleWith(QosProfile offered) {
        if (this.reliability.kind() == Reliability.Kind.RELIABLE
                && offered.reliability.kind() != Reliability.Kind.RELIABLE) {
            return false;
        }
        return this.durability.ordinal() <= offered.durability.ordinal();
    }
}
