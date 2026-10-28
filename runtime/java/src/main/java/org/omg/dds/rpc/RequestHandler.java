// SPDX-License-Identifier: Apache-2.0
package org.omg.dds.rpc;

/**
 * The part of a function-call service a generator writes: unpack a request,
 * call the implementation, pack the reply — DDS-RPC 1.0 §7.9.2.1:
 * "Concrete implementations of the interface operations shall be invoked as
 * a consequence of receiving requests. The operations must return a reply
 * consistent with the language binding."
 *
 * @param <TReq> the request topic type
 * @param <TRep> the reply topic type
 */
public interface RequestHandler<TReq, TRep> {

    /**
     * Answers one request.
     *
     * @param request the request
     * @return the reply, or {@code null} for an operation that has none
     *     (a {@code oneway} operation)
     */
    Response<TRep> handle(TReq request);

    /**
     * The sample the reply travels with when {@link #handle} failed with
     * anything but a declared exception: a reply that carries no result,
     * whose header says {@code REMOTE_EX_UNKNOWN_EXCEPTION} (§7.5.2).
     */
    TRep failureReply(TReq request);
}
