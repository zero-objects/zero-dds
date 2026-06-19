// DDS-RPC 1.0 §7.11.2 / §7.9 — generic Requester interface.
//
// Skeleton-only: declares the API surface used by generated
// `<Service>Requester.java` classes. A pure-Java transport binding is
// expected to provide an implementation; this crate only publishes the
// contract.
package org.zerodds.rpc;

import java.util.concurrent.CompletableFuture;

/**
 * Client-side endpoint for a DDS-RPC service. Generated Requester
 * classes hold a {@code Requester<Object,Object>} and call
 * {@link #sendRequest(Object)} / {@link #sendOneway(Object)} after
 * marshalling per-method argument tuples.
 *
 * @param <TIn>  input payload type ({@code <Service>_<Method>_In} or
 *               {@code Object} for type-erased generated stubs)
 * @param <TOut> output payload type
 */
public interface Requester<TIn, TOut> extends AutoCloseable {
    /**
     * Sends a request and returns a future that completes when the reply
     * arrives or the request times out.
     */
    CompletableFuture<TOut> sendRequest(TIn payload);

    /** Sends a fire-and-forget oneway request — no reply expected. */
    void sendOneway(TIn payload);

    /** Releases native resources. */
    @Override
    void close();
}
