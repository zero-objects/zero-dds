// SPDX-License-Identifier: Apache-2.0
package org.omg.dds.core.status;

import org.omg.dds.core.InstanceHandle;

/**
 * OMG DDS RequestedDeadlineMissedStatus — DDS-DCPS 1.4 §2.2.4.1.
 *
 * <p>Raised on a DataReader when the requested {@code DEADLINE} period
 * elapses for an instance without a new sample arriving.
 *
 * <ul>
 *   <li>{@code totalCount} — cumulative number of missed deadlines.</li>
 *   <li>{@code totalCountChange} — increment since the status was last read.</li>
 *   <li>{@code lastInstanceHandle} — the instance whose deadline was last
 *       missed.</li>
 * </ul>
 */
public final class RequestedDeadlineMissedStatus {
    private final int totalCount;
    private final int totalCountChange;
    private final InstanceHandle lastInstanceHandle;

    public RequestedDeadlineMissedStatus(int totalCount, int totalCountChange,
                                         InstanceHandle lastInstanceHandle) {
        this.totalCount = totalCount;
        this.totalCountChange = totalCountChange;
        this.lastInstanceHandle = lastInstanceHandle;
    }

    public int totalCount() {
        return totalCount;
    }

    public int totalCountChange() {
        return totalCountChange;
    }

    public InstanceHandle lastInstanceHandle() {
        return lastInstanceHandle;
    }

    @Override
    public String toString() {
        return "RequestedDeadlineMissedStatus(total=" + totalCount
                + ", change=" + totalCountChange + ")";
    }
}
