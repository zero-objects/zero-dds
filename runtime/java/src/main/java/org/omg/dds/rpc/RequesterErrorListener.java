// SPDX-License-Identifier: Apache-2.0
package org.omg.dds.rpc;

/**
 * Receives the replies that failed: a remote exception code other than
 * {@code REMOTE_EX_OK} (§7.5.2), or a request that could not be written.
 *
 * <p>DDS-RPC 1.0 §7.11.1.4.9 and §7.11.1.4.10 give the listeners a callback
 * for a reply that arrived and none for one that is an error. A reply
 * delivered from the middleware's own thread cannot throw to the caller
 * that asked, so a requester listener that wants these implements this
 * interface too. Without it the failure stays with the requester and
 * {@link Requester#receiveReply} raises it.
 */
public interface RequesterErrorListener extends ListenerBase {

    /**
     * @param request the request that failed
     * @param failure what the call site would have been thrown, for
     *     example {@link RemoteUnknownOperationError}
     */
    void onReplyFailure(SampleIdentity request, RuntimeException failure);
}
