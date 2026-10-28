// SPDX-License-Identifier: Apache-2.0
package org.zerodds.core;

import java.util.Set;

import org.omg.dds.core.status.Status;

/**
 * The view a {@link org.omg.dds.core.StatusCondition} has on its entity: the
 * communication statuses whose value changed since the application last read
 * them (DDS-Java-PSM 1.0 §7.3.7.3, DDS 1.2 §7.1.4.1).
 */
public interface StatusTrigger {

    /** The triggered statuses, without clearing them. */
    Set<Class<? extends Status>> triggeredStatuses();

    /** All communication statuses of DDS 1.2 §7.1.4.1 — the default list. */
    Set<Class<? extends Status>> allStatuses();
}