// SPDX-License-Identifier: Apache-2.0
package org.zerodds.ddsrpc;

import org.omg.dds.rpc.*;
import java.util.Objects;

/** The implementation of {@link ServiceParams}. */
public final class ServiceParamsImpl implements ServiceParams, Cloneable {

    private org.omg.dds.pub.DataWriterQos dataWriterQos;
    private org.omg.dds.sub.DataReaderQos dataReaderQos;
    private org.omg.dds.pub.Publisher publisher;
    private org.omg.dds.sub.Subscriber subscriber;
    private DomainParticipant participant;
    private String serviceName = "";
    private String instanceName = "";
    private String requestTopicName = "";
    private String replyTopicName = "";
    private String requestTopicNameSuffix = "";
    private String replyTopicNameSuffix = "";
    private RpcQos requestQos = RpcQos.defaults();
    private RpcQos replyQos = RpcQos.defaults();
    private ServiceMapping mapping = ServiceMapping.BASIC;
    private ServiceMatching matching = ServiceMatching.PER_MAPPING;
    private ListenerDelivery listenerDelivery = ListenerDelivery.ON_ARRIVAL;
    private WireEncoding wireEncoding = WireEncoding.XCDR1;

    @Override
    public ServiceParams domainParticipant(DomainParticipant participant) {
        this.participant = participant;
        return this;
    }

    @Override
    public ServiceParams serviceName(String name) {
        this.serviceName = name == null ? "" : name;
        return this;
    }

    @Override
    public ServiceParams instanceName(String name) {
        this.instanceName = name == null ? "" : name;
        return this;
    }

    @Override
    public ServiceParams requestTopicName(String name) {
        this.requestTopicName = name == null ? "" : name;
        return this;
    }

    @Override
    public ServiceParams replyTopicName(String name) {
        this.replyTopicName = name == null ? "" : name;
        return this;
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
    public ServiceParams clone() {
        try {
            return (ServiceParams) super.clone();
        } catch (CloneNotSupportedException impossible) {
            throw new AssertionError(impossible);
        }
    }

    @Override
    public ServiceParams requestTopicNameSuffix(String suffix) {
        this.requestTopicNameSuffix = suffix == null ? "" : suffix;
        return this;
    }

    @Override
    public ServiceParams replyTopicNameSuffix(String suffix) {
        this.replyTopicNameSuffix = suffix == null ? "" : suffix;
        return this;
    }

    @Override
    public ServiceParams requestQos(RpcQos qos) {
        this.requestQos = Objects.requireNonNull(qos, "qos");
        return this;
    }

    @Override
    public ServiceParams replyQos(RpcQos qos) {
        this.replyQos = Objects.requireNonNull(qos, "qos");
        return this;
    }

    @Override
    public ServiceParams mapping(ServiceMapping mapping) {
        this.mapping = Objects.requireNonNull(mapping, "mapping");
        return this;
    }

    @Override
    public ServiceParams matching(ServiceMatching matching) {
        this.matching = Objects.requireNonNull(matching, "matching");
        return this;
    }

    @Override
    public ServiceParams listenerDelivery(ListenerDelivery delivery) {
        this.listenerDelivery = Objects.requireNonNull(delivery, "delivery");
        return this;
    }

    @Override
    public ServiceParams wireEncoding(WireEncoding encoding) {
        this.wireEncoding = Objects.requireNonNull(encoding, "encoding");
        return this;
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
    public String requestTopicNameSuffix() {
        return requestTopicNameSuffix;
    }

    @Override
    public String replyTopicNameSuffix() {
        return replyTopicNameSuffix;
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
    public ListenerDelivery listenerDelivery() {
        return listenerDelivery;
    }

    @Override
    public WireEncoding wireEncoding() {
        return wireEncoding;
    }

    @Override
    public ServiceParams withDataWriterQos(org.omg.dds.pub.DataWriterQos qos) {
        this.dataWriterQos = qos;
        return this;
    }

    @Override
    public ServiceParams withDataReaderQos(org.omg.dds.sub.DataReaderQos qos) {
        this.dataReaderQos = qos;
        return this;
    }

    @Override
    public ServiceParams withPublisher(org.omg.dds.pub.Publisher publisher) {
        this.publisher = publisher;
        return this;
    }

    @Override
    public ServiceParams withSubscriber(org.omg.dds.sub.Subscriber subscriber) {
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
