// SPDX-License-Identifier: Apache-2.0
package org.omg.dds.core;

/**
 * OMG DDS Java-PSM Entity — Spec §7.2.4.
 *
 * <p>Base interface for all DDS entities. Provides enable/disable +
 * lifecycle close.
 */
public interface Entity extends AutoCloseable {
    InstanceHandle getInstanceHandle();

    ReturnCode enable();

    /** Spec §7.2.3.3 — Auto-Close via Cleaner-based mechanism. */
    @Override
    void close();

    boolean isClosed();
}
