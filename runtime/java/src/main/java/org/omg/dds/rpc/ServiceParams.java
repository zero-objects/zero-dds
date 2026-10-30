// SPDX-License-Identifier: Apache-2.0
package org.omg.dds.rpc;

/**
 * DDS-RPC 1.0 §7.4.2.3: "The ServiceParams, ClientParams, RequesterParams,
 * and ReplierParams classes provide functions to specify the service name
 * and the topic name suffix." ServiceParams is the container of the
 * configuration of a function-call service that does not depend on the
 * interface: where it runs, what it is called and how its endpoints are
 * set up. The interface-dependent part (topic and type names, codec) comes
 * from the generated {@code <I>Support}.
 *
 * <p>Every setter returns the params, so a call chain names each parameter
 * at the place it is set. {@link #create()} and
 * {@link RPCRuntime#createServiceParams()} make one.
 */
public interface ServiceParams {

    /** An empty params object. */
    static ServiceParams create() {
        return new org.zerodds.ddsrpc.ServiceParamsImpl();
    }

    /** The participant the endpoints are created from. */
    ServiceParams domainParticipant(DomainParticipant participant);

    /** The {@code <service_name>} segment of the topic names (§7.4.1), {@code ""} for the default. */
    ServiceParams serviceName(String name);

    /** The name this service instance announces as {@code service_instance_name} (§7.6.2.1.1), {@code ""} for the default instance. */
    ServiceParams instanceName(String name);

    /** The whole name of the request topic (§7.4.2.2); {@code ""} for the composed name. */
    ServiceParams requestTopicName(String name);

    /** The whole name of the reply topic (§7.4.2.2); {@code ""} for the composed name. */
    ServiceParams replyTopicName(String name);

    /** The run-time suffix of the request topic name, §7.4.2.3. */
    ServiceParams requestTopicNameSuffix(String suffix);

    /** The run-time suffix of the reply topic name, §7.4.2.3. */
    ServiceParams replyTopicNameSuffix(String suffix);

    /** The QoS of the endpoints on the request topic (§7.10, default §7.10.2). */
    ServiceParams requestQos(RpcQos qos);

    /** The QoS of the endpoints on the reply topic. */
    ServiceParams replyQos(RpcQos qos);

    /** Which profile the endpoints speak; default {@link ServiceMapping#BASIC}. */
    ServiceParams mapping(ServiceMapping mapping);

    /** The §7.6.2.2 matching rule; default {@link ServiceMatching#PER_MAPPING}. */
    ServiceParams matching(ServiceMatching matching);

    /** When the listener is called; default {@link ListenerDelivery#ON_ARRIVAL}. */
    ServiceParams listenerDelivery(ListenerDelivery delivery);

    /** The CDR encoding of the samples; default {@link WireEncoding#XCDR1}. */
    ServiceParams wireEncoding(WireEncoding encoding);

    DomainParticipant domainParticipant();

    String serviceName();

    String instanceName();

    String requestTopicName();

    String replyTopicName();

    String requestTopicNameSuffix();

    String replyTopicNameSuffix();

    RpcQos requestQos();

    RpcQos replyQos();

    ServiceMapping mapping();

    ServiceMatching matching();

    ListenerDelivery listenerDelivery();

    WireEncoding wireEncoding();

    /**
     * Put these settings into the params of a service's endpoint of one
     * interface. The participant is only taken when {@code target} has
     * none, so a params object that was already bound to a participant
     * keeps it.
     *
     * @param target the params to fill
     * @return {@code target}
     */
    default ReplierParams applyTo(ReplierParams target) {
        if (target.domainParticipant() == null) {
            target.domainParticipant(domainParticipant());
        }
        if (!serviceName().isEmpty()) {
            target.serviceName(serviceName());
        }
        if (!instanceName().isEmpty()) {
            target.instanceName(instanceName());
        }
        if (!requestTopicName().isEmpty()) {
            target.requestTopicName(requestTopicName());
        }
        if (!replyTopicName().isEmpty()) {
            target.replyTopicName(replyTopicName());
        }
        if (!requestTopicNameSuffix().isEmpty()) {
            target.requestTopicNameSuffix(requestTopicNameSuffix());
        }
        if (!replyTopicNameSuffix().isEmpty()) {
            target.replyTopicNameSuffix(replyTopicNameSuffix());
        }
        return target.requestQos(requestQos())
                .replyQos(replyQos())
                .mapping(mapping())
                .matching(matching())
                .listenerDelivery(listenerDelivery())
                .wireEncoding(wireEncoding())
                .withPublisher(getPublisher())
                .withSubscriber(getSubscriber())
                .withDataWriterQos(getDataWriterQos())
                .withDataReaderQos(getDataReaderQos());
    }

    // ---- the spelling of omg-dds-rpc-java.zip (DDS-RPC 1.0 §7.11.2) ----

    default ServiceParams withDomainParticipant(org.omg.dds.domain.DomainParticipant participant) {
        return domainParticipant(new org.zerodds.ddsrpc.ZeroDdsParticipant(participant));
    }

    /** The DDS-Java-PSM participant, or {@code null} when the participant is not one of the PSM. */
    default org.omg.dds.domain.DomainParticipant getDomainParticipant() {
        DomainParticipant bound = domainParticipant();
        return bound instanceof org.zerodds.ddsrpc.ZeroDdsParticipant
                ? ((org.zerodds.ddsrpc.ZeroDdsParticipant) bound).psm()
                : null;
    }

    default ServiceParams withServiceName(String name) {
        return serviceName(name);
    }

    default String getServiceName() {
        return serviceName();
    }

    default ServiceParams withInstanceName(String name) {
        return instanceName(name);
    }

    default String getInstanceName() {
        return instanceName();
    }

    default ServiceParams withRequestTopicName(String name) {
        return requestTopicName(name);
    }

    default ServiceParams withReplyTopicName(String name) {
        return replyTopicName(name);
    }

    /** The spelling the Java file of the specification gives {@link #withReplyTopicName}. */
    default ServiceParams withreplyTopicName(String name) {
        return replyTopicName(name);
    }

    default String getRequestTopicName() {
        return requestTopicName();
    }

    default String getReplyTopicName() {
        return replyTopicName();
    }

    /**
     * The QoS the reply DataWriter of each of the service's endpoints is created with (§7.11.2); see
     * {@link ReplierParams#withDataWriterQos}. {@code null} (the default): the one
     * {@link #replyQos} gives.
     */
    ServiceParams withDataWriterQos(org.omg.dds.pub.DataWriterQos qos);

    /** The QoS the request DataReader is created with; see {@link #withDataWriterQos}. */
    ServiceParams withDataReaderQos(org.omg.dds.sub.DataReaderQos qos);

    /** The Publisher the reply DataWriter is created in; {@code null}: one the binding creates. */
    ServiceParams withPublisher(org.omg.dds.pub.Publisher publisher);

    /** The Subscriber the request DataReader is created in; {@code null}: one the binding creates. */
    ServiceParams withSubscriber(org.omg.dds.sub.Subscriber subscriber);

    /** The QoS set with {@link #withDataWriterQos}, or {@code null}. */
    org.omg.dds.pub.DataWriterQos getDataWriterQos();

    /** The QoS set with {@link #withDataReaderQos}, or {@code null}. */
    org.omg.dds.sub.DataReaderQos getDataReaderQos();

    /** The Publisher set with {@link #withPublisher}, or {@code null}. */
    org.omg.dds.pub.Publisher getPublisher();

    /** The Subscriber set with {@link #withSubscriber}, or {@code null}. */
    org.omg.dds.sub.Subscriber getSubscriber();

    /** A copy: later changes to either do not reach the other. */
    ServiceParams clone();
}
