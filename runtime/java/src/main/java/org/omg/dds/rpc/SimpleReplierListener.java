// SPDX-License-Identifier: Apache-2.0
package org.omg.dds.rpc;

/**
 * DDS-RPC 1.0 §7.11.1.4.7: "SimpleReplierListener&lt;TReq, TRep&gt; is used
 * to provide a synchronous request listener for a Replier. […] the
 * callback provides the request object and expects the reply as the
 * return value."
 *
 * @param <TReq> the request topic type
 * @param <TRep> the reply topic type
 */
public interface SimpleReplierListener<TReq, TRep> extends ListenerBase {

    /**
     * The operation as {@code omg-dds-rpc-java.zip} declares it: the request and its identity.
     * The runtime calls this one; by default it calls {@link #processRequest(Sample)}, so a
     * listener implements either.
     */
    default TRep processRequest(Sample<TReq> request, org.omg.dds.core.SampleIdentity identity) {
        return processRequest(request);
    }

    /** The request alone: the earlier form of the operation above. */
    default TRep processRequest(Sample<TReq> request) {
        throw new UnsupportedOperationException(
                "a SimpleReplierListener implements processRequest");
    }
}
