// SPDX-License-Identifier: Apache-2.0
package org.zerodds.ddsrpc;

import org.omg.dds.rpc.*;
/** The implementation of {@link RequesterParams}. */
public final class RequesterParamsImpl implements RequesterParams, Cloneable {

    private DomainParticipant participant;
    private String serviceName = "";
    private String instanceName = "";
    private String requestTopicName = "";
    private String replyTopicName = "";
    private String annotatedServiceName = "";
    private String interfaceName = "";
    private String requestTopicNameSuffix = "";
    private String replyTopicNameSuffix = "";
    private java.util.List<String> parents = java.util.Collections.emptyList();
    private String requestTypeName = "";
    private String replyTypeName = "";
    private TypeDescription requestTypeDescription;
    private TypeDescription replyTypeDescription;
    private ListenerBase listener;
    private java.util.concurrent.ExecutorService executor;
    private TypeSupport<?> requestSupport;
    private TypeSupport<?> replySupport;
    private org.omg.dds.pub.DataWriterQos dataWriterQos;
    private org.omg.dds.sub.DataReaderQos dataReaderQos;
    private org.omg.dds.pub.Publisher publisher;
    private org.omg.dds.sub.Subscriber subscriber;
    private RpcQos requestQos = RpcQos.defaults();
    private RpcQos replyQos = RpcQos.defaults();
    private ServiceMapping mapping = ServiceMapping.BASIC;
    private ServiceMatching matching = ServiceMatching.PER_MAPPING;
    private RelatedIdentity relatedIdentity = RelatedIdentity.REQUIRED;
    private ListenerDelivery listenerDelivery = ListenerDelivery.ON_ARRIVAL;
    private WireEncoding wireEncoding = WireEncoding.XCDR1;

    @Override
    public RequesterParams domainParticipant(DomainParticipant participant) {
        this.participant = participant;
        return this;
    }

    @Override
    public RequesterParams serviceName(String name) {
        this.serviceName = name == null ? "" : name;
        return this;
    }

    @Override
    public RequesterParams instanceName(String name) {
        this.instanceName = name == null ? "" : name;
        return this;
    }

    @Override
    public RequesterParams requestTopicName(String name) {
        this.requestTopicName = name == null ? "" : name;
        return this;
    }

    @Override
    public RequesterParams replyTopicName(String name) {
        this.replyTopicName = name == null ? "" : name;
        return this;
    }

    @Override
    public RequesterParams parents(java.util.List<String> baseInterfaces) {
        this.parents = baseInterfaces == null
                ? java.util.Collections.<String>emptyList()
                : java.util.Collections.unmodifiableList(new java.util.ArrayList<>(baseInterfaces));
        return this;
    }

    @Override
    public java.util.List<String> parents() {
        return parents;
    }

    @Override
    public RequesterParams requestTypeName(String name) {
        this.requestTypeName = name == null ? "" : name;
        return this;
    }

    @Override
    public RequesterParams replyTypeName(String name) {
        this.replyTypeName = name == null ? "" : name;
        return this;
    }

    @Override
    public RequesterParams listener(ListenerBase listener) {
        this.listener = listener;
        return this;
    }

    @Override
    public RequesterParams requestTypeSupport(TypeSupport<?> support) {
        this.requestSupport = support;
        return this;
    }

    @Override
    public RequesterParams replyTypeSupport(TypeSupport<?> support) {
        this.replySupport = support;
        return this;
    }

    @Override
    public RequesterParams requestQos(RpcQos qos) {
        this.requestQos = java.util.Objects.requireNonNull(qos);
        return this;
    }

    @Override
    public RequesterParams replyQos(RpcQos qos) {
        this.replyQos = java.util.Objects.requireNonNull(qos);
        return this;
    }

    @Override
    public RequesterParams qosProfiles(String requestProfile, String replyProfile) {
        if (requestProfile != null) {
            this.requestQos = RpcQos.fromUrl(requestProfile);
        }
        if (replyProfile != null) {
            this.replyQos = RpcQos.fromUrl(replyProfile);
        }
        return this;
    }

    @Override
    public RequesterParams mapping(ServiceMapping mapping) {
        this.mapping = java.util.Objects.requireNonNull(mapping);
        return this;
    }

    @Override
    public RequesterParams matching(ServiceMatching matching) {
        this.matching = java.util.Objects.requireNonNull(matching);
        return this;
    }

    @Override
    public RequesterParams relatedIdentity(RelatedIdentity policy) {
        this.relatedIdentity = java.util.Objects.requireNonNull(policy);
        return this;
    }

    @Override
    public RequesterParams listenerDelivery(ListenerDelivery delivery) {
        this.listenerDelivery = java.util.Objects.requireNonNull(delivery);
        return this;
    }

    @Override
    public RequesterParams wireEncoding(WireEncoding encoding) {
        this.wireEncoding = java.util.Objects.requireNonNull(encoding);
        return this;
    }

    @Override
    public RequesterParams annotatedServiceName(String name) {
        this.annotatedServiceName = name == null ? "" : name;
        return this;
    }

    @Override
    public RequesterParams interfaceName(String name) {
        this.interfaceName = name == null ? "" : name;
        return this;
    }

    @Override
    public RequesterParams requestTopicNameSuffix(String suffix) {
        this.requestTopicNameSuffix = suffix == null ? "" : suffix;
        return this;
    }

    @Override
    public RequesterParams replyTopicNameSuffix(String suffix) {
        this.replyTopicNameSuffix = suffix == null ? "" : suffix;
        return this;
    }

    @Override
    public String annotatedServiceName() {
        return annotatedServiceName;
    }

    @Override
    public String interfaceName() {
        return interfaceName;
    }

    @Override
    public String requestTopicNameSuffix() {
        return requestTopicNameSuffix;
    }

    @Override
    public String replyTopicNameSuffix() {
        return replyTopicNameSuffix;
    }

    @Override
    public DomainParticipant domainParticipant() {
        return participant;
    }

    @Override
    public String serviceName() {
        return serviceName;
    }

    @Override
    public String instanceName() {
        return instanceName;
    }

    @Override
    public String requestTopicName() {
        return requestTopicName;
    }

    @Override
    public String replyTopicName() {
        return replyTopicName;
    }

    @Override
    public String requestTypeName() {
        return requestTypeName;
    }

    @Override
    public RequesterParams typeDescriptions(TypeDescription request, TypeDescription reply) {
        this.requestTypeDescription = request;
        this.replyTypeDescription = reply;
        return this;
    }

    @Override
    public TypeDescription requestTypeDescription() {
        return requestTypeDescription;
    }

    @Override
    public TypeDescription replyTypeDescription() {
        return replyTypeDescription;
    }

    @Override
    public String replyTypeName() {
        return replyTypeName;
    }

    @Override
    public ListenerBase listener() {
        return listener;
    }

    @Override
    public TypeSupport<?> requestTypeSupport() {
        return requestSupport;
    }

    @Override
    public TypeSupport<?> replyTypeSupport() {
        return replySupport;
    }

    @Override
    public RpcQos requestQos() {
        return requestQos;
    }

    @Override
    public RpcQos replyQos() {
        return replyQos;
    }

    @Override
    public ServiceMapping mapping() {
        return mapping;
    }

    @Override
    public ServiceMatching matching() {
        return matching;
    }

    @Override
    public RelatedIdentity relatedIdentity() {
        return relatedIdentity;
    }

    @Override
    public ListenerDelivery listenerDelivery() {
        return listenerDelivery;
    }

    @Override
    public WireEncoding wireEncoding() {
        return wireEncoding;
    }

    @Override
    public RequesterParams clone() {
        try {
            return (RequesterParams) super.clone();
        } catch (CloneNotSupportedException impossible) {
            throw new AssertionError(impossible);
        }
    }

    @Override
    public RequesterParams executor(java.util.concurrent.ExecutorService executor) {
        this.executor = executor;
        return this;
    }

    @Override
    public java.util.concurrent.ExecutorService executor() {
        return executor;
    }

    @Override
    public RequesterParams withDataWriterQos(org.omg.dds.pub.DataWriterQos qos) {
        this.dataWriterQos = qos;
        return this;
    }

    @Override
    public RequesterParams withDataReaderQos(org.omg.dds.sub.DataReaderQos qos) {
        this.dataReaderQos = qos;
        return this;
    }

    @Override
    public RequesterParams withPublisher(org.omg.dds.pub.Publisher publisher) {
        this.publisher = publisher;
        return this;
    }

    @Override
    public RequesterParams withSubscriber(org.omg.dds.sub.Subscriber subscriber) {
        this.subscriber = subscriber;
        return this;
    }

    @Override
    public org.omg.dds.pub.DataWriterQos getDataWriterQos() {
        return dataWriterQos;
    }

    @Override
    public org.omg.dds.sub.DataReaderQos getDataReaderQos() {
        return dataReaderQos;
    }

    @Override
    public org.omg.dds.pub.Publisher getPublisher() {
        return publisher;
    }

    @Override
    public org.omg.dds.sub.Subscriber getSubscriber() {
        return subscriber;
    }
}
