// SPDX-License-Identifier: Apache-2.0
package org.omg.dds.rpc;

/**
 * When a requester's or replier's listener is called.
 *
 * <p>DDS-RPC 1.0 §7.9.2.2 and §7.11.1.4.7 to §7.11.1.4.10: the callbacks
 * report a request or a reply that "is available", which is when the sample
 * arrives.
 */
public enum ListenerDelivery {
    /**
     * The clauses as written: the endpoint registers with its DataReader
     * and the listener is called from the middleware's thread when a sample
     * arrives. A DataReader binding that cannot notify
     * ({@link DataReader#setDataAvailableListener}) makes the endpoint's
     * construction fail with {@link RemoteUnsupportedError}.
     */
    ON_ARRIVAL,
    /**
     * The listener is called only when the application calls
     * {@link Requester#dispatchReplies} or {@link Replier#dispatchRequests}
     * ({@link Server#runOnce} does for a service). For a binding that does
     * not notify.
     */
    ON_PUMP
}
