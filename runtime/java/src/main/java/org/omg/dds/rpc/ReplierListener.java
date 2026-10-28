// SPDX-License-Identifier: Apache-2.0
package org.omg.dds.rpc;

/**
 * DDS-RPC 1.0 §7.11.1.4.8: "ReplierListener&lt;TReq, TRep&gt; interface is
 * used to provide an asynchronous request listener for a Replier. […]
 * the callback provides the Replier object and returns void."
 *
 * @param <TReq> the request topic type
 * @param <TRep> the reply topic type
 */
public interface ReplierListener<TReq, TRep> extends ListenerBase {

    void onRequestAvailable(Replier<TReq, TRep> replier);
}
