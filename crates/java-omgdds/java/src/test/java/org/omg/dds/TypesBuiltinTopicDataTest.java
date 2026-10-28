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
    void thePublicationDataCarriesTheRepresentationAndTheTypeConsistencyOfItsWriter() throws Exception {
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
    void anExactTypeConsistencyForbidsCoercionOnTheWire() throws Exception {
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
    void theSubscriptionDataCarriesTheRepresentationAndTheTypeConsistencyOfItsReader() throws Exception {
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
    void theTopicDataCarriesTheRepresentationAndTheTypeConsistencyOfItsTopic() {
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

    @Test
    void aRemoteEndpointAnnouncesTheRepresentationAndTheTypeConsistencyInSedp() {
        QosParameters qos = new QosParameters();
        qos.dataRepresentation = new int[] {2};
        qos.typeConsistency = new QosParameters.TypeConsistency();
        qos.typeConsistency.kind = 0;
        byte[] payload = DiscoveryData.buildEndpoint(true, new byte[12], new byte[16], "T", "Ty",
                2, 1, new int[] {2}, Locator.udpV4("127.0.0.1", 7411), new byte[0], new byte[0], qos);
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
        SubscriptionBuiltinTopicDataImpl subscription = BuiltinData.subscription(Qos.env(),
                DiscoveryData.parseEndpoint(false, readerPayload));
        assertEquals(Arrays.asList(XCDR2), subscription.getRepresentation().getValue());
        assertEquals(TypeConsistencyEnforcement.Kind.ASSIGNABLE_TYPE_CONSISTENCY,
                subscription.getTypeConsistency().getKind());
    }

    @Test
    void anEndpointThatStatesNeitherHasTheDefaultsOfTheEntityQos() {
        byte[] payload = DiscoveryData.buildEndpoint(true, new byte[12], new byte[16], "T", "Ty",
                2, 1, new int[] {0}, Locator.udpV4("127.0.0.1", 7411), new byte[0]);
        DiscoveryData.Endpoint parsed = DiscoveryData.parseEndpoint(true, payload);
        assertNotNull(parsed);
        PublicationBuiltinTopicDataImpl publication = BuiltinData.publication(Qos.env(), parsed);
        assertEquals(Qos.writer().getRepresentation(), publication.getRepresentation());
        assertEquals(Qos.writer().getTypeConsistency(), publication.getTypeConsistency());
    }
}
