// SPDX-License-Identifier: Apache-2.0
package org.omg.dds;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.omg.dds.core.policy.PolicyFactory;
import org.omg.dds.core.policy.TypeConsistencyEnforcement;
import org.omg.dds.domain.DomainParticipant;
import org.omg.dds.pub.DataWriter;
import org.omg.dds.pub.Publisher;
import org.omg.dds.sub.DataReader;
import org.omg.dds.sub.Subscriber;
import org.omg.dds.topic.Topic;
import org.omg.dds.topic.TopicQos;
import org.zerodds.domain.DomainParticipantImpl;
import org.zerodds.internal.InProcessBus;
import org.zerodds.rtps.DiscoveryData;
import org.zerodds.rtps.Locator;
import org.zerodds.rtps.ParameterList;
import org.zerodds.rtps.QosParameters;
import org.zerodds.topic.BuiltinData;
import org.zerodds.topic.BuiltinQos;
import org.zerodds.topic.BuiltinTopicKeyImpl;
import org.zerodds.topic.BuiltinTopicWire;
import org.zerodds.topic.BuiltinTopics;
import org.zerodds.topic.PublicationBuiltinTopicDataImpl;
import org.zerodds.topic.SubscriptionBuiltinTopicDataImpl;
import org.zerodds.topic.TopicBuiltinTopicDataImpl;

import java.util.Arrays;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;

/**
 * DDS-Java-PSM 1.0 §7.8: the extensions of DDS-XTypes in the built-in topic data types.
 * {@code getRepresentation} (DATA_REPRESENTATION) and {@code getTypeConsistency}
 * (TYPE_CONSISTENCY_ENFORCEMENT) of {@code TopicBuiltinTopicData},
 * {@code PublicationBuiltinTopicData} and {@code SubscriptionBuiltinTopicData}: from the QoS of
 * a local entity, on the wire of the built-in topics (the parameter ids 0x0073 and 0x0074 of
 * XTypes 1.3 §7.6.3.2.2) and from the SEDP announcement of a remote one.
 *
 * <p>Domain ids 1640 to 1649.
 */
class TypesBuiltinTopicDataTest {

    private static final PolicyFactory PF = Qos.pf();
    private static int dom = 1640;

    private static int nextDom() {
        return dom++;
    }

    @BeforeEach
    void setUp() {
        InProcessBus.instance().reset();
    }

    @AfterEach
    void tearDown() {
        InProcessBus.instance().reset();
    }

    private static final short XCDR2 = 2;

    private static org.omg.dds.topic.BuiltinTopicKey keyOf(DomainParticipant participant) {
        return BuiltinTopicKeyImpl.of(Qos.env(), participant.getInstanceHandle());
    }

    @Test
    void javapsm_7_8_the_publication_data_carries_the_representation_and_the_type_consistency_of_its_writer() throws Exception {
        DomainParticipant participant = Qos.factory().createParticipant(nextDom());
        Topic<byte[]> topic = participant.createTopic("TypesRepPublication", byte[].class);
        Publisher publisher = participant.createPublisher();
        DataWriter<byte[]> writer = publisher.createDataWriter(topic, Qos.writer()
                .withPolicy(PF.Representation().withValue(XCDR2))
                .withPolicy(PF.TypeConsistency().withAssignableTypeConsistency()));

        PublicationBuiltinTopicDataImpl data =
                BuiltinData.publication(Qos.env(), writer, publisher, keyOf(participant));
        assertEquals(Arrays.asList(XCDR2), data.getRepresentation().getValue());
        assertEquals(TypeConsistencyEnforcement.Kind.ASSIGNABLE_TYPE_CONSISTENCY,
                data.getTypeConsistency().getKind());

        // On the parameter ids of XTypes 1.3 §7.6.3.2.2: 0x0073 a sequence of the ids,
        // 0x0074 the kind as 16 bits, ALLOW_TYPE_COERCION (1) for the assignable kind.
        byte[] wire = BuiltinTopicWire.encode(data);
        ParameterList pl = ParameterList.fromBytesLe(wire, 4, wire.length);
        assertArrayEquals(new byte[] {1, 0, 0, 0, 2, 0}, Arrays.copyOf(pl.find(0x0073), 6));
        assertArrayEquals(new byte[] {1, 0}, Arrays.copyOf(pl.find(0x0074), 2));

        PublicationBuiltinTopicDataImpl back = (PublicationBuiltinTopicDataImpl)
                BuiltinTopicWire.decode(Qos.env(), BuiltinTopics.PUBLICATION, wire, 0, wire.length);
        assertEquals(Arrays.asList(XCDR2), back.getRepresentation().getValue());
        assertEquals(TypeConsistencyEnforcement.Kind.ASSIGNABLE_TYPE_CONSISTENCY,
                back.getTypeConsistency().getKind());
    }

    @Test
    void javapsm_7_8_an_exact_type_consistency_forbids_coercion_on_the_wire() throws Exception {
        DomainParticipant participant = Qos.factory().createParticipant(nextDom());
        Topic<byte[]> topic = participant.createTopic("TypesExactPublication", byte[].class);
        Publisher publisher = participant.createPublisher();
        DataWriter<byte[]> writer = publisher.createDataWriter(topic, Qos.writer()
                .withPolicy(PF.TypeConsistency().withExactTypeTypeConsistency()));

        PublicationBuiltinTopicDataImpl data =
                BuiltinData.publication(Qos.env(), writer, publisher, keyOf(participant));
        QosParameters q = BuiltinQos.of(data);
        // XTypes 1.3 §7.6.3.4: DISALLOW_TYPE_COERCION is 0.
        assertEquals(0, q.typeConsistency.kind);

        byte[] wire = BuiltinTopicWire.encode(data);
        ParameterList pl = ParameterList.fromBytesLe(wire, 4, wire.length);
        assertArrayEquals(new byte[] {0, 0}, Arrays.copyOf(pl.find(0x0074), 2));
        PublicationBuiltinTopicDataImpl back = (PublicationBuiltinTopicDataImpl)
                BuiltinTopicWire.decode(Qos.env(), BuiltinTopics.PUBLICATION, wire, 0, wire.length);
        assertEquals(TypeConsistencyEnforcement.Kind.EXACT_TYPE_TYPE_CONSISTENCY,
                back.getTypeConsistency().getKind());
    }

    @Test
    void javapsm_7_8_the_subscription_data_carries_the_representation_and_the_type_consistency_of_its_reader() throws Exception {
        DomainParticipant participant = Qos.factory().createParticipant(nextDom());
        Topic<byte[]> topic = participant.createTopic("TypesRepSubscription", byte[].class);
        Subscriber subscriber = participant.createSubscriber();
        DataReader<byte[]> reader = subscriber.createDataReader(topic, Qos.reader()
                .withPolicy(PF.Representation().withValue(XCDR2, (short) 0))
                .withPolicy(PF.TypeConsistency().withAssignableTypeConsistency()));

        SubscriptionBuiltinTopicDataImpl data =
                BuiltinData.subscription(Qos.env(), reader, subscriber, keyOf(participant));
        assertEquals(Arrays.asList(XCDR2, (short) 0), data.getRepresentation().getValue());
        assertEquals(TypeConsistencyEnforcement.Kind.ASSIGNABLE_TYPE_CONSISTENCY,
                data.getTypeConsistency().getKind());

        byte[] wire = BuiltinTopicWire.encode(data);
        ParameterList pl = ParameterList.fromBytesLe(wire, 4, wire.length);
        assertArrayEquals(new byte[] {2, 0, 0, 0, 2, 0, 0, 0}, Arrays.copyOf(pl.find(0x0073), 8));
        SubscriptionBuiltinTopicDataImpl back = (SubscriptionBuiltinTopicDataImpl)
                BuiltinTopicWire.decode(Qos.env(), BuiltinTopics.SUBSCRIPTION, wire, 0, wire.length);
        assertEquals(Arrays.asList(XCDR2, (short) 0), back.getRepresentation().getValue());
        assertEquals(TypeConsistencyEnforcement.Kind.ASSIGNABLE_TYPE_CONSISTENCY,
                back.getTypeConsistency().getKind());
    }

    @Test
    void javapsm_7_8_the_topic_data_carries_the_representation_and_the_type_consistency_of_its_topic() {
        DomainParticipant participant = Qos.factory().createParticipant(nextDom());
        TopicQos qos = Qos.topic()
                .withPolicy(PF.Representation().withValue(XCDR2))
                .withPolicy(PF.TypeConsistency().withAssignableTypeConsistency());
        Topic<byte[]> topic = ((DomainParticipantImpl) participant)
                .createTopic("TypesRepTopic", byte[].class, qos);

        TopicBuiltinTopicDataImpl data = BuiltinData.topic(Qos.env(), topic);
        assertEquals(Arrays.asList(XCDR2), data.getRepresentation().getValue());
        assertEquals(TypeConsistencyEnforcement.Kind.ASSIGNABLE_TYPE_CONSISTENCY,
                data.getTypeConsistency().getKind());

        byte[] wire = BuiltinTopicWire.encode(data);
        TopicBuiltinTopicDataImpl back = (TopicBuiltinTopicDataImpl)
                BuiltinTopicWire.decode(Qos.env(), BuiltinTopics.TOPIC, wire, 0, wire.length);
        assertEquals(Arrays.asList(XCDR2), back.getRepresentation().getValue());
        assertEquals(TypeConsistencyEnforcement.Kind.ASSIGNABLE_TYPE_CONSISTENCY,
                back.getTypeConsistency().getKind());
    }

    /**
     * The value of parameter {@code pid} in a PL_CDR_LE payload (the encapsulation header, then
     * the parameter list of DDSI-RTPS 2.5 §9.4.2.11), read here and not by the decoder under test.
     */
    private static byte[] parameter(byte[] payload, int pid) {
        java.nio.ByteBuffer b = java.nio.ByteBuffer.wrap(payload).order(java.nio.ByteOrder.LITTLE_ENDIAN);
        b.position(4);
        while (b.remaining() >= 4) {
            int id = b.getShort() & 0xFFFF;
            int length = b.getShort() & 0xFFFF;
            if (id == 0x0001) {
                break;
            }
            byte[] value = new byte[length];
            b.get(value);
            if (id == pid) {
                return value;
            }
        }
        return null;
    }

    @Test
    void xtypes_7_6_3_2_a_remote_endpoint_announces_its_representation_and_type_consistency_in_sedp() {
        QosParameters qos = new QosParameters();
        qos.dataRepresentation = new int[] {2};
        qos.typeConsistency = new QosParameters.TypeConsistency();
        qos.typeConsistency.kind = 0;
        byte[] payload = DiscoveryData.buildEndpoint(true, new byte[12], new byte[16], "T", "Ty",
                2, 1, new int[] {2}, Locator.udpV4("127.0.0.1", 7411), new byte[0], new byte[0], qos);
        // DATA_REPRESENTATION (PID 0x0073): a sequence of one short, XCDR2 = 2, padded to four
        assertArrayEquals(new byte[] {1, 0, 0, 0, 2, 0, 0, 0}, parameter(payload, 0x0073));
        // TYPE_CONSISTENCY_ENFORCEMENT (PID 0x0074): the kind as 16 bits (@bit_bound(16)),
        // DISALLOW_TYPE_COERCION = 0, then the five booleans, ignore_sequence_bounds and
        // ignore_string_bounds true (XTypes 1.3 §7.6.3.4.1), padded to eight
        assertArrayEquals(new byte[] {0, 0, 1, 1, 0, 0, 0, 0}, parameter(payload, 0x0074));
        DiscoveryData.Endpoint parsed = DiscoveryData.parseEndpoint(true, payload);
        assertNotNull(parsed);
        assertArrayEquals(new int[] {2}, parsed.qos.dataRepresentation);
        assertEquals(0, parsed.qos.typeConsistency.kind);

        PublicationBuiltinTopicDataImpl publication = BuiltinData.publication(Qos.env(), parsed);
        assertEquals(Arrays.asList(XCDR2), publication.getRepresentation().getValue());
        assertEquals(TypeConsistencyEnforcement.Kind.EXACT_TYPE_TYPE_CONSISTENCY,
                publication.getTypeConsistency().getKind());

        // The same announcement as the data of a reader: an assignable type, coercion allowed.
        qos.typeConsistency.kind = 1;
        byte[] readerPayload = DiscoveryData.buildEndpoint(false, new byte[12], new byte[16], "T",
                "Ty", 2, 1, new int[] {2}, Locator.udpV4("127.0.0.1", 7411), new byte[0],
                new byte[0], qos);
        // ALLOW_TYPE_COERCION = 1
        assertArrayEquals(new byte[] {1, 0, 1, 1, 0, 0, 0, 0}, parameter(readerPayload, 0x0074));
        SubscriptionBuiltinTopicDataImpl subscription = BuiltinData.subscription(Qos.env(),
                DiscoveryData.parseEndpoint(false, readerPayload));
        assertEquals(Arrays.asList(XCDR2), subscription.getRepresentation().getValue());
        assertEquals(TypeConsistencyEnforcement.Kind.ASSIGNABLE_TYPE_CONSISTENCY,
                subscription.getTypeConsistency().getKind());
    }

    @Test
    void javapsm_7_8_an_endpoint_that_states_neither_has_the_defaults_of_the_entity_qos() {
        byte[] payload = DiscoveryData.buildEndpoint(true, new byte[12], new byte[16], "T", "Ty",
                2, 1, new int[] {0}, Locator.udpV4("127.0.0.1", 7411), new byte[0]);
        DiscoveryData.Endpoint parsed = DiscoveryData.parseEndpoint(true, payload);
        assertNotNull(parsed);
        PublicationBuiltinTopicDataImpl publication = BuiltinData.publication(Qos.env(), parsed);
        assertEquals(Qos.writer().getRepresentation(), publication.getRepresentation());
        assertEquals(Qos.writer().getTypeConsistency(), publication.getTypeConsistency());
    }
}
