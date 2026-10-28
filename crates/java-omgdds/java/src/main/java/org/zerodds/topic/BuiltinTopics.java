// SPDX-License-Identifier: Apache-2.0
package org.zerodds.topic;

import java.util.Arrays;
import java.util.Set;

import org.omg.dds.core.ServiceEnvironment;
import org.omg.dds.type.TypeSupport;
import org.omg.dds.topic.BuiltinTopicKey;
import org.zerodds.cdr.EndianMode;
import org.zerodds.cdr.ExtensibilityKind;
import org.zerodds.cdr.TopicTypeSupport;
import org.zerodds.cdr.TypeSupportImpl;
import org.zerodds.cdr.XcdrException;

/**
 * The built-in topics of DDS 1.2 §2.2.1.3.1 / DDSI-RTPS 2.5 §9.3.1.2 and
 * the wire form of their data.
 *
 * <p>A {@link org.omg.dds.domain.DomainParticipant} creates the four topics
 * itself and writes their data with a built-in writer, so that an
 * application reading them through its built-in subscriber sees the entities
 * of the domain.
 *
 * <p>The wire form is the ParameterList of DDSI-RTPS 2.5 §9.6.2 with every
 * member of the structure of DDS 1.4 §2.2.5, the QoS policies included (see
 * {@link BuiltinTopicWire}).
 */
public final class BuiltinTopics {

    private BuiltinTopics() {}

    /** DDS 1.2 §2.2.1.3.1: the four built-in topics of a domain. */
    public static final String PARTICIPANT = "DCPSParticipant";
    public static final String PUBLICATION = "DCPSPublication";
    public static final String SUBSCRIPTION = "DCPSSubscription";
    public static final String TOPIC = "DCPSTopic";

    private static final Set<String> NAMES =
            java.util.Collections.unmodifiableSet(new java.util.HashSet<>(
                    java.util.Arrays.asList(PARTICIPANT, PUBLICATION, SUBSCRIPTION, TOPIC)));

    /** Whether {@code topicName} is one of the four built-in topics. */
    public static boolean isBuiltin(String topicName) {
        return NAMES.contains(topicName);
    }

    /**
     * Whether a topic exists only inside the process: the topic a MultiTopic
     * join writes its result samples on. Such a topic is never announced or
     * sent over RTPS.
     */
    public static boolean isInternalToProcess(String topicName) {
        return MultiTopicEngine.isInternal(topicName);
    }

    /**
     * Whether {@code type} is the Java type of the data of the built-in topic
     * {@code topicName}: the structure of DDS 1.4 §2.2.5 (the PSM interface) or
     * the class of this binding that implements it.
     */
    public static boolean isDataType(String topicName, Class<?> type) {
        Class<?> structure;
        Class<?> implementation;
        switch (topicName) {
            case PARTICIPANT:
                structure = org.omg.dds.topic.ParticipantBuiltinTopicData.class;
                implementation = ParticipantBuiltinTopicDataImpl.class;
                break;
            case PUBLICATION:
                structure = org.omg.dds.topic.PublicationBuiltinTopicData.class;
                implementation = PublicationBuiltinTopicDataImpl.class;
                break;
            case SUBSCRIPTION:
                structure = org.omg.dds.topic.SubscriptionBuiltinTopicData.class;
                implementation = SubscriptionBuiltinTopicDataImpl.class;
                break;
            case TOPIC:
                structure = org.omg.dds.topic.TopicBuiltinTopicData.class;
                implementation = TopicBuiltinTopicDataImpl.class;
                break;
            default:
                return false;
        }
        return type == structure || type == implementation;
    }

    /** The DDS type name of a built-in topic (DDSI-RTPS 2.5 §9.3.1.2). */
    public static String typeNameOf(String topicName) {
        return "DDS::" + topicName;
    }

    /**
     * The {@link TypeSupport} of a built-in topic (§7.5.1): the name the type
     * is registered under is its DDS type name, so a reader matches a writer
     * on it (DDS 1.2 §2.1.2.5).
     */
    public static <T> TypeSupport<T> typeSupportOf(
            ServiceEnvironment environment, String topicName, Class<T> dataType) {
        if (!isBuiltin(topicName)) {
            throw new IllegalArgumentException(
                    topicName + " is not a built-in topic");
        }
        return new TypeSupportImpl<>(environment, typeNameOf(topicName), dataType,
                new IdentityCodec<>(environment, topicName));
    }

    /**
     * The codec of the built-in topic data: the ParameterList form of
     * {@link BuiltinTopicWire}. The key of the DDS structure is the
     * instance key.
     */
    private static final class IdentityCodec<T> implements TopicTypeSupport<T> {

        private final ServiceEnvironment environment;
        private final String typeName;
        private final String topicName;

        IdentityCodec(ServiceEnvironment environment, String topicName) {
            this.environment = environment;
            this.topicName = topicName;
            this.typeName = typeNameOf(topicName);
        }

        @Override
        public String getTypeName() {
            return typeName;
        }

        @Override
        public boolean isKeyed() {
            return true;
        }

        @Override
        public ExtensibilityKind getExtensibility() {
            return ExtensibilityKind.MUTABLE;
        }

        @Override
        public byte[] encode(T sample) {
            return encode(sample, EndianMode.LITTLE_ENDIAN);
        }

        @Override
        public byte[] encode(T sample, EndianMode endian) {
            if (endian != EndianMode.LITTLE_ENDIAN) {
                throw new XcdrException(
                        "a built-in topic is encoded little-endian only");
            }
            if (sample == null) {
                throw new XcdrException("a built-in topic sample must not be null");
            }
            return BuiltinTopicWire.encode(sample);
        }

        @Override
        public T decode(byte[] bytes) {
            return decode(bytes, 0, bytes.length);
        }

        @Override
        @SuppressWarnings("unchecked")
        public T decode(byte[] bytes, int offset, int length) {
            return (T) BuiltinTopicWire.decode(environment, topicName, bytes, offset, length);
        }

        @Override
        public byte[] keyHash(T sample) {
            int[] key = keyOf(sample);
            if (key == null) {
                return new byte[0];
            }
            byte[] hash = new byte[16];
            for (int word = 0; word < 4; word++) {
                hash[word * 4] = (byte) (key[word] >>> 24);
                hash[word * 4 + 1] = (byte) (key[word] >>> 16);
                hash[word * 4 + 2] = (byte) (key[word] >>> 8);
                hash[word * 4 + 3] = (byte) key[word];
            }
            return hash;
        }
    }

    // --- Reading the identifying members out of a built-in sample -----------

    private static int[] keyOf(Object sample) {
        BuiltinTopicKey key = keyOfSample(sample);
        return key == null ? null : key.getValue();
    }

    private static BuiltinTopicKey keyOfSample(Object sample) {
        if (sample instanceof ParticipantBuiltinTopicDataImpl) {
            ParticipantBuiltinTopicDataImpl data = (ParticipantBuiltinTopicDataImpl) sample;
            return data.getKey();
        }
        if (sample instanceof PublicationBuiltinTopicDataImpl) {
            PublicationBuiltinTopicDataImpl data = (PublicationBuiltinTopicDataImpl) sample;
            return data.getKey();
        }
        if (sample instanceof SubscriptionBuiltinTopicDataImpl) {
            SubscriptionBuiltinTopicDataImpl data = (SubscriptionBuiltinTopicDataImpl) sample;
            return data.getKey();
        }
        if (sample instanceof TopicBuiltinTopicDataImpl) {
            TopicBuiltinTopicDataImpl data = (TopicBuiltinTopicDataImpl) sample;
            return data.getKey();
        }
        return null;
    }

    private static String typeNameOf(Object sample) {
        if (sample instanceof PublicationBuiltinTopicDataImpl) {
            PublicationBuiltinTopicDataImpl data = (PublicationBuiltinTopicDataImpl) sample;
            return data.getTypeName();
        }
        if (sample instanceof SubscriptionBuiltinTopicDataImpl) {
            SubscriptionBuiltinTopicDataImpl data = (SubscriptionBuiltinTopicDataImpl) sample;
            return data.getTypeName();
        }
        if (sample instanceof TopicBuiltinTopicDataImpl) {
            TopicBuiltinTopicDataImpl data = (TopicBuiltinTopicDataImpl) sample;
            return data.getTypeName();
        }
        return null;
    }

    /** The key of {@code handle} as a built-in topic key (DDS 1.2 §2.2.1.3.1). */
    public static int[] wordsOf(ServiceEnvironment environment,
            org.omg.dds.core.InstanceHandle handle) {
        return BuiltinTopicKeyImpl.of(environment, handle).getValue();
    }
}
