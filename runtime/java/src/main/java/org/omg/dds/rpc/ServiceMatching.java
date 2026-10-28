// SPDX-License-Identifier: Apache-2.0
package org.omg.dds.rpc;

/**
 * Whether a requester holds its requests back until the service is
 * matched, and a replier its replies until the client is.
 *
 * <p>DDS-RPC 1.0 §7.6.2.2 for the Enhanced Service Mapping: "The
 * client-side DataWriter (request DataWriter) shall not send out any
 * request to a service DataReader unless the client-side DataReader (reply
 * DataReader) has discovered and matched the corresponding service
 * DataWriter (reply DataWriter)", and "the service-side DataReader
 * (request DataReader) shall reject requests from a client DataWriter
 * (request DataWriter) as long as the service-side DataWriter (reply
 * DataWriter) has not discovered and matched the corresponding client-side
 * DataReader". Figure 7.4 adds that "attempts to send requests would be
 * queued". §7.6.1 says the Basic Service Mapping "does not use any
 * extensions to the DDS discovery built-in topics" and lists the race
 * that results as a known weakness.
 */
public enum ServiceMatching {
    /**
     * The clauses as written: the Enhanced mapping queues and rejects
     * until matched, the Basic mapping uses plain DDS matching only.
     */
    PER_MAPPING,
    /** Queue requests and reject them until matched, in either mapping. */
    UNTIL_MATCHED,
    /** Never wait for the other side, in either mapping. */
    IMMEDIATE
}
