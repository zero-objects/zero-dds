// SPDX-License-Identifier: Apache-2.0
package org.omg.dds.rpc;

/**
 * DDS-RPC 1.0 §7.11.1.4.12 through §7.11.2.2.1: "ReplierParams is a valuetype
 * that serves as a container of configuration parameters of a Replier. It is
 * designed to mimic the named-parameters feature available in some
 * programming languages, which improves readability."
 *
 * <p>An interface, as §7.11.2.2.1 lists it among the interfaces of the
 * binding; {@link #create()} and {@link RPCRuntime#createReplierParams()} make
 * the implementation. Every setter returns the params, so a call chain
 * names each parameter at the place it is set.
 */
public interface ReplierParams {

    /** An empty params object. */
    static ReplierParams create() {
        return new org.zerodds.ddsrpc.ReplierParamsImpl();
    }

    /** The participant the endpoints are created from. */
    ReplierParams domainParticipant(DomainParticipant participant);

    /**
     * The {@code <service_name>} segment of the topic names (§7.4.1),
     * {@code ""} for the default {@code "Service"}. It is not the name of
     * a service instance; that is {@link #instanceName}.
     */
    ReplierParams serviceName(String name);

    /**
     * The name this service instance announces as {@code service_instance_name} (§7.6.2.1.1), {@code ""} for the default instance. A request bound to another instance is not answered (§7.11.1.4.3).
     */
    ReplierParams instanceName(String name);

    ReplierParams requestTopicName(String name);

    ReplierParams replyTopicName(String name);

    /**
     * The {@code <service_name>} an annotation on the types gives
     * ({@code @service}, {@code @DDSService}; §7.4.1, §7.4.2.2). The name
     * set with {@link #serviceName} takes precedence over it (§7.4.2.3).
     */
    ReplierParams annotatedServiceName(String name);

    /**
     * The interface name that stands in front of the topic names when a
     * name is synthesised (§7.4.1), {@code ""} for the request-reply style,
     * which has none.
     */
    ReplierParams interfaceName(String name);

    /** The run-time suffix of the request topic name, §7.4.2.3. */
    ReplierParams requestTopicNameSuffix(String suffix);

    /** The run-time suffix of the reply topic name, §7.4.2.3. */
    ReplierParams replyTopicNameSuffix(String suffix);

    /**
     * The qualified names of the base interfaces (§7.4.3): each gives one alias of the request
     * and one of the reply topic, obtained by applying §7.4.1 to the base. The aliases are
     * announced in discovery (PID_TOPIC_ALIASES) so that an endpoint of a base interface
     * matches. None under the request-reply style, whose names carry no interface segment.
     */
    ReplierParams parents(java.util.List<String> baseInterfaces);

    /** The DDS type name of the request topic, §7.5.1.1.6. */
    ReplierParams requestTypeName(String name);

    /** The DDS type name of the reply topic, §7.5.1.1.7. */
    ReplierParams replyTypeName(String name);

    /**
     * The XTypes descriptions of the request and the reply type (§7.5.1.2, §7.7.2.5): the
     * endpoints announce them and serve their objects. {@code null} for none; peers then match
     * the topic by its type name.
     */
    ReplierParams typeDescriptions(org.zerodds.ddsrpc.TypeDescription request, org.zerodds.ddsrpc.TypeDescription reply);

    /** The listener that is called when a request has arrived (§7.11.1.4.7, §7.11.1.4.8). */
    ReplierParams listener(ListenerBase listener);

    ReplierParams requestTypeSupport(TypeSupport<?> support);

    ReplierParams replyTypeSupport(TypeSupport<?> support);

    /** The QoS of the endpoints on the request topic (§7.10, default §7.10.2). */
    ReplierParams requestQos(RpcQos qos);

    /** The QoS of the endpoints on the reply topic. */
    ReplierParams replyQos(RpcQos qos);

    /**
     * The two {@code @Qos} URLs of §7.10.1, {@code RequestProfile} and
     * {@code ReplyProfile}, resolved with {@link RpcQos#fromUrl}. Either may
     * be {@code null} and then leaves the QoS of its topic alone.
     */
    ReplierParams qosProfiles(String requestProfile, String replyProfile);

    /** Which profile the endpoints speak; default {@link ServiceMapping#BASIC}. */
    ReplierParams mapping(ServiceMapping mapping);

    /** The §7.6.2.2 matching rule; default {@link ServiceMatching#PER_MAPPING}. */
    ReplierParams matching(ServiceMatching matching);

    /** Whether replies must carry §7.8.2's parameter; default {@link RelatedIdentity#REQUIRED}. */
    ReplierParams relatedIdentity(RelatedIdentity policy);

    /**
     * The CDR encoding of the samples; default {@link WireEncoding#XCDR1}
     * (§7.5.1.1: DDS 1.3 without XTypes), {@link WireEncoding#XCDR2} is the
     * named alternative. Both ends of a topic pair must agree.
     */
    ReplierParams wireEncoding(WireEncoding encoding);

    /** When the listener is called; default {@link ListenerDelivery#ON_ARRIVAL}. */
    ReplierParams listenerDelivery(ListenerDelivery delivery);

    /**
     * Takes the settings that are not about one interface — the mapping, the
     * matching rule, the related-identity policy, when the listener is
     * called, the Publisher, the Subscriber and the two PSM QoS — from
     * {@code other}. A function-call replier of a derived
     * interface creates one for each base interface and gives it the
     * settings of its own.
     */
    default ReplierParams sameSettingsAs(ReplierParams other) {
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

    default ReplierParams withDomainParticipant(org.omg.dds.domain.DomainParticipant participant) {
        return domainParticipant(new org.zerodds.ddsrpc.ZeroDdsParticipant(participant));
    }

    /** The DDS-Java-PSM participant, or {@code null} when the participant is not one of the PSM. */
    default org.omg.dds.domain.DomainParticipant getDomainParticipant() {
        DomainParticipant bound = domainParticipant();
        return bound instanceof org.zerodds.ddsrpc.ZeroDdsParticipant
                ? ((org.zerodds.ddsrpc.ZeroDdsParticipant) bound).psm()
                : null;
    }

    default ReplierParams withServiceName(String name) {
        return serviceName(name);
    }

    default String getServiceName() {
        return serviceName();
    }

    default ReplierParams withInstanceName(String name) {
        return instanceName(name);
    }

    default String getInstanceName() {
        return instanceName();
    }

    default ReplierParams withRequestTopicName(String name) {
        return requestTopicName(name);
    }

    default ReplierParams withReplyTopicName(String name) {
        return replyTopicName(name);
    }

    /** The spelling the Java file of the specification gives {@link #withReplyTopicName}. */
    default ReplierParams withreplyTopicName(String name) {
        return replyTopicName(name);
    }

    default String getRequestTopicName() {
        return requestTopicName();
    }

    default String getReplyTopicName() {
        return replyTopicName();
    }

    /**
     * The QoS the reply DataWriter is created with (§7.11.2). Set, it is the writer's QoS as
     * it stands, and {@link #replyQos} is derived from it; {@code null} (the default): the
     * writer's QoS is the one {@link #replyQos} gives.
     */
    ReplierParams withDataWriterQos(org.omg.dds.pub.DataWriterQos qos);

    /**
     * The QoS the request DataReader is created with; see {@link #withDataWriterQos}, with
     * {@link #requestQos} derived from it.
     */
    ReplierParams withDataReaderQos(org.omg.dds.sub.DataReaderQos qos);

    /**
     * The Publisher the reply DataWriter is created in, a Publisher of the participant;
     * {@code null} (the default): one the binding creates.
     */
    ReplierParams withPublisher(org.omg.dds.pub.Publisher publisher);

    /** The Subscriber the request DataReader is created in; see {@link #withPublisher}. */
    ReplierParams withSubscriber(org.omg.dds.sub.Subscriber subscriber);

    /** The QoS set with {@link #withDataWriterQos}, or {@code null}. */
    org.omg.dds.pub.DataWriterQos getDataWriterQos();

    /** The QoS set with {@link #withDataReaderQos}, or {@code null}. */
    org.omg.dds.sub.DataReaderQos getDataReaderQos();

    /** The Publisher set with {@link #withPublisher}, or {@code null}. */
    org.omg.dds.pub.Publisher getPublisher();

    /** The Subscriber set with {@link #withSubscriber}, or {@code null}. */
    org.omg.dds.sub.Subscriber getSubscriber();

    default <TReq, TRep> ReplierParams simpleReplierListener(SimpleReplierListener<TReq, TRep> listener) {
        return listener(listener);
    }

    default <TReq, TRep> ReplierParams replierListener(ReplierListener<TReq, TRep> listener) {
        return listener(listener);
    }

    /** The listener if it is a {@link SimpleReplierListener}, else {@code null}. */
    default java.util.EventListener getSimpleRequesterListener() {
        return listener() instanceof SimpleReplierListener ? listener() : null;
    }

    /** The listener if it is a {@link ReplierListener}, else {@code null}. */
    default java.util.EventListener getRequesterListener() {
        return listener() instanceof ReplierListener ? listener() : null;
    }

    /** A copy: later changes to either do not reach the other. */
    ReplierParams clone();
}
