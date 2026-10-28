// SPDX-License-Identifier: Apache-2.0
package org.omg.dds.rpc;

import java.util.List;

/**
 * DDS-RPC 1.0 §7.11.1.4.3 through §7.11.2.2.1: "A requester sends
 * requests and receives replies. […] Requester is inherently
 * asynchronous as sending a request and receiving its corresponding
 * reply (or replies) are separated. Requester allows listener-based,
 * polling-based, and future-based reception of replies."
 *
 * <p>An interface, as §7.11.2.2.1 lists it; {@link org.zerodds.ddsrpc.RequesterImpl} is the
 * implementation and {@link RPCRuntime#createRequester} makes one.
 *
 * @param <TReq> the request topic type
 * @param <TRep> the reply topic type
 */
public interface Requester<TReq, TRep> extends ServiceProxy {

    /**
     * Send one request and return its identity: the writer GUID and
     * sequence number the sample has, or will have when the request is
     * queued until the service is matched (§7.6.2.2). The type is the one of
     * the machine-readable {@code Requester.java}; the object is an
     * {@link SampleIdentity org.omg.dds.rpc.SampleIdentity}.
     */
    org.omg.dds.core.SampleIdentity sendRequest(TReq request);

    /**
     * Poll for replies — §7.11.1.4.3: "Requester provides functions to
     * allow polling reception of replies."
     *
     * @throws RuntimeException the failure of a reply that carried a remote
     *     exception code or of a request that could not be written, if one
     *     is waiting; the replies taken with it stay and are returned by
     *     the next call
     */
    List<Sample<TRep>> receiveReplies();

    /** An iterator over the replies that have arrived — §7.11.2.2.7. */
    Sample.Iterator<TRep> replyIterator();

    /**
     * The reply to one request, by the correlation of §7.8. Every other
     * reply taken in the meantime is kept (§7.2.3).
     *
     * @return the reply, or {@code null} when it has not arrived
     * @throws RuntimeException the failure recorded for this request, such
     *     as {@link RemoteUnknownOperationError} for a reply with
     *     {@code REMOTE_EX_UNKNOWN_OPERATION} (§7.11.2.1)
     */
    Sample<TRep> receiveReply(org.omg.dds.core.SampleIdentity requestId);

    /**
     * The future-based reception of §7.11.1.4.3: "no request-reply
     * correlation is necessary because every future represents a reply
     * to a unique request." The type is the {@code java.util.concurrent.Future}
     * of the machine-readable {@code Requester.java}; the object is a
     * {@link Future org.omg.dds.rpc.Future}, which takes a completion listener.
     */
    java.util.concurrent.Future<Sample<TRep>> sendRequestAsync(TReq request);

    /**
     * Hand every reply that has arrived to the listener the params
     * carried. A binding whose readers notify on arrival
     * ({@link DataReader#setDataAvailableListener}) calls this itself;
     * calling it by hand is for a pump loop.
     *
     * @return how many replies were handed over
     */
    int dispatchReplies();

    /**
     * The writer of the request topic, the entity the requester sends with.
     *
     * @throws IllegalStateException when the requester was closed
     */
    DataWriter requestWriter();

    /**
     * The reader of the reply topic, the entity the requester receives with.
     *
     * @throws IllegalStateException when the requester was closed
     */
    DataReader replyReader();

    /**
     * Change the mutable QoS policies of the request writer and the reply
     * reader at run time — §7.10.2.
     *
     * @param requestQos the QoS of the request topic's writer
     * @param replyQos the QoS of the reply topic's reader
     * @throws IllegalArgumentException when an immutable policy would change
     */
    void setQos(RpcQos requestQos, RpcQos replyQos);

    // ---- the operations of omg-dds-rpc-java.zip (DDS-RPC 1.0 §7.11.2) ----

    /**
     * Sends a request that expects no reply: the reply the service sends anyway (the Return
     * union has a case for every operation, §7.5.1.1.7 rule 5) is dropped when it arrives.
     */
    void sendRequestOneway(TReq request);

    /**
     * As {@link #sendRequestAsync(Object)}; the completion listener of the future is called on
     * {@code executor}.
     */
    java.util.concurrent.Future<Sample<TRep>> sendRequestAsync(
            TReq request, java.util.concurrent.ExecutorService executor);

    /** Waits up to {@code timeout} for a reply and takes it; {@code null} when none came. */
    Sample<TRep> receiveReply(org.omg.dds.core.Duration timeout);

    /** Waits up to {@code maxWait} for a reply and takes every reply that has arrived. */
    Sample.Iterator<TRep> receiveReplies(org.omg.dds.core.Duration maxWait);

    /**
     * Waits up to {@code maxWait} for at least {@code minCount} replies and takes up to
     * {@code maxCount}.
     */
    Sample.Iterator<TRep> receiveReplies(
            int minCount, int maxCount, org.omg.dds.core.Duration maxWait);

    /** Waits until {@code minCount} replies have arrived; {@code false} when {@code maxWait} ran out. */
    boolean waitForReplies(int minCount, org.omg.dds.core.Duration maxWait);

    /** Waits until a reply has arrived. */
    boolean waitForReplies(org.omg.dds.core.Duration maxWait);

    /** Waits until {@code minCount} replies to the request {@code relatedRequestId} have arrived. */
    boolean waitForReplies(
            int minCount, org.omg.dds.core.Duration maxWait, org.omg.dds.core.SampleIdentity relatedRequestId);

    /** Takes the oldest reply that has arrived, without waiting; {@code null} when none. */
    Sample<TRep> takeReply();

    /** Takes the reply to {@code relatedRequestId}, without waiting; {@code null} when none. */
    Sample<TRep> takeReply(org.omg.dds.core.SampleIdentity relatedRequestId);

    /** Takes up to {@code maxCount} replies, without waiting. */
    Sample.Iterator<TRep> takeReplies(int maxCount);

    /** Takes up to {@code maxCount} replies to {@code relatedRequestId}, without waiting. */
    Sample.Iterator<TRep> takeReplies(int maxCount, org.omg.dds.core.SampleIdentity relatedRequestId);

    /** Takes every reply to {@code relatedRequestId}, without waiting. */
    Sample.Iterator<TRep> takeReplies(org.omg.dds.core.SampleIdentity relatedRequestId);

    /** Reads the oldest reply that has arrived and leaves it; {@code null} when none. */
    Sample<TRep> readReply();

    /** Reads the reply to {@code relatedRequestId} and leaves it; {@code null} when none. */
    Sample<TRep> readReply(org.omg.dds.core.SampleIdentity relatedRequestId);

    /** Reads up to {@code maxCount} replies and leaves them. */
    Sample.Iterator<TRep> readReplies(int maxCount);

    /**
     * Reads the first of up to {@code maxCount} replies to {@code relatedRequestId}, as the
     * Java file of the specification declares it (a single {@code Sample}).
     */
    Sample<TRep> readReplies(int maxCount, org.omg.dds.core.SampleIdentity relatedRequestId);

    /** Reads the reply to {@code relatedRequestId}, as the Java file declares it. */
    Sample<TRep> readReplies(org.omg.dds.core.SampleIdentity relatedRequestId);

    /**
     * Whether the samples of the reply topic that carry no data are handed over: the
     * samples whose {@code valid_data} is false (DDS 1.4 §2.2.2.5.1), which only tell of a
     * change of the instance — a disposed instance, or one whose writers went away
     * (NOT_ALIVE_NO_WRITERS). Off by default: the requester then hands over replies only.
     * On, {@link #receiveReplies()}, {@code takeReply()}, {@code takeReplies(int)},
     * {@code readReply()}, {@code readReplies(int)}, the {@code receiveReplies} and
     * {@code waitForReplies} that name no request, and the listener hand them over too, as
     * samples whose {@code validData()} is false and whose {@code getData()} is {@code null},
     * with the state of the instance; they answer no request, so an operation that names one
     * does not return them.
     *
     * @return the setting this one replaces
     */
    boolean receiveNondataSamples(boolean enable);

    /** The parameters this requester was made with. */
    RequesterParams getRequesterParams();

    /**
     * The request DataWriter of the DDS Java PSM — the entity {@link #requestWriter()} sends
     * with, typed by the request type: a sample written to it is a request as the topic carries
     * it (§7.5.1.1.6), header and all. {@code null} when the participant is not one of the PSM
     * ({@link RequesterParams#getDomainParticipant()} is {@code null}) or the request type
     * support names no class (see {@code org.zerodds.ddsrpc.TopicSample}).
     */
    org.omg.dds.pub.DataWriter<TReq> getRequestDataWriter();

    /** The reply DataReader of the DDS Java PSM; see {@link #getRequestDataWriter()}. */
    org.omg.dds.sub.DataReader<TRep> getReplyDataReader();
}
