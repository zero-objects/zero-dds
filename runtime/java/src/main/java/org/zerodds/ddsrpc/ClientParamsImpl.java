// SPDX-License-Identifier: Apache-2.0
package org.zerodds.ddsrpc;

import org.omg.dds.rpc.*;
import java.util.Objects;

/** The implementation of {@link ClientParams}. */
public final class ClientParamsImpl implements ClientParams, Cloneable {

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
    public ClientParams domainParticipant(DomainParticipant participant) {
        this.participant = participant;
        return this;
    }

    @Override
    public ClientParams serviceName(String name) {
        this.serviceName = name == null ? "" : name;
        return this;
    }

    @Override
    public ClientParams instanceName(String name) {
        this.instanceName = name == null ? "" : name;
        return this;
    }

    @Override
    public ClientParams requestTopicName(String name) {
        this.requestTopicName = name == null ? "" : name;
        return this;
    }

    @Override
    public ClientParams replyTopicName(String name) {
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
    public ClientParams clone() {
        try {
            return (ClientParams) super.clone();
        } catch (CloneNotSupportedException impossible) {
            throw new AssertionError(impossible);
        }
    }

    @Override
    public ClientParams requestTopicNameSuffix(String suffix) {
        this.requestTopicNameSuffix = suffix == null ? "" : suffix;
        return this;
    }

    @Override
    public ClientParams replyTopicNameSuffix(String suffix) {
        this.replyTopicNameSuffix = suffix == null ? "" : suffix;
        return this;
    }

    @Override
    public ClientParams requestQos(RpcQos qos) {
        this.requestQos = Objects.requireNonNull(qos, "qos");
        return this;
    }

    @Override
    public ClientParams replyQos(RpcQos qos) {
        this.replyQos = Objects.requireNonNull(qos, "qos");
        return this;
    }

    @Override
    public ClientParams mapping(ServiceMapping mapping) {
        this.mapping = Objects.requireNonNull(mapping, "mapping");
        return this;
    }

    @Override
    public ClientParams matching(ServiceMatching matching) {
        this.matching = Objects.requireNonNull(matching, "matching");
        return this;
    }

    @Override
    public ClientParams listenerDelivery(ListenerDelivery delivery) {
        this.listenerDelivery = Objects.requireNonNull(delivery, "delivery");
        return this;
    }

    @Override
    public ClientParams wireEncoding(WireEncoding encoding) {
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
    public ClientParams withDataWriterQos(org.omg.dds.pub.DataWriterQos qos) {
        this.dataWriterQos = qos;
        return this;
    }

    @Override
    public ClientParams withDataReaderQos(org.omg.dds.sub.DataReaderQos qos) {
        this.dataReaderQos = qos;
        return this;
    }

    @Override
    public ClientParams withPublisher(org.omg.dds.pub.Publisher publisher) {
        this.publisher = publisher;
        return this;
    }

    @Override
    public ClientParams withSubscriber(org.omg.dds.sub.Subscriber subscriber) {
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
