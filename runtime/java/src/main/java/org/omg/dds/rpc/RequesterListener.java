// SPDX-License-Identifier: Apache-2.0
package org.omg.dds.rpc;

/**
 * DDS-RPC 1.0 §7.11.1.4.10: "RequesterListener&lt;TReq,TRep&gt; interface
 * is used to receive notification of reply arrival. […] the callback
 * provides the replier object and returns void."
 *
 * @param <TReq> the request topic type
 * @param <TRep> the reply topic type
 */
public interface RequesterListener<TReq, TRep> extends ListenerBase {

    void onReplyAvailable(Requester<TReq, TRep> requester);
}
