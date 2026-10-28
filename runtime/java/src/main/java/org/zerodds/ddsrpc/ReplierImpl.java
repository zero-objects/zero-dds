// SPDX-License-Identifier: Apache-2.0
package org.zerodds.ddsrpc;

import org.omg.dds.rpc.*;
import java.io.ByteArrayOutputStream;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;

/**
 * The {@link Replier} over a {@link DomainParticipant}.
 *
 * @param <TReq> the request topic type
 * @param <TRep> the reply topic type
 */
public class ReplierImpl<TReq, TRep> implements Replier<TReq, TRep> {

    private final ReplierParams params;
    /** The type supports of the params, typed: the params carry them for any type. */
    private final TypeSupport<TReq> requestSupport;
    private final TypeSupport<TRep> replySupport;
    private final ServiceMapping mapping;
    private final boolean gateOnMatching;
    private final RelatedIdentity relatedIdentity;
    private DataWriter writer;
    private DataReader reader;
    private RpcQos requestQos;
    private RpcQos replyQos;
    private final String instanceName;
    /** Requests taken from the reader and not yet taken by the application. */
    private final List<Sample<TReq>> held = new ArrayList<>();
    private boolean nondata = false;
    /** Requests taken from the reader whose client has not completed its matching yet. */
    private final List<TakenSample> heldBack = new ArrayList<>();
    /**
     * §7.6.2.2.1, Figure 7.3: a held request is handed on once its client is matched, and the
     * discovery that completes the match delivers no sample. A replier whose listener is called
     * on arrival (§7.9.2.2 way 1) asks again every {@link #RECHECK_MILLIS} while requests are held
     * back; a polled one (receiveRequests, waitForRequests, a Server) asks at every poll.
     */
    private static final long RECHECK_MILLIS = 5;
    private static final java.util.concurrent.ScheduledExecutorService RECHECK =
            java.util.concurrent.Executors.newSingleThreadScheduledExecutor(runnable -> {
                Thread thread = new Thread(runnable, "zerodds-rpc-client-matching");
                thread.setDaemon(true);
                return thread;
            });
    /** The pending recheck of the held requests, or {@code null}. Guarded by this. */
    private java.util.concurrent.ScheduledFuture<?> recheck;
    /** The runtime that made this replier, or {@code null} — see {@link #getEnvironment}. */
    private final RPCRuntime runtime;

    public ReplierImpl(ReplierParams params) {
        this(null, params);
    }

    /**
     * The constructor {@link RPCRuntime#createReplier} uses, so that
     * §7.11.2.2.2's {@code getEnvironment} has the runtime the
     * entity was derived from to return.
     *
     * @param runtime the runtime this replier is derived from
     * @param params the configuration of §7.11.1.4.12
     */
    @SuppressWarnings({"this-escape", "unchecked"})
    public ReplierImpl(RPCRuntime runtime, ReplierParams params) {
        this.runtime = runtime;
        this.params = params;
        this.requestSupport = (TypeSupport<TReq>) params.requestTypeSupport();
        this.replySupport = (TypeSupport<TRep>) params.replyTypeSupport();
        this.mapping = params.mapping();
        this.relatedIdentity = params.relatedIdentity();
        if (mapping == ServiceMapping.ENHANCED && relatedIdentity == RelatedIdentity.IF_SUPPORTED) {
            throw new IllegalArgumentException(
                    "the Enhanced mapping has no header, so a reply must carry "
                            + "PID_RELATED_SAMPLE_IDENTITY (§7.8.2)");
        }
        this.gateOnMatching = Matching.gates(params.matching(), params.mapping());
        // §7.11.2: the DataReaderQos of the request reader and the DataWriterQos of the reply
        // writer the params name are the entities' QoS as they stand.
        RpcQos request = params.getDataReaderQos() == null ? params.requestQos()
                : PsmEndpoint.rpcQosOf(params.getDataReaderQos(), params.requestQos());
        RpcQos reply = params.getDataWriterQos() == null ? params.replyQos()
                : PsmEndpoint.rpcQosOf(params.getDataWriterQos(), params.replyQos());
        this.requestQos = request.withDataRepresentation(params.wireEncoding());
        this.replyQos = reply.withDataRepresentation(params.wireEncoding());
        DomainParticipant participant = params.domainParticipant();
        if (participant != null) {
            // §7.9.1: activation creates the DataReader and the DataWriter.
            // §7.4.2.3: the names the run-time service name, the annotations
            // and the default give, in that order of precedence.
            TopicNames topics = TopicNames.of(params);
            this.reader =
                    participant.createReader(
                            topics.request(),
                            params.requestTypeName(),
                            requestQos,
                            params.instanceName(),
                            params.requestTypeDescription(),
                            PsmEndpoint.reader(
                                    TopicSample.request(requestSupport, mapping, params.wireEncoding()),
                                    params.getSubscriber(),
                                    params.getDataReaderQos()));
            this.writer =
                    participant.createWriter(
                            topics.reply(),
                            params.replyTypeName(),
                            replyQos,
                            params.instanceName(),
                            params.replyTypeDescription(),
                            PsmEndpoint.writer(
                                    TopicSample.reply(replySupport, mapping, params.wireEncoding()),
                                    params.getPublisher(),
                                    params.getDataWriterQos()));
            // §7.6.2.1.1/2: the service-side endpoints name each other as their related
            // endpoint, and announce the aliases of §7.4.3. A binding without discovery has
            // nothing to announce them in.
            this.reader.announce(topics.requestAliases(), this.writer.guid());
            this.writer.announce(topics.replyAliases(), this.reader.guid());
        }
        this.instanceName = params.instanceName();
        // §7.9.2.2 ways 1 and 2: the listener is called when a request has
        // arrived.
        if (reader != null
                && params.listener() != null
                && params.listenerDelivery() == ListenerDelivery.ON_ARRIVAL) {
            reader.setDataAvailableListener(this::onDataAvailable);
        }
    }

    private void onDataAvailable() {
        dispatchRequests();
    }

    @Override
    public synchronized List<Sample<TReq>> receiveRequests() {
        held.addAll(pull());
        List<Sample<TReq>> out = new ArrayList<>(held);
        held.clear();
        return out;
    }

    /**
     * §7.6.2.2.1, Figure 7.3: "Client Matching" is complete for a client when the reply writer
     * has discovered the client's reply reader, which the client's request writer names as its
     * related endpoint ({@code related_datareader_key}). The mask is per client: a request of
     * another client, already matched, is not held back by this one. A client whose request
     * writer announced no related endpoint does not take part in the extension, and the reply
     * writer having matched a reader is all there is to wait for, as it is for a binding that
     * sees no discovery data.
     */
    private boolean clientMatched(TakenSample taken) {
        if (writer == null || writer.matchedCount() <= 0) {
            return false;
        }
        List<RpcPeerInfo> requestWriters = reader.matchedPeers();
        List<RpcPeerInfo> replyReaders = writer.matchedPeers();
        SampleIdentity source = taken.identity();
        if (requestWriters == null || replyReaders == null || source.isUnknown()) {
            return true;
        }
        for (RpcPeerInfo requestWriter : requestWriters) {
            if (!Arrays.equals(requestWriter.guid(), source.writerGuid())) {
                continue;
            }
            byte[] related = requestWriter.relatedGuid();
            if (related == null) {
                return true;
            }
            for (RpcPeerInfo replyReader : replyReaders) {
                if (Arrays.equals(replyReader.guid(), related)) {
                    return true;
                }
            }
            return false;
        }
        // A writer the reader has not (yet) discovered cannot be matched.
        return false;
    }

    /** Starts the recheck of the held requests, for a replier nobody polls. Called holding this. */
    private void scheduleRecheck() {
        if (recheck != null || reader == null || params.listener() == null
                || params.listenerDelivery() != ListenerDelivery.ON_ARRIVAL) {
            return;
        }
        recheck = RECHECK.scheduleWithFixedDelay(this::recheckHeld, RECHECK_MILLIS, RECHECK_MILLIS,
                java.util.concurrent.TimeUnit.MILLISECONDS);
    }

    /** Hands on the held requests whose client is matched by now; ends when none is held. */
    private void recheckHeld() {
        synchronized (this) {
            if (reader == null || heldBack.isEmpty()) {
                if (recheck != null) {
                    recheck.cancel(false);
                    recheck = null;
                }
                return;
            }
            boolean ready = false;
            for (TakenSample taken : heldBack) {
                if (clientMatched(taken)) {
                    ready = true;
                    break;
                }
            }
            if (!ready) {
                return;
            }
        }
        dispatchRequests();
    }

    /** Takes what the reader has, decoded and filtered. */
    private List<Sample<TReq>> pull() {
        List<Sample<TReq>> out = new ArrayList<>();
        if (reader == null) {
            return out;
        }
        // §7.6.2.2: "The service-side DataReader (request DataReader) shall
        // reject requests from a client DataWriter (request DataWriter) as
        // long as the service-side DataWriter (reply DataWriter) has not
        // discovered and matched the corresponding client-side DataReader
        // (reply DataReader)." The requests are kept for the next call.
        List<TakenSample> incoming = new ArrayList<>(heldBack);
        heldBack.clear();
        incoming.addAll(reader.take());
        for (TakenSample taken : incoming) {
            if (!taken.validData()) {
                // §7.11.2 receiveNondataSamples: a sample without data is no request; the
                // client it came from may be gone, so the matching does not hold it back.
                if (nondata) {
                    out.add(new NondataSample<TReq>(taken.identity(), taken.instanceState()));
                }
                continue;
            }
            if (gateOnMatching && !clientMatched(taken)) {
                heldBack.add(taken);
                scheduleRecheck();
                continue;
            }
            if (mapping == ServiceMapping.BASIC
                    && requestSupport instanceof HeaderInRequestType) {
                // §7.3.1.4: the request type has its own `header` member.
                TReq value =
                        requestSupport
                                .deserialize(taken.octets(), new HeaderCodec.Cursor(0), params.wireEncoding());
                RequestHeader header = requestHeaderIn(requestSupport).requestHeader(value);
                if (!header.instanceName().equals(instanceName)) {
                    continue;
                }
                out.add(Sample.of(value, header.requestId(), SampleIdentity.UNKNOWN));
            } else if (mapping == ServiceMapping.BASIC) {
                HeaderCodec.Cursor cursor = new HeaderCodec.Cursor(0);
                RequestHeader header = HeaderCodec.decodeRequestHeader(taken.octets(), cursor);
                // §7.11.1.4.3: "Requests sent through a bound requester
                // reference shall be sent to the bound service instance
                // only." The empty name is the default instance and matches
                // only itself, so a named replier does not answer a request
                // that names no instance and an unnamed replier does not
                // answer one bound to an instance.
                if (!header.instanceName().equals(instanceName)) {
                    continue;
                }
                HeaderCodec.alignTo4(cursor);
                TReq value =
                        requestSupport
                                .deserialize(taken.octets(), cursor, params.wireEncoding());
                // §7.8.1: the request identity comes from the explicit
                // header, not from the transport's sample metadata, which a
                // binding such as the one over ZeroDDS does not fill.
                out.add(Sample.of(value, header.requestId(), SampleIdentity.UNKNOWN));
            } else {
                // §7.8.2.1: the identity of the request is the identity of
                // its sample.
                TReq value;
                try {
                    value =
                            requestSupport
                                    .deserialize(
                                            taken.octets(),
                                            new HeaderCodec.Cursor(0),
                                            params.wireEncoding());
                } catch (RemoteInvalidArgumentError malformed) {
                    // §7.5.2: a request that cannot be decoded (a @Choice with no member or
                    // with two, §7.5.1.2.1.1) is answered REMOTE_EX_INVALID_ARGUMENT, not
                    // dropped and not thrown at whoever called the dispatch.
                    sendReply(null, taken.identity(), RemoteExceptionCode.REMOTE_EX_INVALID_ARGUMENT);
                    continue;
                }
                out.add(Sample.of(value, taken.identity(), SampleIdentity.UNKNOWN));
            }
        }
        return out;
    }

    @Override
    public void sendReply(TRep reply, org.omg.dds.core.SampleIdentity relatedRequestId) {
        sendReply(reply, relatedRequestId, RemoteExceptionCode.REMOTE_EX_OK);
    }

    @Override
    @SuppressWarnings("unchecked")
    public synchronized void sendReply(
            TRep reply, org.omg.dds.core.SampleIdentity relatedRequestIdentity,
            RemoteExceptionCode code) {
        SampleIdentity relatedRequestId = SampleIdentity.of(relatedRequestIdentity);
        if (writer == null) {
            throw new RemoteOutOfResourcesError("the replier has no writer");
        }
        byte[] sample;
        if (mapping == ServiceMapping.BASIC && replySupport instanceof HeaderInReplyType) {
            // §7.3.1.4: the reply type has its own `header` member, which
            // is the first member of the sample.
            replyHeaderIn(replySupport)
                    .setReplyHeader(reply, new ReplyHeader(relatedRequestId, code));
            sample = replySupport.serialize(reply, 0, params.wireEncoding());
        } else if (mapping == ServiceMapping.BASIC) {
            ByteArrayOutputStream out = new ByteArrayOutputStream();
            byte[] header = HeaderCodec.encodeReplyHeader(new ReplyHeader(relatedRequestId, code));
            out.write(header, 0, header.length);
            HeaderCodec.padTo4(out);
            byte[] payload =
                    replySupport.serialize(reply, out.size(), params.wireEncoding());
            out.write(payload, 0, payload.length);
            sample = out.toByteArray();
        } else if (code == RemoteExceptionCode.REMOTE_EX_OK) {
            sample = replySupport.serialize(reply, 0, params.wireEncoding());
        } else if (replySupport instanceof EnhancedReplySupport) {
            // §7.5.1.2.5 rule 5: the code travels in `remoteEx`.
            sample =
                    replySupport
                            .serialize(
                                    ((EnhancedReplySupport<TRep>) replySupport)
                                            .remoteExceptionSample(code),
                                    0,
                                    params.wireEncoding());
        } else {
            throw new RemoteUnsupportedError(
                    "an Enhanced reply carries a remote exception code in its remoteEx member; "
                            + "the reply type support is not an EnhancedReplySupport");
        }
        // §7.8.2: "The PID_RELATED_SAMPLE_IDENTITY shall be used for data
        // samples sent as reply to a request."
        if (relatedIdentity == RelatedIdentity.REQUIRED || writer.carriesRelatedIdentity()) {
            writer.write(sample, relatedRequestId);
        } else {
            writer.write(sample);
        }
    }

    @Override
    @SuppressWarnings("unchecked")
    public synchronized int dispatchRequests() {
        ListenerBase listener = params.listener();
        if (listener instanceof ReplierListener) {
            ((ReplierListener<TReq, TRep>) listener).onRequestAvailable(this);
            return 1;
        }
        if (!(listener instanceof SimpleReplierListener)) {
            return 0;
        }
        SimpleReplierListener<TReq, TRep> simple = (SimpleReplierListener<TReq, TRep>) listener;
        int answered = 0;
        List<Sample<TReq>> withoutData = new ArrayList<>();
        for (Sample<TReq> request : receiveRequests()) {
            if (!request.validData()) {
                // No request to answer: it stays for the operations that take requests.
                withoutData.add(request);
                continue;
            }
            sendReply(simple.processRequest(request, request.identity()), request.identity());
            ++answered;
        }
        held.addAll(0, withoutData);
        return answered;
    }

    // ---- the operations of omg-dds-rpc-java.zip (DDS-RPC 1.0 §7.11.2) ----

    private synchronized int available() {
        held.addAll(pull());
        return held.size();
    }

    @Override
    public boolean waitForRequests(org.omg.dds.core.Duration maxWait) {
        return waitForRequests(1, maxWait);
    }

    @Override
    public boolean waitForRequests(int minCount, org.omg.dds.core.Duration maxWait) {
        return ServiceProxy.pollUntil(maxWait, () -> available() >= Math.max(minCount, 1));
    }

    @SuppressWarnings("unchecked")
    private static <T> SampleHolder<T> holderOf(Sample<T> request) {
        if (!(request instanceof SampleHolder)) {
            throw new IllegalArgumentException(
                    "the sample must be a SampleHolder, which the call fills in");
        }
        return (SampleHolder<T>) request;
    }

    @Override
    public boolean receiveRequest(Sample<TReq> request, org.omg.dds.core.Duration maxWait) {
        holderOf(request);
        waitForRequests(1, maxWait);
        return takeRequest(request);
    }

    @Override
    public Sample.Iterator<TReq> receiveRequests(org.omg.dds.core.Duration maxWait) {
        waitForRequests(1, maxWait);
        return takeRequests(Integer.MAX_VALUE);
    }

    @Override
    public Sample.Iterator<TReq> receiveRequests(
            int minRequestCount, int maxRequestCount, org.omg.dds.core.Duration maxWait) {
        waitForRequests(minRequestCount, maxWait);
        return takeRequests(maxRequestCount);
    }

    @Override
    public synchronized boolean takeRequest(Sample<TReq> request) {
        SampleHolder<TReq> holder = holderOf(request);
        if (available() == 0) {
            return false;
        }
        holder.fill(held.remove(0));
        return true;
    }

    @Override
    public synchronized Sample.Iterator<TReq> takeRequests(int maxSamples) {
        available();
        List<Sample<TReq>> out = new ArrayList<>();
        while (!held.isEmpty() && out.size() < maxSamples) {
            out.add(held.remove(0));
        }
        return Sample.iterate(out);
    }

    @Override
    public synchronized boolean readRequest(Sample<TReq> request) {
        SampleHolder<TReq> holder = holderOf(request);
        if (available() == 0) {
            return false;
        }
        holder.fill(held.get(0));
        return true;
    }

    @Override
    public synchronized Sample.Iterator<TReq> readRequests(int maxSamples) {
        available();
        return Sample.iterate(new ArrayList<>(held.subList(0, Math.min(maxSamples, held.size()))));
    }

    @Override
    public synchronized boolean receiveNondataSamples(boolean enable) {
        boolean previous = nondata;
        nondata = enable;
        return previous;
    }

    @Override
    public ReplierParams getReplierParams() {
        return params;
    }

    @Override
    @SuppressWarnings("unchecked")
    public synchronized org.omg.dds.sub.DataReader<TReq> getRequestDataReader() {
        if (reader == null) {
            throw new IllegalStateException("the replier was closed");
        }
        // The reader is typed by TReq when it is the PSM's: the class of the request support.
        return (org.omg.dds.sub.DataReader<TReq>) reader.psm();
    }

    @Override
    @SuppressWarnings("unchecked")
    public synchronized org.omg.dds.pub.DataWriter<TRep> getReplyDataWriter() {
        if (writer == null) {
            throw new IllegalStateException("the replier was closed");
        }
        return (org.omg.dds.pub.DataWriter<TRep>) writer.psm();
    }

    /** The support of a request type that has its own header, as the interface that says so. */
    @SuppressWarnings("unchecked")
    private HeaderInRequestType<TReq> requestHeaderIn(TypeSupport<TReq> support) {
        return (HeaderInRequestType<TReq>) support;
    }

    /** The support of a reply type that has its own header, as the interface that says so. */
    @SuppressWarnings("unchecked")
    private HeaderInReplyType<TRep> replyHeaderIn(TypeSupport<TRep> support) {
        return (HeaderInReplyType<TRep>) support;
    }

    @Override
    public synchronized void setQos(RpcQos requestedRequestQos, RpcQos requestedReplyQos) {
        RpcQos newRequestQos = requestedRequestQos.withDataRepresentation(params.wireEncoding());
        RpcQos newReplyQos = requestedReplyQos.withDataRepresentation(params.wireEncoding());
        if (writer == null || reader == null) {
            throw new RemoteOutOfResourcesError("the replier has no endpoints");
        }
        if (!requestQos.mutableChangeTo(newRequestQos) || !replyQos.mutableChangeTo(newReplyQos)) {
            throw new IllegalArgumentException(
                    "RELIABILITY, DURABILITY and HISTORY cannot change after the endpoints "
                            + "are enabled (DDS 1.4 §2.2.3)");
        }
        reader.setQos(newRequestQos);
        writer.setQos(newReplyQos);
        requestQos = newRequestQos;
        replyQos = newReplyQos;
    }

    @Override
    public synchronized void close() {
        if (recheck != null) {
            recheck.cancel(false);
            recheck = null;
        }
        // §7.9.3: deactivation deletes the underlying DDS entities.
        DomainParticipant participant = params.domainParticipant();
        if (reader != null
                && params.listener() != null
                && params.listenerDelivery() == ListenerDelivery.ON_ARRIVAL) {
            reader.setDataAvailableListener(null);
        }
        if (participant != null) {
            if (writer != null) {
                participant.deleteWriter(writer);
            }
            if (reader != null) {
                participant.deleteReader(reader);
            }
        }
        writer = null;
        reader = null;
    }

    @Override
    public synchronized boolean isNull() {
        return writer == null && reader == null;
    }

    /**
     * §7.11.2.2.2 through [DDS-Java-PSM] §7.3.1: the environment this
     * replier was derived from, which §7.11.2.2.3 makes the
     * {@link RPCRuntime} that created it.
     *
     * @return that runtime.
     * @throws IllegalStateException when the replier was constructed
     *     without one.
     */
    @Override
    public org.omg.dds.core.ServiceEnvironment getEnvironment() {
        if (runtime == null) {
            return Replier.super.getEnvironment();
        }
        return runtime;
    }
}
