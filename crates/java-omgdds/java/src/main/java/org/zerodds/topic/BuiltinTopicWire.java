// SPDX-License-Identifier: Apache-2.0
package org.zerodds.topic;

import org.omg.dds.core.ServiceEnvironment;
import org.omg.dds.topic.BuiltinTopicKey;
import org.zerodds.cdr.XcdrException;
import org.zerodds.rtps.ParameterList;
import org.zerodds.rtps.QosParameters;
import org.zerodds.rtps.Rtps;
import org.zerodds.rtps.Wire;

/**
 * The wire form of the built-in topic data of DDS 1.4 §2.2.5: the
 * {@code PL_CDR_LE} ParameterList of DDSI-RTPS 2.5 §9.6.2 (§9.4.2.11) that
 * SPDP and SEDP announce, with every member of the structure on its
 * ParameterId (Table 9.13).
 *
 * <ul>
 *   <li>ParticipantBuiltinTopicData: {@code PID_PARTICIPANT_GUID} (the key)
 *       and {@code PID_USER_DATA};</li>
 *   <li>TopicBuiltinTopicData: {@code PID_ENDPOINT_GUID} (the key),
 *       {@code PID_TOPIC_NAME}, {@code PID_TYPE_NAME} and the QoS of the
 *       topic;</li>
 *   <li>Publication/SubscriptionBuiltinTopicData: {@code PID_ENDPOINT_GUID}
 *       (the key), {@code PID_PARTICIPANT_GUID} (the participant key),
 *       {@code PID_TOPIC_NAME}, {@code PID_TYPE_NAME} and the QoS of the
 *       writer or reader.</li>
 * </ul>
 *
 * <p>The 16 octets of a GUID are the four words of the DDS key, each word
 * big-endian. A policy that is not in the announcement is the default of the
 * entity kind ({@link BuiltinQos}).
 */
public final class BuiltinTopicWire {

    private BuiltinTopicWire() {}

    private static final int PID_TOPIC_NAME = 0x0005;
    private static final int PID_TYPE_NAME = 0x0007;
    private static final int PID_PARTICIPANT_GUID = 0x0050;
    private static final int PID_ENDPOINT_GUID = 0x005A;

    /** Encodes a built-in topic sample: the encapsulation header and the ParameterList. */
    public static byte[] encode(Object sample) {
        ParameterList pl = new ParameterList();
        if (sample instanceof ParticipantBuiltinTopicDataImpl) {
            ParticipantBuiltinTopicDataImpl data = (ParticipantBuiltinTopicDataImpl) sample;
            pl.add(PID_PARTICIPANT_GUID, guid(requireKey(data.getKey())));
            BuiltinQos.of(data).writeTo(pl, QosParameters.Entity.PARTICIPANT);
        } else if (sample instanceof PublicationBuiltinTopicDataImpl) {
            PublicationBuiltinTopicDataImpl data = (PublicationBuiltinTopicDataImpl) sample;
            pl.add(PID_PARTICIPANT_GUID, guid(data.getParticipantKey()));
            pl.add(PID_ENDPOINT_GUID, guid(requireKey(data.getKey())));
            pl.add(PID_TOPIC_NAME, ParameterList.cdrString(orEmpty(data.getTopicName())));
            pl.add(PID_TYPE_NAME, ParameterList.cdrString(orEmpty(data.getTypeName())));
            BuiltinQos.of(data).writeTo(pl, QosParameters.Entity.PUBLICATION);
        } else if (sample instanceof SubscriptionBuiltinTopicDataImpl) {
            SubscriptionBuiltinTopicDataImpl data = (SubscriptionBuiltinTopicDataImpl) sample;
            pl.add(PID_PARTICIPANT_GUID, guid(data.getParticipantKey()));
            pl.add(PID_ENDPOINT_GUID, guid(requireKey(data.getKey())));
            pl.add(PID_TOPIC_NAME, ParameterList.cdrString(orEmpty(data.getTopicName())));
            pl.add(PID_TYPE_NAME, ParameterList.cdrString(orEmpty(data.getTypeName())));
            BuiltinQos.of(data).writeTo(pl, QosParameters.Entity.SUBSCRIPTION);
        } else if (sample instanceof TopicBuiltinTopicDataImpl) {
            TopicBuiltinTopicDataImpl data = (TopicBuiltinTopicDataImpl) sample;
            pl.add(PID_ENDPOINT_GUID, guid(requireKey(data.getKey())));
            pl.add(PID_TOPIC_NAME, ParameterList.cdrString(orEmpty(data.getName())));
            pl.add(PID_TYPE_NAME, ParameterList.cdrString(orEmpty(data.getTypeName())));
            BuiltinQos.of(data).writeTo(pl, QosParameters.Entity.TOPIC);
        } else {
            throw new XcdrException("not a built-in topic sample: " + sample);
        }
        Wire w = new Wire();
        w.bytes(Rtps.ENCAP_PL_CDR_LE);
        w.bytes(pl.toBytesLe());
        return w.toBytes();
    }

    /**
     * Decodes the built-in topic data of {@code topicName}.
     *
     * @throws XcdrException if the bytes are not a ParameterList of that
     *         topic, lack its key, or hold a policy that does not decode.
     */
    public static Object decode(ServiceEnvironment env, String topicName,
            byte[] bytes, int offset, int length) {
        if (length < 4 || bytes[offset] != 0x00 || bytes[offset + 1] != 0x03) {
            throw new XcdrException("a built-in topic sample is a PL_CDR_LE ParameterList");
        }
        ParameterList pl = ParameterList.fromBytesLe(bytes, offset + 4, offset + length);
        try {
            switch (topicName) {
                case BuiltinTopics.PARTICIPANT: {
                    ParticipantBuiltinTopicDataImpl data = new ParticipantBuiltinTopicDataImpl(
                            env, key(env, pl, PID_PARTICIPANT_GUID, true), null);
                    BuiltinQos.applyTo(data, QosParameters.readFrom(pl,
                            QosParameters.Entity.PARTICIPANT), env);
                    return data;
                }
                case BuiltinTopics.PUBLICATION: {
                    PublicationBuiltinTopicDataImpl data = new PublicationBuiltinTopicDataImpl(env);
                    data.setKey(key(env, pl, PID_ENDPOINT_GUID, true));
                    data.setParticipantKey(key(env, pl, PID_PARTICIPANT_GUID, false));
                    data.setTopicName(string(pl, PID_TOPIC_NAME));
                    data.setTypeName(string(pl, PID_TYPE_NAME));
                    BuiltinQos.applyTo(data, QosParameters.readFrom(pl,
                            QosParameters.Entity.PUBLICATION), env);
                    return data;
                }
                case BuiltinTopics.SUBSCRIPTION: {
                    SubscriptionBuiltinTopicDataImpl data = new SubscriptionBuiltinTopicDataImpl(env);
                    data.setKey(key(env, pl, PID_ENDPOINT_GUID, true));
                    data.setParticipantKey(key(env, pl, PID_PARTICIPANT_GUID, false));
                    data.setTopicName(string(pl, PID_TOPIC_NAME));
                    data.setTypeName(string(pl, PID_TYPE_NAME));
                    BuiltinQos.applyTo(data, QosParameters.readFrom(pl,
                            QosParameters.Entity.SUBSCRIPTION), env);
                    return data;
                }
                default: {
                    TopicBuiltinTopicDataImpl data = new TopicBuiltinTopicDataImpl(env);
                    data.setKey(key(env, pl, PID_ENDPOINT_GUID, true));
                    data.setName(string(pl, PID_TOPIC_NAME));
                    data.setTypeName(string(pl, PID_TYPE_NAME));
                    BuiltinQos.applyTo(data, QosParameters.readFrom(pl,
                            QosParameters.Entity.TOPIC), env);
                    return data;
                }
            }
        } catch (IllegalArgumentException e) {
            throw new XcdrException("built-in topic " + topicName + ": " + e.getMessage());
        }
    }

    /** The key of an endpoint as its 16-octet GUID (four big-endian words). */
    public static byte[] guid(BuiltinTopicKey key) {
        byte[] out = new byte[16];
        if (key == null) {
            return out;
        }
        int[] words = key.getValue();
        for (int word = 0; word < 4; word++) {
            int v = word < words.length ? words[word] : 0;
            out[word * 4] = (byte) (v >>> 24);
            out[word * 4 + 1] = (byte) (v >>> 16);
            out[word * 4 + 2] = (byte) (v >>> 8);
            out[word * 4 + 3] = (byte) v;
        }
        return out;
    }

    private static BuiltinTopicKey requireKey(BuiltinTopicKey key) {
        if (key == null) {
            throw new XcdrException("a built-in topic sample must carry its key");
        }
        return key;
    }

    private static BuiltinTopicKey key(ServiceEnvironment env, ParameterList pl, int pid,
            boolean required) {
        byte[] v = pl.find(pid);
        if (v == null || v.length < 16) {
            if (required) {
                throw new XcdrException("the built-in topic sample has no key (parameter "
                        + Integer.toHexString(pid) + ")");
            }
            return new BuiltinTopicKeyImpl(env, new int[4]);
        }
        int[] words = new int[4];
        for (int word = 0; word < 4; word++) {
            int value = 0;
            for (int b = 0; b < 4; b++) {
                value = (value << 8) | (v[word * 4 + b] & 0xFF);
            }
            words[word] = value;
        }
        return new BuiltinTopicKeyImpl(env, words);
    }

    private static String string(ParameterList pl, int pid) {
        byte[] v = pl.find(pid);
        return v == null ? "" : ParameterList.readCdrString(v);
    }

    private static String orEmpty(String value) {
        return value == null ? "" : value;
    }
}
