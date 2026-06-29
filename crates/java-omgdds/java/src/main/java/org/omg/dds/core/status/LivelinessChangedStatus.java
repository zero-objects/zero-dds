// SPDX-License-Identifier: Apache-2.0
package org.omg.dds.core.status;

/**
 * OMG DDS LivelinessChangedStatus — DDS-DCPS 1.4 §2.2.4.1.
 *
 * <p>Tracks the number of matched DataWriters that are currently asserting
 * liveliness ({@code aliveCount}) vs. those that have lost it
 * ({@code notAliveCount}), plus the change in each since the status was last
 * read.
 */
public final class LivelinessChangedStatus {
    private final int aliveCount;
    private final int notAliveCount;
    private final int aliveCountChange;
    private final int notAliveCountChange;

    public LivelinessChangedStatus(int aliveCount, int notAliveCount,
                                   int aliveCountChange, int notAliveCountChange) {
        this.aliveCount = aliveCount;
        this.notAliveCount = notAliveCount;
        this.aliveCountChange = aliveCountChange;
        this.notAliveCountChange = notAliveCountChange;
    }

    public int aliveCount() {
        return aliveCount;
    }

    public int notAliveCount() {
        return notAliveCount;
    }

    public int aliveCountChange() {
        return aliveCountChange;
    }

    public int notAliveCountChange() {
        return notAliveCountChange;
    }

    @Override
    public String toString() {
        return "LivelinessChangedStatus(alive=" + aliveCount
                + ", notAlive=" + notAliveCount
                + ", aliveChange=" + aliveCountChange
                + ", notAliveChange=" + notAliveCountChange + ")";
    }
}
