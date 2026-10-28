// SPDX-License-Identifier: Apache-2.0
package org.omg.dds.rpc;

/**
 * The state of a service as the Java binding's {@code ServiceStatus} lists it
 * (DDS-RPC 1.0 §7.11.2, {@code omg-dds-rpc-java.zip}).
 */
public enum ServiceStatus {
    /** The service was closed: its DDS entities are deleted. */
    CLOSED,
    /** The service is paused: requests stay in the reader (§7.11.1.5.2). */
    PAUSED,
    /** The service dispatches requests. */
    RUNNING
}
