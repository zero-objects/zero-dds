// SPDX-License-Identifier: Apache-2.0
package org.omg.dds.rpc;

import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.ExecutionException;

/**
 * The part of a function-call {@link Client} that does not depend on the
 * interface: DDS-RPC 1.0 §7.11.1.5.4 "Client is a reference type and
 * provides functions to invoke operations on a remote service synchronously
 * and asynchronously. A Client instantiates the underlying DDS entities and
 * makes them discoverable."
 *
 * <p>The generated {@code <I>ClientImpl} extends this, builds the request
 * of each operation, hands it to {@link #call}, {@link #callAsync} or
 * {@link #callOneway} and unpacks the reply. A client of a derived
 * interface registers the clients of its bases (§7.5.1.1.8: "two topics for
 * every interface in the hierarchy"), and the lifecycle operations apply to
 * all of them.
 *
 * @param <TReq> the request topic type
 * @param <TRep> the reply topic type
 */
public abstract class FunctionCallClient<TReq, TRep> implements ClientEndpoint {

    private static final long DEFAULT_TIMEOUT_MILLIS = 5000L;

    /** The requester; the runtime's, whose futures take a completion listener. */
    private final org.zerodds.ddsrpc.RequesterImpl<TReq, TRep> requester;
    private final RequesterParams params;
    private final List<ClientEndpoint> bases = new ArrayList<>();
    private volatile long timeoutMillis = DEFAULT_TIMEOUT_MILLIS;

    /**
     * Creates the underlying DataWriter and DataReader.
     *
     * @param params the topics, types and QoS of the interface
     */
    protected FunctionCallClient(RequesterParams params) {
        // The futures of the asynchronous operations are completed when the
        // reply is taken from the reader, so the requester has a listener to
        // be told on arrival (§7.11.1.4.10) that takes it; the replies
        // themselves go to the futures, not to the listener.
        if (params.listener() == null) {
            params.listener(new Arrival<TReq, TRep>());
        }
        this.params = params;
        this.requester = (org.zerodds.ddsrpc.RequesterImpl<TReq, TRep>)
                RPCRuntime.over(params.domainParticipant()).<TReq, TRep>createRequester(params);
    }

    /** Takes the replies on arrival; the replies belong to the futures. */
    private static final class Arrival<TReq, TRep> implements RequesterListener<TReq, TRep> {
        @Override
        public void onReplyAvailable(Requester<TReq, TRep> from) {
            // Completing the futures is the point of being called.
        }
    }

    /**
     * Takes the replies that have arrived and completes the futures of the
     * asynchronous operations, for a binding that does not notify on arrival
     * ({@link ListenerDelivery#ON_PUMP}).
     */
    public final void dispatchReplies() {
        requester.dispatchReplies();
        for (ClientEndpoint base : bases) {
            if (base instanceof FunctionCallClient) {
                ((FunctionCallClient<?, ?>) base).dispatchReplies();
            }
        }
    }

    /** Registers the client of a base interface, to be closed and bound with this one. */
    protected final void registerBase(ClientEndpoint base) {
        bases.add(base);
    }

    /**
     * How long a synchronous call waits for its reply; the default is five
     * seconds.
     *
     * @param millis the wait, positive
     */
    public final void setTimeoutMillis(long millis) {
        if (millis <= 0) {
            throw new IllegalArgumentException("a timeout is positive");
        }
        this.timeoutMillis = millis;
    }

    /** The requester behind this client. */
    protected final Requester<TReq, TRep> requester() {
        return requester;
    }

    /**
     * Sends {@code request} and waits for its reply.
     *
     * @return the reply sample's value
     * @throws TimeoutError when no reply arrived in time
     * @throws RuntimeException the remote exception of §7.11.2.1 when the
     *     reply carried a code other than {@code REMOTE_EX_OK}
     */
    protected final TRep call(TReq request) {
        Future<Sample<TRep>> pending = requester.sendRequestAsync(request);
        long deadline = System.nanoTime() + timeoutMillis * 1_000_000L;
        while (!pending.isDone()) {
            // A binding whose reader notifies has completed the future
            // already; for one that does not, taking the replies does.
            requester.receiveReplies();
            if (pending.isDone()) {
                break;
            }
            if (System.nanoTime() >= deadline) {
                pending.cancel(true);
                throw new TimeoutError("no reply within " + timeoutMillis + " ms");
            }
            try {
                Thread.sleep(1L);
            } catch (InterruptedException interrupted) {
                Thread.currentThread().interrupt();
                pending.cancel(true);
                throw new RemoteOutOfResourcesError("interrupted while waiting for the reply");
            }
        }
        try {
            return pending.get().getData();
        } catch (ExecutionException failure) {
            Throwable cause = failure.getCause();
            if (cause instanceof RuntimeException) {
                throw (RuntimeException) cause;
            }
            throw new RemoteUnknownExceptionError("the call failed: " + cause);
        } catch (InterruptedException interrupted) {
            Thread.currentThread().interrupt();
            throw new RemoteOutOfResourcesError("interrupted while waiting for the reply");
        }
    }

    /** Sends {@code request} and returns the future of the reply sample. */
    protected final Future<Sample<TRep>> callAsync(TReq request) {
        return requester.sendRequestAsync(request);
    }

    /** Sends {@code request}, an operation that has no reply. */
    protected final void callOneway(TReq request) {
        requester.sendRequest(request);
    }

    @Override
    public ClientParams getClientParams() {
        return ClientParams.create()
                .domainParticipant(params.domainParticipant())
                .serviceName(params.serviceName())
                .instanceName(params.instanceName())
                .requestTopicName(params.requestTopicName())
                .replyTopicName(params.replyTopicName())
                .requestTopicNameSuffix(params.requestTopicNameSuffix())
                .replyTopicNameSuffix(params.replyTopicNameSuffix())
                .requestQos(params.requestQos())
                .replyQos(params.replyQos())
                .mapping(params.mapping())
                .matching(params.matching())
                .listenerDelivery(params.listenerDelivery())
                .wireEncoding(params.wireEncoding())
                .withPublisher(params.getPublisher())
                .withSubscriber(params.getSubscriber())
                .withDataWriterQos(params.getDataWriterQos())
                .withDataReaderQos(params.getDataReaderQos());
    }

    @Override
    public DataWriter requestWriter() {
        return requester.requestWriter();
    }

    /**
     * §7.11.2 {@code getRequestDataWriter(Class)}: the request DataWriter of the DDS Java PSM
     * whose topic type is {@code requestType} — this client's, or that of the client of a base
     * interface, whose operations travel on the base's topics (§7.5.1.1.8). {@code null} when
     * that requester has no PSM entity ({@link Requester#getRequestDataWriter()}).
     *
     * @throws IllegalArgumentException when no topic of this client has that type
     */
    @Override
    @SuppressWarnings("unchecked")
    public <T> org.omg.dds.pub.DataWriter<T> getRequestDataWriter(Class<?> requestType) {
        FunctionCallClient<?, ?> owner = owning(requestType, true);
        if (owner == null) {
            throw new IllegalArgumentException(
                    requestType + " is not the request type of this client or of a base");
        }
        return (org.omg.dds.pub.DataWriter<T>) owner.requester.getRequestDataWriter();
    }

    /** §7.11.2 {@code getReplyDataReader(Class)}; see {@link #getRequestDataWriter(Class)}. */
    @Override
    @SuppressWarnings("unchecked")
    public <T> org.omg.dds.sub.DataReader<T> getReplyDataReader(Class<?> replyType) {
        FunctionCallClient<?, ?> owner = owning(replyType, false);
        if (owner == null) {
            throw new IllegalArgumentException(
                    replyType + " is not the reply type of this client or of a base");
        }
        return (org.omg.dds.sub.DataReader<T>) owner.requester.getReplyDataReader();
    }

    /** The client, this one or one of a base, whose request (or reply) type is {@code type}. */
    private FunctionCallClient<?, ?> owning(Class<?> type, boolean request) {
        Class<?> own = org.zerodds.ddsrpc.TopicSample.classOf(
                request ? params.requestTypeSupport() : params.replyTypeSupport());
        if (own != null && own == type) {
            return this;
        }
        for (ClientEndpoint base : bases) {
            if (base instanceof FunctionCallClient) {
                FunctionCallClient<?, ?> found = ((FunctionCallClient<?, ?>) base).owning(type, request);
                if (found != null) {
                    return found;
                }
            }
        }
        return null;
    }

    @Override
    public DataReader replyReader() {
        return requester.replyReader();
    }

    @Override
    public boolean bind(String instanceName) {
        boolean bound = requester.bind(instanceName);
        for (ClientEndpoint base : bases) {
            bound &= base.bind(instanceName);
        }
        return bound;
    }

    @Override
    public boolean unbind() {
        boolean was = requester.unbind();
        for (ClientEndpoint base : bases) {
            was |= base.unbind();
        }
        return was;
    }

    @Override
    public java.util.Collection<String> getDiscoveredServiceInstances() {
        return requester.getDiscoveredServiceInstances();
    }

    @Override
    public boolean isBound() {
        return requester.isBound();
    }

    @Override
    public String boundInstanceName() {
        return requester.boundInstanceName();
    }

    @Override
    public boolean isServiceMatched() {
        boolean matched = requester.isServiceMatched();
        for (ClientEndpoint base : bases) {
            matched &= base.isServiceMatched();
        }
        return matched;
    }

    @Override
    public void close() {
        for (ClientEndpoint base : bases) {
            base.close();
        }
        requester.close();
    }

    @Override
    public boolean isNull() {
        return requester.isNull();
    }

    @Override
    public org.omg.dds.core.ServiceEnvironment getEnvironment() {
        return requester.getEnvironment();
    }
}
