// SPDX-License-Identifier: Apache-2.0
package org.zerodds.ddsrpc;

import org.omg.dds.rpc.*;
import java.io.ByteArrayOutputStream;
import java.util.ArrayDeque;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Deque;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.concurrent.CompletableFuture;

/**
 * The {@link Requester} over a {@link DomainParticipant}.
 *
 * <p>A generated {@code <I>Requester} extends this with the two type
 * arguments fixed, as the C++ column says with a {@code using}.
 *
 * @param <TReq> the request topic type
 * @param <TRep> the reply topic type
 */
public class RequesterImpl<TReq, TRep> implements Requester<TReq, TRep> {

    /** One request that has been built and not yet written. */
    private static final class Queued {
        final byte[] sample;
        final SampleIdentity predicted;
        final String instance;

        Queued(byte[] sample, SampleIdentity predicted, String instance) {
            this.sample = sample;
            this.predicted = predicted;
            this.instance = instance;
        }
    }

    private final RequesterParams params;
    /** The type supports of the params, typed: the params carry them for any type. */
    private final TypeSupport<TReq> requestSupport;
    private final TypeSupport<TRep> replySupport;
    private final ServiceMapping mapping;
    private final boolean gateOnMatching;
    private DataWriter writer;
    private DataReader reader;
    private RpcQos requestQos;
    private RpcQos replyQos;
    private final List<Sample<TRep>> held = new ArrayList<>();
    /** One outstanding {@link #sendRequestAsync}, by the identity its reply will carry. */
    private final Map<SampleIdentity, CompletableFuture<Sample<TRep>>> pending =
            new LinkedHashMap<>();
    /** Requests waiting for the service to be matched, oldest first. */
    private final Deque<Queued> queue = new ArrayDeque<>();
    /** What went wrong for one request: a remote exception code or a failed write. */
    private final Map<SampleIdentity, RuntimeException> failures = new LinkedHashMap<>();
    private String bound = "";
    /** Requests sent with {@link #sendRequestOneway}: their replies are dropped on arrival. */
    private final java.util.Set<SampleIdentity> oneway = new java.util.HashSet<>();
    private boolean nondata = false;
    /**
     * The request writer's own GUID. §7.2.1: the client's reader filters
     * "responses for remote invocations of other clients" by a
     * content-based filter, and a reply answers this requester only when
     * its related request id carries this GUID.
     */
    private byte[] requestWriterGuid = new byte[16];
    /** The runtime that made this requester, or {@code null} — see {@link #getEnvironment}. */
    private final RPCRuntime runtime;

    public RequesterImpl(RequesterParams params) {
        this(null, params);
    }

    /**
     * The constructor {@link RPCRuntime#createRequester} uses, so that
     * §7.11.2.2.2's {@code getEnvironment} has the runtime the
     * entity was derived from to return.
     *
     * @param runtime the runtime this requester is derived from
     * @param params the configuration of §7.11.1.4.11
     */
    @SuppressWarnings({"this-escape", "unchecked"})
    public RequesterImpl(RPCRuntime runtime, RequesterParams params) {
        this.runtime = runtime;
        this.params = params;
        this.requestSupport = (TypeSupport<TReq>) params.requestTypeSupport();
        this.replySupport = (TypeSupport<TRep>) params.replyTypeSupport();
        this.mapping = params.mapping();
        this.gateOnMatching = Matching.gates(params.matching(), params.mapping());
        // §7.11.2: a DataWriterQos or DataReaderQos the params name is the entity's QoS as it
        // stands, and the RpcQos the runtime works with is what it says.
        RpcQos request = params.getDataWriterQos() == null ? params.requestQos()
                : PsmEndpoint.rpcQosOf(params.getDataWriterQos(), params.requestQos());
        RpcQos reply = params.getDataReaderQos() == null ? params.replyQos()
                : PsmEndpoint.rpcQosOf(params.getDataReaderQos(), params.replyQos());
        this.requestQos = request.withDataRepresentation(params.wireEncoding());
        this.replyQos = reply.withDataRepresentation(params.wireEncoding());
        DomainParticipant participant = params.domainParticipant();
        if (participant != null) {
            // §7.6.2.1.1/2: "the client-side DataWriter shall not include
            // service_instance_name", nor the client-side DataReader. The
            // instance a requester is bound to travels in the request
            // (Basic: RequestHeader.instanceName), not in discovery.
            // §7.4.2.3: the names the run-time service name, the annotations
            // and the default give, in that order of precedence.
            TopicNames topics = TopicNames.of(params);
            this.writer =
                    participant.createWriter(
                            topics.request(),
                            params.requestTypeName(),
                            requestQos,
                            "",
                            params.requestTypeDescription(),
                            PsmEndpoint.writer(
                                    TopicSample.request(requestSupport, mapping, params.wireEncoding()),
                                    params.getPublisher(),
                                    params.getDataWriterQos()));
            this.reader =
                    participant.createReader(
                            topics.reply(),
                            params.replyTypeName(),
                            replyQos,
                            "",
                            params.replyTypeDescription(),
                            PsmEndpoint.reader(
                                    TopicSample.reply(replySupport, mapping, params.wireEncoding()),
                                    params.getSubscriber(),
                                    params.getDataReaderQos()));
            this.requestWriterGuid = this.writer.nextIdentity().writerGuid();
            // §7.6.2.1.1/2: the client's request writer names the reply reader as its related
            // endpoint and the reply reader the request writer; §7.4.3: the aliases of the
            // base interfaces. A binding without discovery has nothing to announce them in.
            this.writer.announce(topics.requestAliases(), this.reader.guid());
            this.reader.announce(topics.replyAliases(), this.writer.guid());
        }
        // Assigned rather than routed through `bind`: a constructor
        // that calls an overridable method hands `this` to a subclass
        // before the subclass's own fields are initialised, which
        // `javac -Xlint:this-escape` reports and which a generated
        // `<I>Requester` would walk straight into.
        this.bound = params.instanceName();
        // §7.9.2.2 and §7.11.1.4.9/10: a listener is called when the reply
        // arrives, so the reader is told to call this requester.
        if (reader != null
                && params.listener() != null
                && params.listenerDelivery() == ListenerDelivery.ON_ARRIVAL) {
            reader.setDataAvailableListener(this::onDataAvailable);
        }
    }

    private void onDataAvailable() {
        dispatchReplies();
    }

    @Override
    public synchronized SampleIdentity sendRequest(TReq request) {
        return send(request, null);
    }

    private SampleIdentity send(TReq request, CompletableFuture<Sample<TRep>> future) {
        if (writer == null) {
            throw new RemoteOutOfResourcesError("the requester has no writer");
        }
        // The identity the sample will carry: the next one the writer
        // stamps, and one further for each request still waiting ahead of it.
        SampleIdentity next = writer.nextIdentity();
        SampleIdentity predicted =
                new SampleIdentity(next.writerGuid(), next.sequenceNumber() + queue.size());
        byte[] sample;
        if (mapping == ServiceMapping.BASIC && requestSupport instanceof HeaderInRequestType) {
            // §7.3.1.4: the request type has its own `header` member, which
            // is the first member of the sample: the header goes into the
            // value, and the sample is the value.
            requestHeaderIn(requestSupport)
                    .setRequestHeader(request, new RequestHeader(predicted, bound));
            sample = requestSupport.serialize(request, 0, params.wireEncoding());
        } else if (mapping == ServiceMapping.BASIC) {
            // §7.8.1: the Basic profile carries the request identity
            // explicitly in the header, so the header names the identity the
            // writer is about to stamp on the sample it is part of.
            ByteArrayOutputStream out = new ByteArrayOutputStream();
            byte[] header = HeaderCodec.encodeRequestHeader(new RequestHeader(predicted, bound));
            out.write(header, 0, header.length);
            HeaderCodec.padTo4(out);
            // The payload starts here, and its alignment counts from the
            // start of the sample.
            byte[] payload =
                    requestSupport
                            .serialize(request, out.size(), params.wireEncoding());
            out.write(payload, 0, payload.length);
            sample = out.toByteArray();
        } else {
            // §7.5.1.2: no header; the sample identity is the correlation
            // and the bound instance is addressed by the write.
            sample = requestSupport.serialize(request, 0, params.wireEncoding());
        }
        queue.addLast(new Queued(sample, predicted, bound));
        // Registered before the write: a binding that delivers the reply
        // from inside the write has to find the future.
        if (future != null) {
            pending.put(predicted, future);
        }
        flushQueue();
        return predicted;
    }

    /** Whether §7.6.2.2's service matching is complete. */
    private boolean serviceMatched() {
        if (writer == null || reader == null) {
            return false;
        }
        if (writer.matchedCount() <= 0 || reader.matchedCount() <= 0) {
            return false;
        }
        if (!(bound.isEmpty() || mapping == ServiceMapping.BASIC || writer.matchedCount(bound) > 0)) {
            return false;
        }
        return serviceDiscovered();
    }

    /**
     * §7.6.2.2.2, Figure 7.4: "Service Matching" is complete for a service instance when the
     * client's reply reader has discovered the instance's reply writer, which the instance's
     * request reader names as its related endpoint ({@code related_datawriter_key}). A request
     * reader that announced no related endpoint is a peer that does not take part in the
     * extension, and the match by topic stands; so does a binding that sees no discovery data.
     */
    private boolean serviceDiscovered() {
        List<RpcPeerInfo> requestReaders = writer.matchedPeers();
        List<RpcPeerInfo> replyWriters = reader.matchedPeers();
        if (requestReaders == null || replyWriters == null) {
            return true;
        }
        for (RpcPeerInfo requestReader : requestReaders) {
            if (mapping == ServiceMapping.ENHANCED && !bound.isEmpty()
                    && !requestReader.instanceName().equals(bound)) {
                continue;
            }
            byte[] related = requestReader.relatedGuid();
            if (related == null) {
                return true;
            }
            for (RpcPeerInfo replyWriter : replyWriters) {
                if (Arrays.equals(replyWriter.guid(), related)) {
                    return true;
                }
            }
        }
        return false;
    }

    /**
     * Writes the queued requests, oldest first, while the rule allows. A
     * request that fails is recorded under its identity, so the call site
     * that asked for the reply sees it.
     */
    private void flushQueue() {
        while (!queue.isEmpty() && (!gateOnMatching || serviceMatched())) {
            Queued next = queue.pollFirst();
            try {
                SampleIdentity actual =
                        mapping == ServiceMapping.ENHANCED && !next.instance.isEmpty()
                                ? writer.writeTo(next.sample, next.instance)
                                : writer.write(next.sample);
                if (!actual.equals(next.predicted)) {
                    throw new RemoteUnknownExceptionError(
                            "the writer stamped " + actual + " on a request that named "
                                    + next.predicted + ": nextIdentity() did not predict it");
                }
            } catch (RuntimeException failure) {
                fail(next.predicted, failure);
            }
        }
    }

    private void fail(SampleIdentity request, RuntimeException failure) {
        CompletableFuture<Sample<TRep>> waiting = pending.remove(request);
        if (waiting != null) {
            waiting.completeExceptionally(failure);
            return;
        }
        failures.put(request, failure);
    }

    @Override
    public synchronized List<Sample<TRep>> receiveReplies() {
        drain();
        if (!failures.isEmpty()) {
            SampleIdentity first = failures.keySet().iterator().next();
            throw failures.remove(first);
        }
        List<Sample<TRep>> out = new ArrayList<>(held);
        held.clear();
        return out;
    }

    @Override
    public Sample.Iterator<TRep> replyIterator() {
        return Sample.iterate(receiveReplies());
    }

    @Override
    public synchronized Sample<TRep> receiveReply(org.omg.dds.core.SampleIdentity requestIdentity) {
        SampleIdentity requestId = SampleIdentity.of(requestIdentity);
        drain();
        RuntimeException failure = failures.remove(requestId);
        if (failure != null) {
            throw failure;
        }
        for (int at = 0; at < held.size(); ++at) {
            if (held.get(at).relatedIdentity().equals(requestId)) {
                return held.remove(at);
            }
        }
        return null;
    }

    /**
     * The future is made at send time and completed by whichever later
     * call reads the reader — {@link #receiveReply}, {@link #receiveReplies}
     * or the arrival of the reply when the reader notifies.
     */
    @Override
    public synchronized Future<Sample<TRep>> sendRequestAsync(TReq request) {
        CompletableFuture<Sample<TRep>> completable = new CompletableFuture<>();
        send(request, completable);
        drain();
        return Future.over(completable);
    }

    @Override
    @SuppressWarnings("unchecked")
    public synchronized int dispatchReplies() {
        ListenerBase listener = params.listener();
        if (listener == null) {
            return 0;
        }
        drain();
        if (listener instanceof RequesterErrorListener && !failures.isEmpty()) {
            List<Map.Entry<SampleIdentity, RuntimeException>> raised =
                    new ArrayList<>(failures.entrySet());
            failures.clear();
            for (Map.Entry<SampleIdentity, RuntimeException> entry : raised) {
                ((RequesterErrorListener) listener)
                        .onReplyFailure(entry.getKey(), entry.getValue());
            }
        }
        if (listener instanceof SimpleRequesterListener) {
            List<Sample<TRep>> replies = new ArrayList<>(held);
            held.clear();
            for (Sample<TRep> reply : replies) {
                onExecutor(() -> ((SimpleRequesterListener<TRep>) listener).process_reply(
                        reply, reply.relatedIdentity()));
            }
            return replies.size();
        }
        if (listener instanceof RequesterListener) {
            onExecutor(() -> ((RequesterListener<TReq, TRep>) listener).onReplyAvailable(this));
        }
        return 0;
    }

    /** Runs {@code call} on the executor the params carry, or on the calling thread. */
    private void onExecutor(Runnable call) {
        java.util.concurrent.ExecutorService executor = params.executor();
        if (executor == null) {
            call.run();
        } else {
            executor.execute(call);
        }
    }

    @Override
    public synchronized void setQos(RpcQos requestedRequestQos, RpcQos requestedReplyQos) {
        RpcQos newRequestQos = requestedRequestQos.withDataRepresentation(params.wireEncoding());
        RpcQos newReplyQos = requestedReplyQos.withDataRepresentation(params.wireEncoding());
        if (writer == null || reader == null) {
            throw new RemoteOutOfResourcesError("the requester has no endpoints");
        }
        if (!requestQos.mutableChangeTo(newRequestQos) || !replyQos.mutableChangeTo(newReplyQos)) {
            throw new IllegalArgumentException(
                    "RELIABILITY, DURABILITY and HISTORY cannot change after the endpoints "
                            + "are enabled (DDS 1.4 §2.2.3)");
        }
        writer.setQos(newRequestQos);
        reader.setQos(newReplyQos);
        requestQos = newRequestQos;
        replyQos = newReplyQos;
    }

    @Override
    public synchronized boolean bind(String instanceName) {
        String name = instanceName == null ? "" : instanceName;
        if (mapping == ServiceMapping.ENHANCED && !name.isEmpty() && writer != null) {
            // The Enhanced mapping addresses an instance through discovery;
            // a binding that cannot see it says so here, not at the first send.
            writer.matchedCount(name);
        }
        bound = name;
        return true;
    }

    @Override
    public synchronized boolean unbind() {
        boolean was = !bound.isEmpty();
        bound = "";
        return was;
    }

    @Override
    public synchronized java.util.Collection<String> getDiscoveredServiceInstances() {
        if (writer == null) {
            throw new IllegalStateException("the requester was closed");
        }
        return writer.matchedInstances();
    }

    @Override
    public synchronized boolean isBound() {
        return !bound.isEmpty();
    }

    @Override
    public synchronized String boundInstanceName() {
        return bound;
    }

    @Override
    public synchronized boolean isServiceMatched() {
        flushQueue();
        return serviceMatched();
    }

    @Override
    public synchronized void close() {
        // §7.9.3: deactivation deletes the underlying DDS entities.
        DomainParticipant participant = params.domainParticipant();
        if (reader != null
                && params.listener() != null
                && params.listenerDelivery() == ListenerDelivery.ON_ARRIVAL) {
            reader.setDataAvailableListener(null);
        }
        // A request still waiting for the service will not be written.
        for (Queued dropped : queue) {
            fail(dropped.predicted, new RemoteOutOfResourcesError(
                    "the requester was closed before the service was matched"));
        }
        queue.clear();
        for (CompletableFuture<Sample<TRep>> waiting : pending.values()) {
            waiting.cancel(true);
        }
        pending.clear();
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
    public synchronized DataWriter requestWriter() {
        if (writer == null) {
            throw new IllegalStateException("the requester has no writer: it was closed");
        }
        return writer;
    }

    @Override
    public synchronized DataReader replyReader() {
        if (reader == null) {
            throw new IllegalStateException("the requester has no reader: it was closed");
        }
        return reader;
    }

    @Override
    public synchronized boolean isNull() {
        return writer == null && reader == null;
    }

    /**
     * §7.11.2.2.2 through [DDS-Java-PSM] §7.3.1: the environment this
     * requester was derived from, which §7.11.2.2.3 makes the
     * {@link RPCRuntime} that created it.
     *
     * @return that runtime.
     * @throws IllegalStateException when the requester was constructed
     *     without one.
     */
    @Override
    public org.omg.dds.core.ServiceEnvironment getEnvironment() {
        if (runtime == null) {
            return Requester.super.getEnvironment();
        }
        return runtime;
    }

    /**
     * Take everything the reader has: complete the future of any request
     * waiting on one, park the rest. A reply that carries a remote
     * exception code is recorded as the failure of its request, which is
     * where §7.11.1.3 puts it — the client's call site is what has to see
     * it — and the samples behind it in the same take are kept.
     */
    @SuppressWarnings("unchecked")
    private void drain() {
        if (reader == null) {
            return;
        }
        flushQueue();
        for (TakenSample taken : reader.take()) {
            if (!taken.validData()) {
                // §7.11.2 receiveNondataSamples: a sample without data answers no request.
                if (nondata) {
                    held.add(new NondataSample<TRep>(taken.identity(), taken.instanceState()));
                }
                continue;
            }
            SampleIdentity related;
            TRep value;
            RemoteExceptionCode code;
            if (mapping == ServiceMapping.BASIC
                    && replySupport instanceof HeaderInReplyType) {
                // §7.3.1.4: the reply type has its own `header` member.
                value = replySupport
                                .deserialize(taken.octets(), new HeaderCodec.Cursor(0), params.wireEncoding());
                ReplyHeader header = replyHeaderIn(replySupport).replyHeader(value);
                related = header.relatedRequestId();
                code = header.remoteEx();
                if (!Arrays.equals(related.writerGuid(), requestWriterGuid)) {
                    continue;
                }
            } else if (mapping == ServiceMapping.BASIC) {
                HeaderCodec.Cursor cursor = new HeaderCodec.Cursor(0);
                ReplyHeader header = HeaderCodec.decodeReplyHeader(taken.octets(), cursor);
                related = header.relatedRequestId();
                code = header.remoteEx();
                // §7.2.1: content-based filter. A reply answering another
                // client's request carries a different request-writer GUID
                // and is dropped here, before its exception code could be
                // raised.
                if (!Arrays.equals(related.writerGuid(), requestWriterGuid)) {
                    continue;
                }
                HeaderCodec.alignTo4(cursor);
                value = replySupport.deserialize(taken.octets(), cursor, params.wireEncoding());
            } else {
                // §7.8.2.2: the client retrieves the related sample identity
                // of the reply from the reply SampleInfo.
                related = taken.relatedIdentity();
                if (related.isUnknown() || related.equals(SampleIdentity.RELATED_SAMPLE_IDENTITY_INVALID)) {
                    throw new RemoteInvalidArgumentError(
                            "a reply without PID_RELATED_SAMPLE_IDENTITY cannot be correlated "
                                    + "in the Enhanced mapping (§7.8.2)");
                }
                if (!Arrays.equals(related.writerGuid(), requestWriterGuid)) {
                    continue;
                }
                value = replySupport
                                .deserialize(taken.octets(), new HeaderCodec.Cursor(0), params.wireEncoding());
                code = replySupport instanceof EnhancedReplySupport
                        ? ((EnhancedReplySupport<TRep>) replySupport)
                                .remoteExceptionOf(value)
                        : RemoteExceptionCode.REMOTE_EX_OK;
            }
            if (oneway.remove(related)) {
                continue; // the reply of a request that expected none
            }
            if (code != RemoteExceptionCode.REMOTE_EX_OK) {
                RuntimeException failure = failureOf(code);
                CompletableFuture<Sample<TRep>> waiting = pending.remove(related);
                if (waiting != null) {
                    waiting.completeExceptionally(failure);
                } else {
                    failures.put(related, failure);
                }
                continue;
            }
            Sample<TRep> sample = Sample.of(value, taken.identity(), related);
            CompletableFuture<Sample<TRep>> waiting = pending.remove(related);
            if (waiting != null) {
                waiting.complete(sample);
                continue;
            }
            held.add(sample);
        }
    }

    /** The support of a reply type that has its own header, as the interface that says so. */
    @SuppressWarnings("unchecked")
    private HeaderInReplyType<TRep> replyHeaderIn(TypeSupport<TRep> support) {
        return (HeaderInReplyType<TRep>) support;
    }

    /** The support of a request type that has its own header, as the interface that says so. */
    @SuppressWarnings("unchecked")
    private HeaderInRequestType<TReq> requestHeaderIn(TypeSupport<TReq> support) {
        return (HeaderInRequestType<TReq>) support;
    }

    // ---- the operations of omg-dds-rpc-java.zip (DDS-RPC 1.0 §7.11.2) ----

    @Override
    public synchronized void sendRequestOneway(TReq request) {
        oneway.add(send(request, null));
    }

    @Override
    public synchronized Future<Sample<TRep>> sendRequestAsync(
            TReq request, java.util.concurrent.ExecutorService executor) {
        CompletableFuture<Sample<TRep>> completable = new CompletableFuture<>();
        send(request, completable);
        drain();
        return Future.over(completable, java.util.Objects.requireNonNull(executor, "executor"));
    }

    private static long nanosOf(org.omg.dds.core.Duration maxWait) {
        return maxWait == null || maxWait.isInfinite()
                ? Long.MAX_VALUE
                : maxWait.getDuration(java.util.concurrent.TimeUnit.NANOSECONDS);
    }

    /** The number of replies held that answer {@code related} (all of them when {@code null}). */
    private synchronized int available(SampleIdentity related) {
        drain();
        if (!failures.isEmpty() && related == null) {
            return Math.max(held.size(), 1);
        }
        int count = 0;
        for (Sample<TRep> reply : held) {
            if (related == null || reply.relatedIdentity().equals(related)) {
                ++count;
            }
        }
        return count;
    }

    private boolean waitFor(int minCount, org.omg.dds.core.Duration maxWait, SampleIdentity related) {
        return ServiceProxy.pollUntil(
                maxWait == null ? null : maxWait,
                () -> available(related) >= Math.max(minCount, 1));
    }

    @Override
    public boolean waitForReplies(int minCount, org.omg.dds.core.Duration maxWait) {
        return waitFor(minCount, maxWait, null);
    }

    @Override
    public boolean waitForReplies(org.omg.dds.core.Duration maxWait) {
        return waitFor(1, maxWait, null);
    }

    @Override
    public boolean waitForReplies(
            int minCount, org.omg.dds.core.Duration maxWait,
            org.omg.dds.core.SampleIdentity relatedRequestId) {
        return waitFor(minCount, maxWait, SampleIdentity.of(relatedRequestId));
    }

    @Override
    public Sample<TRep> receiveReply(org.omg.dds.core.Duration timeout) {
        waitFor(1, timeout, null);
        return takeReply();
    }

    @Override
    public Sample.Iterator<TRep> receiveReplies(org.omg.dds.core.Duration maxWait) {
        waitFor(1, maxWait, null);
        return takeReplies(Integer.MAX_VALUE);
    }

    @Override
    public Sample.Iterator<TRep> receiveReplies(
            int minCount, int maxCount, org.omg.dds.core.Duration maxWait) {
        waitFor(minCount, maxWait, null);
        return takeReplies(maxCount);
    }

    /** Raises the failure that is waiting, as {@link #receiveReplies()} does. */
    private void raiseFailure() {
        if (!failures.isEmpty()) {
            SampleIdentity first = failures.keySet().iterator().next();
            throw failures.remove(first);
        }
    }

    private List<Sample<TRep>> pick(int maxCount, SampleIdentity related, boolean remove) {
        List<Sample<TRep>> out = new ArrayList<>();
        for (int at = 0; at < held.size() && out.size() < maxCount; ) {
            Sample<TRep> reply = held.get(at);
            if (related == null || reply.relatedIdentity().equals(related)) {
                out.add(reply);
                if (remove) {
                    held.remove(at);
                    continue;
                }
            }
            ++at;
        }
        return out;
    }

    @Override
    public synchronized Sample<TRep> takeReply() {
        drain();
        raiseFailure();
        List<Sample<TRep>> one = pick(1, null, true);
        return one.isEmpty() ? null : one.get(0);
    }

    @Override
    public synchronized Sample<TRep> takeReply(org.omg.dds.core.SampleIdentity relatedRequestId) {
        return receiveReply(relatedRequestId);
    }

    @Override
    public synchronized Sample.Iterator<TRep> takeReplies(int maxCount) {
        drain();
        raiseFailure();
        return Sample.iterate(pick(maxCount, null, true));
    }

    @Override
    public synchronized Sample.Iterator<TRep> takeReplies(
            int maxCount, org.omg.dds.core.SampleIdentity relatedRequestId) {
        SampleIdentity related = SampleIdentity.of(relatedRequestId);
        drain();
        RuntimeException failure = failures.remove(related);
        if (failure != null) {
            throw failure;
        }
        return Sample.iterate(pick(maxCount, related, true));
    }

    @Override
    public Sample.Iterator<TRep> takeReplies(org.omg.dds.core.SampleIdentity relatedRequestId) {
        return takeReplies(Integer.MAX_VALUE, relatedRequestId);
    }

    @Override
    public synchronized Sample<TRep> readReply() {
        drain();
        raiseFailure();
        List<Sample<TRep>> one = pick(1, null, false);
        return one.isEmpty() ? null : one.get(0);
    }

    @Override
    public synchronized Sample<TRep> readReply(org.omg.dds.core.SampleIdentity relatedRequestId) {
        drain();
        List<Sample<TRep>> one = pick(1, SampleIdentity.of(relatedRequestId), false);
        return one.isEmpty() ? null : one.get(0);
    }

    @Override
    public synchronized Sample.Iterator<TRep> readReplies(int maxCount) {
        drain();
        raiseFailure();
        return Sample.iterate(pick(maxCount, null, false));
    }

    @Override
    public Sample<TRep> readReplies(int maxCount, org.omg.dds.core.SampleIdentity relatedRequestId) {
        return readReply(relatedRequestId);
    }

    @Override
    public Sample<TRep> readReplies(org.omg.dds.core.SampleIdentity relatedRequestId) {
        return readReply(relatedRequestId);
    }

    @Override
    public synchronized boolean receiveNondataSamples(boolean enable) {
        boolean previous = nondata;
        nondata = enable;
        return previous;
    }

    @Override
    public RequesterParams getRequesterParams() {
        return params;
    }

    @Override
    @SuppressWarnings("unchecked")
    public synchronized org.omg.dds.pub.DataWriter<TReq> getRequestDataWriter() {
        if (writer == null) {
            throw new IllegalStateException("the requester was closed");
        }
        // The writer is typed by TReq when it is the PSM's: the class of the request support.
        return (org.omg.dds.pub.DataWriter<TReq>) writer.psm();
    }

    @Override
    @SuppressWarnings("unchecked")
    public synchronized org.omg.dds.sub.DataReader<TRep> getReplyDataReader() {
        if (reader == null) {
            throw new IllegalStateException("the requester was closed");
        }
        return (org.omg.dds.sub.DataReader<TRep>) reader.psm();
    }

    private static RuntimeException failureOf(RemoteExceptionCode code) {
        try {
            code.raise("the service returned a remote exception");
        } catch (RuntimeException raised) {
            return raised;
        }
        throw new IllegalStateException("REMOTE_EX_OK has no failure");
    }
}
