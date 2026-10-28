// SPDX-License-Identifier: Apache-2.0
package org.omg.dds.rpc;

/**
 * DDS-RPC 1.0 §7.11.1.4.11 through §7.11.2.2.1: "RequesterParams is a valuetype
 * that serves as a container of configuration parameters of a Requester. It is
 * designed to mimic the named-parameters feature available in some
 * programming languages, which improves readability."
 *
 * <p>An interface, as §7.11.2.2.1 lists it among the interfaces of the
 * binding; {@link #create()} and {@link RPCRuntime#createRequesterParams()} make
 * the implementation. Every setter returns the params, so a call chain
 * names each parameter at the place it is set.
 */
public interface RequesterParams {

    /** An empty params object. */
    static RequesterParams create() {
        return new org.zerodds.ddsrpc.RequesterParamsImpl();
    }

    /** The participant the endpoints are created from. */
    RequesterParams domainParticipant(DomainParticipant participant);

    /**
     * The {@code <service_name>} segment of the topic names (§7.4.1),
     * {@code ""} for the default {@code "Service"}. It is not the name of
     * a service instance; that is {@link #instanceName}.
     */
    RequesterParams serviceName(String name);

    /**
     * The service instance every request of this requester is bound to (§7.11.1.4.3), {@code ""} for the default instance. It travels in the {@code RequestHeader} of the Basic mapping and is never announced in discovery: "the client-side DataWriter shall not include service_instance_name" (§7.6.2.1.1).
     */
    RequesterParams instanceName(String name);

    RequesterParams requestTopicName(String name);

    RequesterParams replyTopicName(String name);

    /**
     * The {@code <service_name>} an annotation on the types gives
     * ({@code @service}, {@code @DDSService}; §7.4.1, §7.4.2.2). The name
     * set with {@link #serviceName} takes precedence over it (§7.4.2.3).
     */
    RequesterParams annotatedServiceName(String name);

    /**
     * The interface name that stands in front of the topic names when a
     * name is synthesised (§7.4.1), {@code ""} for the request-reply style,
     * which has none.
     */
    RequesterParams interfaceName(String name);

    /** The run-time suffix of the request topic name, §7.4.2.3. */
    RequesterParams requestTopicNameSuffix(String suffix);

    /** The run-time suffix of the reply topic name, §7.4.2.3. */
    RequesterParams replyTopicNameSuffix(String suffix);

    /**
     * The qualified names of the base interfaces (§7.4.3): each gives one alias of the request
     * and one of the reply topic, obtained by applying §7.4.1 to the base. The aliases are
     * announced in discovery (PID_TOPIC_ALIASES) so that an endpoint of a base interface
     * matches. None under the request-reply style, whose names carry no interface segment.
     */
    RequesterParams parents(java.util.List<String> baseInterfaces);

    /** The DDS type name of the request topic, §7.5.1.1.6. */
    RequesterParams requestTypeName(String name);

    /** The DDS type name of the reply topic, §7.5.1.1.7. */
    RequesterParams replyTypeName(String name);

    /**
     * The XTypes descriptions of the request and the reply type (§7.5.1.2, §7.7.2.5): the
     * endpoints announce them and serve their objects. {@code null} for none; peers then match
     * the topic by its type name.
     */
    RequesterParams typeDescriptions(org.zerodds.ddsrpc.TypeDescription request, org.zerodds.ddsrpc.TypeDescription reply);

    /** The listener that is called when a reply has arrived (§7.11.1.4.9, §7.11.1.4.10). */
    RequesterParams listener(ListenerBase listener);

    RequesterParams requestTypeSupport(TypeSupport<?> support);

    RequesterParams replyTypeSupport(TypeSupport<?> support);

    /** The QoS of the endpoints on the request topic (§7.10, default §7.10.2). */
    RequesterParams requestQos(RpcQos qos);

    /** The QoS of the endpoints on the reply topic. */
    RequesterParams replyQos(RpcQos qos);

    /**
     * The two {@code @Qos} URLs of §7.10.1, {@code RequestProfile} and
     * {@code ReplyProfile}, resolved with {@link RpcQos#fromUrl}. Either may
     * be {@code null} and then leaves the QoS of its topic alone.
     */
    RequesterParams qosProfiles(String requestProfile, String replyProfile);

    /** Which profile the endpoints speak; default {@link ServiceMapping#BASIC}. */
    RequesterParams mapping(ServiceMapping mapping);

    /** The §7.6.2.2 matching rule; default {@link ServiceMatching#PER_MAPPING}. */
    RequesterParams matching(ServiceMatching matching);

    /** Whether replies must carry §7.8.2's parameter; default {@link RelatedIdentity#REQUIRED}. */
    RequesterParams relatedIdentity(RelatedIdentity policy);

    /**
     * The CDR encoding of the samples; default {@link WireEncoding#XCDR1}
     * (§7.5.1.1: DDS 1.3 without XTypes), {@link WireEncoding#XCDR2} is the
     * named alternative. Both ends of a topic pair must agree.
     */
    RequesterParams wireEncoding(WireEncoding encoding);

    /** When the listener is called; default {@link ListenerDelivery#ON_ARRIVAL}. */
    RequesterParams listenerDelivery(ListenerDelivery delivery);

    /**
     * Takes the settings that are not about one interface — the mapping, the
     * matching rule, the related-identity policy, when the listener is
     * called, the Publisher, the Subscriber and the two PSM QoS — from
     * {@code other}. A function-call requester of a derived
     * interface creates one for each base interface and gives it the
     * settings of its own.
     */
    default RequesterParams sameSettingsAs(RequesterParams other) {
        return mapping(other.mapping())
                .matching(other.matching())
                .relatedIdentity(other.relatedIdentity())
                .listenerDelivery(other.listenerDelivery())
                .wireEncoding(other.wireEncoding())
                .withPublisher(other.getPublisher())
                .withSubscriber(other.getSubscriber())
                .withDataWriterQos(other.getDataWriterQos())
                .withDataReaderQos(other.getDataReaderQos());
    }

    DomainParticipant domainParticipant();

    String serviceName();

    String instanceName();

    String requestTopicName();

    String replyTopicName();

    String annotatedServiceName();

    String interfaceName();

    String requestTopicNameSuffix();

    String replyTopicNameSuffix();

    java.util.List<String> parents();

    String requestTypeName();

    String replyTypeName();

    /** The description of the request type, or {@code null}. */
    org.zerodds.ddsrpc.TypeDescription requestTypeDescription();

    /** The description of the reply type, or {@code null}. */
    org.zerodds.ddsrpc.TypeDescription replyTypeDescription();

    ListenerBase listener();

    TypeSupport<?> requestTypeSupport();

    TypeSupport<?> replyTypeSupport();

    RpcQos requestQos();

    RpcQos replyQos();

    ServiceMapping mapping();

    ServiceMatching matching();

    RelatedIdentity relatedIdentity();

    ListenerDelivery listenerDelivery();

    WireEncoding wireEncoding();

    // ---- the spelling of omg-dds-rpc-java.zip (DDS-RPC 1.0 §7.11.2) ----

    default RequesterParams withDomainParticipant(org.omg.dds.domain.DomainParticipant participant) {
        return domainParticipant(new org.zerodds.ddsrpc.ZeroDdsParticipant(participant));
    }

    /** The DDS-Java-PSM participant, or {@code null} when the participant is not one of the PSM. */
    default org.omg.dds.domain.DomainParticipant getDomainParticipant() {
        DomainParticipant bound = domainParticipant();
        return bound instanceof org.zerodds.ddsrpc.ZeroDdsParticipant
                ? ((org.zerodds.ddsrpc.ZeroDdsParticipant) bound).psm()
                : null;
    }

    default RequesterParams withServiceName(String name) {
        return serviceName(name);
    }

    default String getServiceName() {
        return serviceName();
    }

    default RequesterParams withRequestTopicName(String name) {
        return requestTopicName(name);
    }

    default RequesterParams withReplyTopicName(String name) {
        return replyTopicName(name);
    }

    /** The spelling the Java file of the specification gives {@link #withReplyTopicName}. */
    default RequesterParams withreplyTopicName(String name) {
        return replyTopicName(name);
    }

    default String getRequestTopicName() {
        return requestTopicName();
    }

    default String getReplyTopicName() {
        return replyTopicName();
    }

    /**
     * The QoS the request DataWriter is created with (§7.11.2). Set, it is the writer's QoS as
     * it stands, and {@link #requestQos} is derived from it; {@code null} (the default): the
     * writer's QoS is the one {@link #requestQos} gives.
     */
    RequesterParams withDataWriterQos(org.omg.dds.pub.DataWriterQos qos);

    /**
     * The QoS the reply DataReader is created with; {@link #replyQos} is derived from it, as
     * {@link #withDataWriterQos} describes.
     */
    RequesterParams withDataReaderQos(org.omg.dds.sub.DataReaderQos qos);

    /**
     * The Publisher the request DataWriter is created in, a Publisher of the participant;
     * {@code null} (the default): one the binding creates.
     */
    RequesterParams withPublisher(org.omg.dds.pub.Publisher publisher);

    /** The Subscriber the reply DataReader is created in; see {@link #withPublisher}. */
    RequesterParams withSubscriber(org.omg.dds.sub.Subscriber subscriber);

    /** The QoS set with {@link #withDataWriterQos}, or {@code null}. */
    org.omg.dds.pub.DataWriterQos getDataWriterQos();

    /** The QoS set with {@link #withDataReaderQos}, or {@code null}. */
    org.omg.dds.sub.DataReaderQos getDataReaderQos();

    /** The Publisher set with {@link #withPublisher}, or {@code null}. */
    org.omg.dds.pub.Publisher getPublisher();

    /** The Subscriber set with {@link #withSubscriber}, or {@code null}. */
    org.omg.dds.sub.Subscriber getSubscriber();

    /** The executor the listener is called on; none: the thread that dispatches. */
    RequesterParams executor(java.util.concurrent.ExecutorService executor);

    /** The executor of {@link #executor(java.util.concurrent.ExecutorService)}, or {@code null}. */
    java.util.concurrent.ExecutorService executor();

    default RequesterParams withExecutor(java.util.concurrent.ExecutorService executor) {
        return executor(executor);
    }

    default <TRep> RequesterParams simpleRequesterListener(SimpleRequesterListener<TRep> listener) {
        return listener(listener);
    }

    default <TReq, TRep> RequesterParams requesterListener(RequesterListener<TReq, TRep> listener) {
        return listener(listener);
    }

    /** The listener if it is a {@link SimpleRequesterListener}, else {@code null}. */
    default java.util.EventListener getSimpleRequesterListener() {
        return listener() instanceof SimpleRequesterListener ? listener() : null;
    }

    /** The listener if it is a {@link RequesterListener}, else {@code null}. */
    default java.util.EventListener getRequesterListener() {
        return listener() instanceof RequesterListener ? listener() : null;
    }

    /** A copy: later changes to either do not reach the other. */
    RequesterParams clone();
}
