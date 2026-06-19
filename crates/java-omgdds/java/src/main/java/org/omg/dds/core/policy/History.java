// SPDX-License-Identifier: Apache-2.0
package org.omg.dds.core.policy;

import java.util.Objects;

/**
 * OMG DDS Java-PSM HistoryQosPolicy — Spec §7.2.4.
 */
public final class History {
    public enum Kind {
        KEEP_LAST,
        KEEP_ALL,
    }

    public static final History KEEP_LAST_1 = new History(Kind.KEEP_LAST, 1);
    public static final History KEEP_LAST_10 = new History(Kind.KEEP_LAST, 10);
    public static final History KEEP_ALL = new History(Kind.KEEP_ALL, 0);

    private final Kind kind;
    private final int depth;

    public History(Kind kind, int depth) {
        this.kind = Objects.requireNonNull(kind);
        if (depth < 0) {
            throw new IllegalArgumentException("depth must be >= 0");
        }
        this.depth = depth;
    }

    public Kind kind() {
        return kind;
    }

    public int depth() {
        return depth;
    }

    @Override
    public boolean equals(Object o) {
        if (this == o) return true;
        if (!(o instanceof History)) return false;
        History other = (History) o;
        return kind == other.kind && depth == other.depth;
    }

    @Override
    public int hashCode() {
        return Objects.hash(kind, depth);
    }
}
