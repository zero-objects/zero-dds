// SPDX-License-Identifier: Apache-2.0
package org.omg.dds.core.policy;

import java.util.Objects;

/**
 * OMG DDS OwnershipQosPolicy — DDS-DCPS 1.4 §2.2.3.12.1.
 *
 * <p>{@code SHARED}: multiple writers may update an instance concurrently;
 * the reader sees every update. {@code EXCLUSIVE}: for each instance, only
 * the writer with the highest {@link OwnershipStrength} (the "owner") is
 * allowed to update the reader's view; samples from lower-strength writers
 * are filtered out (§2.2.3.12.1, "the DataReader will only receive
 * modifications from the DataWriter with the highest strength").
 *
 * <p>RxO (§2.2.3 compatibility table): both sides must use the same kind.
 */
public final class Ownership {
    public enum Kind {
        SHARED,
        EXCLUSIVE,
    }

    public static final Ownership SHARED = new Ownership(Kind.SHARED);
    public static final Ownership EXCLUSIVE = new Ownership(Kind.EXCLUSIVE);

    private final Kind kind;

    public Ownership(Kind kind) {
        this.kind = Objects.requireNonNull(kind, "kind");
    }

    public Kind kind() {
        return kind;
    }

    /** RxO compatibility (§2.2.3): the kinds must match exactly. */
    public boolean isCompatibleWith(Ownership offered) {
        return this.kind == offered.kind;
    }

    @Override
    public boolean equals(Object o) {
        if (this == o) return true;
        if (!(o instanceof Ownership)) return false;
        return kind == ((Ownership) o).kind;
    }

    @Override
    public int hashCode() {
        return Objects.hash(kind);
    }

    @Override
    public String toString() {
        return "Ownership(" + kind + ")";
    }
}
