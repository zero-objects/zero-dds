// SPDX-License-Identifier: Apache-2.0
package org.omg.dds.rpc;

import java.util.function.BooleanSupplier;
import org.omg.dds.core.Duration;

/**
 * DDS-RPC 1.0 §7.11.1.5.3 through §7.11.2.3: "A Server is a container of
 * one or more Services. Server.run() function begins dispatching the
 * requests to the Service implementation. Server is a reference type."
 *
 * <p>An interface, as the Java binding lists its entities;
 * {@link #create()} and {@link RPCRuntime#createServer()} make the
 * implementation.
 */
public interface Server extends RPCEntity, Runnable {

    /** A server with no service. */
    static Server create() {
        return new org.zerodds.ddsrpc.ServerImpl(null);
    }

    /**
     * Add one service. §7.11.1.5.1: "A Service may belong to only one
     * Server."
     *
     * @throws IllegalStateException when the service already belongs to a
     *     server
     */
    void add(Service service);

    int size();

    /**
     * Dispatch one round over every service that is not paused. Returns
     * how many requests were answered.
     */
    int runOnce();

    /**
     * Dispatch until {@code until} says to stop — §7.11.1.5.3's
     * {@code run()}, with the stop condition a parameter rather than a
     * signal handler: a library that installed one would take it from
     * the application.
     */
    void run(BooleanSupplier until);

    /**
     * Dispatch the requests of every service until {@link #close()} is
     * called or the calling thread is interrupted — the blocking
     * {@code run()} of the machine-readable {@code Server.java}
     * (§7.11.2.3: "begins dispatching the requests to the Service
     * implementation").
     */
    @Override
    void run();

    /**
     * Dispatch for at most {@code maxWait}: returns when a round answered
     * requests, when {@code maxWait} has elapsed, or when the server was
     * closed — the non-blocking {@code run(Duration)} of the
     * machine-readable {@code Server.java}.
     *
     * @param maxWait the longest the call waits for a request
     */
    void run(Duration maxWait);

    /**
     * The params the services of this server are created with unless the
     * caller sets others: bound to the participant of the runtime that made
     * the server, with the defaults of §7.10.2. Each call returns a new
     * object; changing it changes no service.
     */
    ServiceParams getDefaultServiceParams();

    /** Close every service and forget them. */
    @Override
    void close();

    /** Whether the server was closed. */
    @Override
    boolean isNull();
}
