// SPDX-License-Identifier: Apache-2.0
package org.omg.dds.core.policy;

/**
 * OMG DDS Java-PSM DurabilityQosPolicy — Spec §7.2.4.
 */
public enum Durability {
    VOLATILE,
    TRANSIENT_LOCAL,
    TRANSIENT,
    PERSISTENT,
}
