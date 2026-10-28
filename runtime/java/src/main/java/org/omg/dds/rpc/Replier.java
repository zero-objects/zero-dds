// SPDX-License-Identifier: Apache-2.0
package org.omg.dds.rpc;

import java.util.List;

/**
 * DDS-RPC 1.0 §7.11.1.4.5 through §7.11.2.2.1: "A replier receives
 * requests and send[s] replies. […] Replier allows listener-based and
 * polling-based reception of requests."
 *
 * <p>An interface, as §7.11.2.2.1 lists it; {@link org.zerodds.ddsrpc.ReplierImpl} is the
 * implementation and {@link RPCRuntime#createReplier} makes one.
 *
 * @param <TReq> the request topic type
 * @param <TRep> the reply topic type
 */
public interface Replier<TReq, TRep> extends RPCEntity {

    /**
     * Poll for requests — §7.9.2.2 way 3. The identity of each request
     * is {@link Sample#identity()}, which the reply is sent against
     * (§7.8.2.1).
     */
    List<Sample<TReq>> receiveRequests();

    /** Send one reply, correlated to the request it answers — §7.8.2.3. */
    void sendReply(TRep reply, org.omg.dds.core.SampleIdentity relatedRequestId);

    /** Send one reply carrying a remote exception code — §7.5.2. */
    void sendReply(TRep reply, org.omg.dds.core.SampleIdentity relatedRequestId, RemoteExceptionCode code);

    /**
     * Take every request that has arrived and answer it through the
     * listener the params carried — §7.11.1.4.7 and §7.11.1.4.8. A binding
     * whose readers notify on arrival calls this itself.
     *
     * @return how many requests were answered
     */
    int dispatchRequests();

    /**
     * Change the mutable QoS policies of the request reader and the reply
     * writer at run time — §7.10.2.
     *
     * @throws IllegalArgumentException when an immutable policy would change
     */
    void setQos(RpcQos requestQos, RpcQos replyQos);

    // ---- the operations of omg-dds-rpc-java.zip (DDS-RPC 1.0 §7.11.2) ----

    /**
     * Waits up to {@code maxWait} for a request and takes it into {@code request}.
     *
     * @param request a {@link SampleHolder}, which the call fills
     * @return {@code false} when no request came in time
     * @throws IllegalArgumentException when {@code request} is not a {@link SampleHolder}
     */
    boolean receiveRequest(Sample<TReq> request, org.omg.dds.core.Duration maxWait);

    /** Waits up to {@code maxWait} for a request and takes every request that has arrived. */
    Sample.Iterator<TReq> receiveRequests(org.omg.dds.core.Duration maxWait);

    /**
     * Waits up to {@code maxWait} for at least {@code minRequestCount} requests and takes up
     * to {@code maxRequestCount}.
     */
    Sample.Iterator<TReq> receiveRequests(
            int minRequestCount, int maxRequestCount, org.omg.dds.core.Duration maxWait);

    /** Waits until a request has arrived; {@code false} when {@code maxWait} ran out. */
    boolean waitForRequests(org.omg.dds.core.Duration maxWait);

    /** Waits until {@code minCount} requests have arrived. */
    boolean waitForRequests(int minCount, org.omg.dds.core.Duration maxWait);

    /** Takes the oldest request into {@code request}, without waiting. */
    boolean takeRequest(Sample<TReq> request);

    /** Takes up to {@code maxSamples} requests, without waiting. */
    Sample.Iterator<TReq> takeRequests(int maxSamples);

    /** Reads the oldest request into {@code request} and leaves it. */
    boolean readRequest(Sample<TReq> request);

    /** Reads up to {@code maxSamples} requests and leaves them. */
    Sample.Iterator<TReq> readRequests(int maxSamples);

    /**
     * Whether the samples of the request topic that carry no data are handed over: the
     * samples whose {@code valid_data} is false (DDS 1.4 §2.2.2.5.1), such as the end of
     * the instance a requester whose request writer was deleted leaves. Off by
     * default. On, the operations that receive, take or read requests hand them over too, as
     * samples whose {@code validData()} is false and whose {@code getData()} is {@code null},
     * with the state of the instance. A {@link SimpleReplierListener} is not called for them,
     * as there is no request to answer; they stay for those operations.
     *
     * @return the setting this one replaces
     */
    boolean receiveNondataSamples(boolean enable);

    /** The parameters this replier was made with. */
    ReplierParams getReplierParams();

    /**
     * The request DataReader of the DDS Java PSM — the entity the replier receives with, typed
     * by the request type: a sample taken from it is a request as the topic carries it
     * (§7.5.1.1.6), and is no longer there for the replier. {@code null} when the participant
     * is not one of the PSM or the request type support names no class (see
     * {@code org.zerodds.ddsrpc.TopicSample}).
     */
    org.omg.dds.sub.DataReader<TReq> getRequestDataReader();

    /** The reply DataWriter of the DDS Java PSM; see {@link #getRequestDataReader()}. */
    org.omg.dds.pub.DataWriter<TRep> getReplyDataWriter();
}
