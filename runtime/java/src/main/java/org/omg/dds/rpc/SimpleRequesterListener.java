// SPDX-License-Identifier: Apache-2.0
package org.omg.dds.rpc;

/**
 * DDS-RPC 1.0 §7.11.1.4.9: "SimpleRequesterListener&lt;TRep&gt; abstract
 * class is used to receive notifications of arrival of replies. […] the
 * callback provides access to the reply sample and returns void."
 *
 * @param <TRep> the reply topic type
 */
public interface SimpleRequesterListener<TRep> extends ListenerBase {

    /**
     * The operation as {@code omg-dds-rpc-java.zip} declares it: the reply and the identity of
     * the request it answers. The runtime calls this one; by default it calls
     * {@link #processReply(Sample)}, so a listener implements either.
     */
    default void process_reply(Sample<TRep> sample, org.omg.dds.core.SampleIdentity identity) {
        processReply(sample);
    }

    /** The reply alone: the earlier form of {@link #process_reply}. */
    default void processReply(Sample<TRep> reply) {}
}
