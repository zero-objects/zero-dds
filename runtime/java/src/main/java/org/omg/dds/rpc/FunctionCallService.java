// SPDX-License-Identifier: Apache-2.0
package org.omg.dds.rpc;

import java.util.ArrayList;
import java.util.List;

/**
 * The part of a function-call {@link Service} that does not depend on the
 * interface: DDS-RPC 1.0 §7.11.1.5.1 "A Service instantiates the underlying
 * DDS entities and makes the service discoverable", §7.9.1: creating it
 * activates it, §7.9.3: {@code close()} deactivates it.
 *
 * <p>The requests are answered when they arrive (§7.9.2.1: "Concrete
 * implementations of the interface operations shall be invoked as a
 * consequence of receiving requests") and by {@link #dispatch}, which a
 * {@link Server} calls for a binding that does not notify on arrival.
 *
 * @param <TReq> the request topic type
 * @param <TRep> the reply topic type
 */
public abstract class FunctionCallService<TReq, TRep> implements Service {

    private final ReplierParams params;
    private final Replier<TReq, TRep> replier;
    private final List<Service> bases = new ArrayList<>();
    private volatile boolean paused = false;
    private int answered = 0;

    /**
     * Creates the underlying DataReader and DataWriter.
     *
     * @param params the topics, types, instance name and QoS of the interface
     * @param handler the generated unpack/call/pack of the interface
     */
    @SuppressWarnings("this-escape")
    protected FunctionCallService(
            ReplierParams params, RequestHandler<TReq, TRep> handler) {
        this.params = params;
        this.replier =
                RPCRuntime.over(params.domainParticipant())
                        .createReplier(params.listener(new Answering(handler)));
    }

    /** Registers the service of a base interface, to be paused and closed with this one. */
    protected final void registerBase(Service base) {
        bases.add(base);
    }

    /** Answers every request that has arrived, through the handler. */
    private final class Answering implements ReplierListener<TReq, TRep> {
        private final RequestHandler<TReq, TRep> handler;

        Answering(RequestHandler<TReq, TRep> handler) {
            this.handler = handler;
        }

        @Override
        public void onRequestAvailable(Replier<TReq, TRep> from) {
            if (paused) {
                return;
            }
            for (Sample<TReq> request : from.receiveRequests()) {
                if (!request.validData()) {
                    continue; // a state change, no call (receiveNondataSamples)
                }
                Response<TRep> response;
                try {
                    response = handler.handle(request.getData());
                } catch (RuntimeException failure) {
                    // §7.5.2: the implementation failed with something its
                    // operation does not declare.
                    response =
                            Response.failed(
                                    handler.failureReply(request.getData()),
                                    RemoteExceptionCode.REMOTE_EX_UNKNOWN_EXCEPTION);
                }
                if (response != null) {
                    from.sendReply(response.reply(), request.identity(), response.code());
                }
                synchronized (FunctionCallService.this) {
                    ++answered;
                }
            }
        }
    }

    @Override
    public final int dispatch() {
        int before;
        synchronized (this) {
            before = answered;
        }
        if (!paused) {
            replier.dispatchRequests();
        }
        int total;
        synchronized (this) {
            total = answered - before;
        }
        for (Service base : bases) {
            total += base.isPaused() ? 0 : base.dispatch();
        }
        return total;
    }

    @Override
    public ServiceParams getServiceParams() {
        return ServiceParams.create()
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

    /**
     * §7.11.2 {@code getRequestDataReader(Class)}: the request DataReader of the DDS Java PSM
     * whose topic type is {@code requestType} — this service's, or that of the service of a
     * base interface (§7.5.1.1.8). {@code null} when that replier has no PSM entity
     * ({@link Replier#getRequestDataReader()}).
     *
     * @throws IllegalArgumentException when no topic of this service has that type
     */
    @Override
    @SuppressWarnings("unchecked")
    public <T> org.omg.dds.sub.DataReader<T> getRequestDataReader(Class<?> requestType) {
        FunctionCallService<?, ?> owner = owning(requestType, true);
        if (owner == null) {
            throw new IllegalArgumentException(
                    requestType + " is not the request type of this service or of a base");
        }
        return (org.omg.dds.sub.DataReader<T>) owner.replier.getRequestDataReader();
    }

    /** §7.11.2 {@code getReplyDataWriter(Class)}; see {@link #getRequestDataReader(Class)}. */
    @Override
    @SuppressWarnings("unchecked")
    public <T> org.omg.dds.pub.DataWriter<T> getReplyDataWriter(Class<?> replyType) {
        FunctionCallService<?, ?> owner = owning(replyType, false);
        if (owner == null) {
            throw new IllegalArgumentException(
                    replyType + " is not the reply type of this service or of a base");
        }
        return (org.omg.dds.pub.DataWriter<T>) owner.replier.getReplyDataWriter();
    }

    /** The service, this one or one of a base, whose request (or reply) type is {@code type}. */
    private FunctionCallService<?, ?> owning(Class<?> type, boolean request) {
        Class<?> own = org.zerodds.ddsrpc.TopicSample.classOf(
                request ? params.requestTypeSupport() : params.replyTypeSupport());
        if (own != null && own == type) {
            return this;
        }
        for (Service base : bases) {
            if (base instanceof FunctionCallService) {
                FunctionCallService<?, ?> found =
                        ((FunctionCallService<?, ?>) base).owning(type, request);
                if (found != null) {
                    return found;
                }
            }
        }
        return null;
    }

    @Override
    public String serviceName() {
        return params.serviceName().isEmpty() ? "Service" : params.serviceName();
    }

    @Override
    public void pause() {
        paused = true;
        for (Service base : bases) {
            base.pause();
        }
    }

    @Override
    public void resume() {
        paused = false;
        for (Service base : bases) {
            base.resume();
        }
        // What arrived while paused stayed in the reader.
        dispatch();
    }

    @Override
    public boolean isPaused() {
        return paused;
    }

    @Override
    public void close() {
        for (Service base : bases) {
            base.close();
        }
        replier.close();
    }

    @Override
    public boolean isNull() {
        return replier.isNull();
    }

    @Override
    public org.omg.dds.core.ServiceEnvironment getEnvironment() {
        return replier.getEnvironment();
    }
}
