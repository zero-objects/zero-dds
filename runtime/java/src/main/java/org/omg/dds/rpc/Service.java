// SPDX-License-Identifier: Apache-2.0
package org.omg.dds.rpc;

/**
 * DDS-RPC 1.0 §7.11.1.5.1 through §7.11.2.3: "Service is a reference type
 * and accepts a reference to the service implementation. A Service
 * instantiates the underlying DDS entities and makes the service
 * discoverable. Every Service inherits from ServiceEndpoint. A Service
 * may belong to only one Server."
 */
public interface Service extends ServiceEndpoint {

    /**
     * Take the requests that have arrived and answer them. Returns how
     * many were answered, so a {@link Server} can report progress.
     */
    int dispatch();

    /** The name the service is discoverable under — §7.4.1. */
    String serviceName();
}
