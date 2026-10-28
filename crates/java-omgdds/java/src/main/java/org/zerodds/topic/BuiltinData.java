// SPDX-License-Identifier: Apache-2.0
package org.zerodds.topic;

import org.omg.dds.core.ServiceEnvironment;
import org.omg.dds.pub.DataWriter;
import org.omg.dds.pub.DataWriterQos;
import org.omg.dds.pub.Publisher;
import org.omg.dds.pub.PublisherQos;
import org.omg.dds.sub.DataReader;
import org.omg.dds.sub.DataReaderQos;
import org.omg.dds.sub.Subscriber;
import org.omg.dds.sub.SubscriberQos;
import org.omg.dds.topic.BuiltinTopicKey;
import org.omg.dds.topic.Topic;
import org.omg.dds.topic.TopicQos;

/**
 * The built-in topic data (DDS 1.4 §2.2.5) of a living entity: every member
 * of the structure, taken from the entity's own QoS and from the QoS of the
 * Publisher or Subscriber and the Topic it belongs to (PRESENTATION,
 * PARTITION and GROUP_DATA are policies of the Publisher/Subscriber,
 * TOPIC_DATA one of the Topic).
 */
public final class BuiltinData {

    private BuiltinData() {}

    /**
     * What the type support says about the type for the built-in topic data (XTypes 1.3
     * §7.6.3.2.2 and DDS-Java-PSM 1.0): TypeInformation and the names of the base and
     * equivalent types. The XTypes 1.1 TypeObject ({@code type}) is produced from the DynamicType of
     * the type where the type support has one.
     */
    private static void describeType(ServiceEnvironment env, Object support, Object data) {
        if (!(support instanceof org.zerodds.cdr.TopicTypeSupport)) {
            return;
        }
        org.zerodds.cdr.TopicTypeSupport<?> typed = (org.zerodds.cdr.TopicTypeSupport<?>) support;
        byte[] information = typed.typeInformation();
        byte[] typeObject = org.zerodds.typeobject.TypeObjects.announce(env, support);
        java.util.List<String> base = typed.baseTypeNames();
        java.util.List<String> equivalent = typed.equivalentTypeNames();
        if (data instanceof TopicBuiltinTopicDataImpl) {
            TopicBuiltinTopicDataImpl d = (TopicBuiltinTopicDataImpl) data;
            d.setTypeInformation(information);
            d.setTypeObjectBytes(typeObject);
            d.setBaseTypeName(base.isEmpty() ? null : base);
            d.setEquivalentTypeName(equivalent.isEmpty() ? null : equivalent);
        } else if (data instanceof PublicationBuiltinTopicDataImpl) {
            PublicationBuiltinTopicDataImpl d = (PublicationBuiltinTopicDataImpl) data;
            d.setTypeInformation(information);
            d.setTypeObjectBytes(typeObject);
            d.setBaseTypeName(base.isEmpty() ? null : base);
            d.setEquivalentTypeName(equivalent.isEmpty() ? null : equivalent);
        } else {
            SubscriptionBuiltinTopicDataImpl d = (SubscriptionBuiltinTopicDataImpl) data;
            d.setTypeInformation(information);
            d.setTypeObjectBytes(typeObject);
            d.setBaseTypeName(base.isEmpty() ? null : base);
            d.setEquivalentTypeName(equivalent.isEmpty() ? null : equivalent);
        }
    }

    /** The data of {@code topic}. */
    public static TopicBuiltinTopicDataImpl topic(ServiceEnvironment env, Topic<?> topic) {
        TopicQos qos = topic.getQos();
        TopicBuiltinTopicDataImpl data = new TopicBuiltinTopicDataImpl(env);
        data.setKey(BuiltinTopicKeyImpl.of(env, topic.getInstanceHandle()));
        data.setName(topic.getName());
        data.setTypeName(topic.getTypeName());
        data.setDurability(qos.getDurability());
        data.setDurabilityService(qos.getDurabilityService());
        data.setDeadline(qos.getDeadline());
        data.setLatencyBudget(qos.getLatencyBudget());
        data.setLiveliness(qos.getLiveliness());
        data.setReliability(qos.getReliability());
        data.setTransportPriority(qos.getTransportPriority());
        data.setLifespan(qos.getLifespan());
        data.setDestinationOrder(qos.getDestinationOrder());
        data.setHistory(qos.getHistory());
        data.setResourceLimits(qos.getResourceLimits());
        data.setOwnership(qos.getOwnership());
        data.setTopicData(qos.getTopicData());
        data.setRepresentation(qos.getRepresentation());
        data.setTypeConsistency(qos.getTypeConsistency());
        if (topic instanceof org.zerodds.topic.TopicImpl) {
            describeType(env, ((org.zerodds.topic.TopicImpl<?>) topic).getTypeSupport(), data);
        }
        return data;
    }

    /** The data of {@code writer}, which belongs to {@code parent} (may be {@code null}). */
    public static PublicationBuiltinTopicDataImpl publication(ServiceEnvironment env,
            DataWriter<?> writer, Publisher parent, BuiltinTopicKey participantKey) {
        PublisherQos parentQos = parent == null ? null : parent.getQos();
        DataWriterQos qos = writer.getQos();
        PublicationBuiltinTopicDataImpl data = new PublicationBuiltinTopicDataImpl(env);
        data.setKey(BuiltinTopicKeyImpl.of(env, writer.getInstanceHandle()));
        data.setParticipantKey(participantKey);
        data.setTopicName(writer.getTopic().getName());
        data.setTypeName(writer.getTopic().getTypeName());
        data.setDurability(qos.getDurability());
        data.setDurabilityService(qos.getDurabilityService());
        data.setDeadline(qos.getDeadline());
        data.setLatencyBudget(qos.getLatencyBudget());
        data.setLiveliness(qos.getLiveliness());
        data.setReliability(qos.getReliability());
        data.setLifespan(qos.getLifespan());
        data.setUserData(qos.getUserData());
        data.setOwnership(qos.getOwnership());
        data.setOwnershipStrength(qos.getOwnershipStrength());
        data.setDestinationOrder(qos.getDestinationOrder());
        data.setRepresentation(qos.getRepresentation());
        data.setTypeConsistency(qos.getTypeConsistency());
        data.setTopicData(writer.getTopic().getQos().getTopicData());
        if (writer instanceof org.zerodds.pub.DataWriterImpl) {
            describeType(env, ((org.zerodds.pub.DataWriterImpl<?>) writer).typeSupport(), data);
        }
        if (parentQos != null) {
            data.setPresentation(parentQos.getPresentation());
            data.setPartition(parentQos.getPartition());
            data.setGroupData(parentQos.getGroupData());
        }
        return data;
    }

    /** The data of {@code reader}, which belongs to {@code parent} (may be {@code null}). */
    public static SubscriptionBuiltinTopicDataImpl subscription(ServiceEnvironment env,
            DataReader<?> reader, Subscriber parent, BuiltinTopicKey participantKey) {
        SubscriberQos parentQos = parent == null ? null : parent.getQos();
        return subscription(env, reader, parentQos, participantKey);
    }

    /** As above, with the QoS of the Subscriber given. */
    public static SubscriptionBuiltinTopicDataImpl subscription(ServiceEnvironment env,
            DataReader<?> reader, SubscriberQos parentQos, BuiltinTopicKey participantKey) {
        DataReaderQos qos = reader.getQos();
        SubscriptionBuiltinTopicDataImpl data = new SubscriptionBuiltinTopicDataImpl(env);
        data.setKey(BuiltinTopicKeyImpl.of(env, reader.getInstanceHandle()));
        data.setParticipantKey(participantKey);
        data.setTopicName(reader.getTopicDescription().getName());
        data.setTypeName(reader.getTopicDescription().getTypeName());
        data.setDurability(qos.getDurability());
        data.setDeadline(qos.getDeadline());
        data.setLatencyBudget(qos.getLatencyBudget());
        data.setLiveliness(qos.getLiveliness());
        // §7.3.5.2: RELIABILITY is an entry of the QoS map, not an accessor.
        data.setReliability(qos.get(org.omg.dds.core.policy.Reliability.class));
        data.setOwnership(qos.getOwnership());
        data.setDestinationOrder(qos.getDestinationOrder());
        data.setUserData(qos.getUserData());
        data.setTimeBasedFilter(qos.getTimeBasedFilter());
        data.setRepresentation(qos.getRepresentation());
        data.setTypeConsistency(qos.getTypeConsistency());
        if (reader instanceof org.zerodds.sub.DataReaderImpl) {
            describeType(env, ((org.zerodds.sub.DataReaderImpl<?>) reader).typeSupport(), data);
        }
        if (reader.getTopicDescription() instanceof Topic) {
            data.setTopicData(((Topic<?>) reader.getTopicDescription()).getQos().getTopicData());
        }
        if (parentQos != null) {
            data.setPresentation(parentQos.getPresentation());
            data.setPartition(parentQos.getPartition());
            data.setGroupData(parentQos.getGroupData());
        }
        return data;
    }

    // --- The data of a remote entity, as SPDP and SEDP announced it ---------

    /** The key of a 16-octet GUID: four big-endian words. */
    public static BuiltinTopicKeyImpl keyOfGuid(ServiceEnvironment env, byte[] guid) {
        int[] words = new int[4];
        for (int word = 0; word < 4; word++) {
            int value = 0;
            for (int b = 0; b < 4; b++) {
                value = (value << 8) | (guid[word * 4 + b] & 0xFF);
            }
            words[word] = value;
        }
        return new BuiltinTopicKeyImpl(env, words);
    }

    /** The GUID of the participant with {@code prefix}. */
    public static byte[] participantGuid(byte[] prefix) {
        return new org.zerodds.rtps.Guid(prefix, org.zerodds.rtps.Guid.ENTITYID_PARTICIPANT).bytes();
    }

    /** The data of a participant discovered through SPDP. */
    public static ParticipantBuiltinTopicDataImpl participant(ServiceEnvironment env,
            org.zerodds.rtps.DiscoveryData.Participant participant) {
        ParticipantBuiltinTopicDataImpl data = new ParticipantBuiltinTopicDataImpl(env,
                keyOfGuid(env, participantGuid(participant.guidPrefix)), null);
        org.zerodds.rtps.QosParameters q = new org.zerodds.rtps.QosParameters();
        q.userData = participant.userData;
        BuiltinQos.applyTo(data, q, env);
        return data;
    }

    /** The data of a DataWriter discovered through SEDP. */
    public static PublicationBuiltinTopicDataImpl publication(ServiceEnvironment env,
            org.zerodds.rtps.DiscoveryData.Endpoint endpoint) {
        PublicationBuiltinTopicDataImpl data = new PublicationBuiltinTopicDataImpl(env);
        data.setKey(keyOfGuid(env, endpoint.endpointGuid));
        data.setParticipantKey(keyOfGuid(env, participantGuid(endpoint.participantPrefix == null
                ? java.util.Arrays.copyOf(endpoint.endpointGuid, 12) : endpoint.participantPrefix)));
        data.setTopicName(endpoint.topicName);
        data.setTypeName(endpoint.typeName);
        BuiltinQos.applyTo(data, endpoint.qos, env);
        return data;
    }

    /** The data of a DataReader discovered through SEDP. */
    public static SubscriptionBuiltinTopicDataImpl subscription(ServiceEnvironment env,
            org.zerodds.rtps.DiscoveryData.Endpoint endpoint) {
        SubscriptionBuiltinTopicDataImpl data = new SubscriptionBuiltinTopicDataImpl(env);
        data.setKey(keyOfGuid(env, endpoint.endpointGuid));
        data.setParticipantKey(keyOfGuid(env, participantGuid(endpoint.participantPrefix == null
                ? java.util.Arrays.copyOf(endpoint.endpointGuid, 12) : endpoint.participantPrefix)));
        data.setTopicName(endpoint.topicName);
        data.setTypeName(endpoint.typeName);
        BuiltinQos.applyTo(data, endpoint.qos, env);
        return data;
    }
}
