// SPDX-License-Identifier: Apache-2.0
package org.omg.dds;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.omg.dds.core.policy.Deadline;
import org.omg.dds.core.policy.Durability;
import org.omg.dds.core.policy.GroupData;
import org.omg.dds.core.policy.Liveliness;
import org.omg.dds.core.policy.Ownership;
import org.omg.dds.core.policy.OwnershipStrength;
import org.omg.dds.core.policy.Partition;
import org.omg.dds.core.policy.PolicyFactory;
import org.omg.dds.core.policy.Reliability;
import org.omg.dds.core.policy.TimeBasedFilter;
import org.omg.dds.core.policy.UserData;
import org.omg.dds.domain.DomainParticipant;
import org.omg.dds.pub.DataWriter;
import org.omg.dds.pub.DataWriterQos;
import org.omg.dds.pub.Publisher;
import org.omg.dds.pub.PublisherQos;
import org.omg.dds.sub.DataReader;
import org.omg.dds.sub.DataReaderQos;
import org.omg.dds.sub.Sample;
import org.omg.dds.sub.Subscriber;
import org.omg.dds.topic.PublicationBuiltinTopicData;
import org.omg.dds.topic.SubscriptionBuiltinTopicData;
import org.omg.dds.topic.Topic;
import org.omg.dds.topic.TopicBuiltinTopicData;
import org.omg.dds.topic.TopicQos;
import org.zerodds.domain.DomainParticipantImpl;
import org.zerodds.internal.InProcessBus;
import org.zerodds.rtps.DiscoveryData;
import org.zerodds.rtps.Locator;
import org.zerodds.rtps.ParameterList;
import org.zerodds.rtps.QosParameters;
import org.zerodds.topic.BuiltinData;
import org.zerodds.topic.BuiltinQos;
import org.zerodds.topic.BuiltinTopicWire;
import org.zerodds.topic.BuiltinTopics;
import org.zerodds.topic.PublicationBuiltinTopicDataImpl;
import org.zerodds.topic.SubscriptionBuiltinTopicDataImpl;
import org.zerodds.topic.TopicBuiltinTopicDataImpl;

import java.util.Arrays;
import java.util.List;
import java.util.concurrent.TimeUnit;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * DDS 1.4 §2.2.5 and DDSI-RTPS 2.5 §9.6.2 (Table 9.13): the built-in topic
 * data carries the QoS policies of the structure, on the wire between
 * participants and in the discovery announcements of RTPS.
 */
class BuiltinTopicQosTest {

    private static int dom = 700;

    private static int nextDom() {
        return dom++;
    }

    private static final PolicyFactory PF = Qos.pf();

    @BeforeEach
    void setUp() {
        InProcessBus.instance().reset();
    }

    @AfterEach
    void tearDown() {
        InProcessBus.instance().reset();
    }

    @SuppressWarnings("unchecked")
    private static <T> Topic<T> builtin(DomainParticipant participant, String name,
            Class<T> dataType) {
        return (Topic<T>) participant.lookupTopicDescription(name);
    }

    private static byte[] bytes(int... values) {
        byte[] out = new byte[values.length];
        for (int i = 0; i < out.length; i++) {
            out[i] = (byte) values[i];
        }
        return out;
    }

    /** The writer QoS of the tests: every policy of PublicationBuiltinTopicData off its default. */
    private static DataWriterQos writerQos() {
        return Qos.writer()
                .withPolicy(PF.Durability().withTransientLocal())
                .withPolicy(PF.Deadline().withPeriod(5, TimeUnit.SECONDS))
                .withPolicy(PF.LatencyBudget().withDuration(2, TimeUnit.SECONDS))
                .withPolicy(PF.Liveliness().withManualByTopic()
                        .withLeaseDuration(7, TimeUnit.SECONDS))
                .withPolicy(PF.Reliability().withReliable()
                        .withMaxBlockingTime(1, TimeUnit.SECONDS))
                .withPolicy(PF.Lifespan().withDuration(30, TimeUnit.SECONDS))
                .withPolicy(PF.UserData().withValue(bytes(1, 2, 3), 0, 3))
                .withPolicy(PF.Ownership().withExclusive())
                .withPolicy(PF.OwnershipStrength().withValue(7))
                .withPolicy(PF.DestinationOrder().withSourceTimestamp());
    }

    private static PublisherQos publisherQos() {
        return Qos.publisher()
                .withPolicy(PF.Partition().withName(Arrays.asList("A", "B")))
                .withPolicy(PF.GroupData().withValue(bytes(9, 8), 0, 2));
    }

    // --- The wire form of the built-in topics ------------------------------

    @Test
    void javapsm_2_the_publication_wire_form_carries_the_qos_on_the_parameter_ids_of_rtps_table_9_13() {
        DomainParticipant participant = Qos.factory().createParticipant(nextDom());
        Topic<String> topic = participant.createTopic("Chatter", String.class);
        Publisher publisher = participant.createPublisher(publisherQos());
        DataWriter<String> writer = publisher.createDataWriter(topic, writerQos());

        PublicationBuiltinTopicDataImpl data = BuiltinData.publication(Qos.env(), writer,
                publisher, org.zerodds.topic.BuiltinTopicKeyImpl.of(Qos.env(),
                        participant.getInstanceHandle()));
        byte[] wire = BuiltinTopicWire.encode(data);

        // PL_CDR_LE encapsulation, then the ParameterList (DDSI-RTPS 2.5 §10.5).
        assertArrayEquals(bytes(0x00, 0x03, 0x00, 0x00), Arrays.copyOf(wire, 4));
        ParameterList pl = ParameterList.fromBytesLe(wire, 4, wire.length);
        // PID_DURABILITY 0x001d: TRANSIENT_LOCAL = 1.
        assertArrayEquals(bytes(1, 0, 0, 0), pl.find(0x001d));
        // PID_DEADLINE 0x0023: Duration_t { 5, 0 }.
        assertArrayEquals(bytes(5, 0, 0, 0, 0, 0, 0, 0), pl.find(0x0023));
        // PID_LATENCY_BUDGET 0x0027.
        assertArrayEquals(bytes(2, 0, 0, 0, 0, 0, 0, 0), pl.find(0x0027));
        // PID_LIVELINESS 0x001b: MANUAL_BY_TOPIC = 2, lease { 7, 0 }.
        assertArrayEquals(bytes(2, 0, 0, 0, 7, 0, 0, 0, 0, 0, 0, 0), pl.find(0x001b));
        // PID_RELIABILITY 0x001a: RELIABLE = 2, max_blocking_time { 1, 0 }.
        assertArrayEquals(bytes(2, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0), pl.find(0x001a));
        // PID_LIFESPAN 0x002b.
        assertArrayEquals(bytes(30, 0, 0, 0, 0, 0, 0, 0), pl.find(0x002b));
        // PID_USER_DATA 0x002c: sequence<octet>.
        assertArrayEquals(bytes(3, 0, 0, 0, 1, 2, 3, 0), pl.find(0x002c));
        // PID_OWNERSHIP 0x001f: EXCLUSIVE = 1; PID_OWNERSHIP_STRENGTH 0x0006.
        assertArrayEquals(bytes(1, 0, 0, 0), pl.find(0x001f));
        assertArrayEquals(bytes(7, 0, 0, 0), pl.find(0x0006));
        // PID_DESTINATION_ORDER 0x0025: BY_SOURCE_TIMESTAMP = 1.
        assertArrayEquals(bytes(1, 0, 0, 0), pl.find(0x0025));
        // PID_PARTITION 0x0029: sequence<string>, each string padded to 4.
        assertArrayEquals(bytes(2, 0, 0, 0, 2, 0, 0, 0, 'A', 0, 0, 0, 2, 0, 0, 0, 'B', 0, 0, 0),
                pl.find(0x0029));
        // PID_GROUP_DATA 0x002d.
        assertArrayEquals(bytes(2, 0, 0, 0, 9, 8, 0, 0), pl.find(0x002d));
        // PID_PRESENTATION 0x0021: INSTANCE scope, no coherent, no ordered.
        assertArrayEquals(bytes(0, 0, 0, 0, 0, 0, 0, 0), pl.find(0x0021));
        // PID_TOPIC_NAME 0x0005 and PID_TYPE_NAME 0x0007: CDR strings.
        assertEquals("Chatter", ParameterList.readCdrString(pl.find(0x0005)));
        assertEquals(topic.getTypeName(), ParameterList.readCdrString(pl.find(0x0007)));
        // PID_ENDPOINT_GUID 0x005a and PID_PARTICIPANT_GUID 0x0050: 16 octets.
        assertEquals(16, pl.find(0x005a).length);
        assertEquals(16, pl.find(0x0050).length);
    }

    @Test
    void javapsm_2_the_subscription_and_topic_data_carry_their_qos_on_the_parameter_ids_of_rtps_table_9_13() {
        DomainParticipant participant = Qos.factory().createParticipant(nextDom());
        TopicQos topicQos = Qos.topic()
                .withPolicy(PF.Durability().withTransient())
                .withPolicy(PF.History().withKeepLast(5))
                .withPolicy(PF.ResourceLimits().withMaxSamples(100).withMaxInstances(10)
                        .withMaxSamplesPerInstance(20))
                .withPolicy(PF.TransportPriority().withValue(3))
                .withPolicy(PF.Deadline().withPeriod(4, TimeUnit.SECONDS))
                .withPolicy(PF.DurabilityService().withHistoryDepth(8).withMaxSamples(50)
                        .withServiceCleanupDelay(3, TimeUnit.SECONDS))
                .withPolicy(PF.LatencyBudget().withDuration(2, TimeUnit.SECONDS))
                .withPolicy(PF.Liveliness().withManualByParticipant().withLeaseDuration(6, TimeUnit.SECONDS))
                .withPolicy(PF.Lifespan().withDuration(20, TimeUnit.SECONDS))
                .withPolicy(PF.DestinationOrder().withSourceTimestamp())
                .withPolicy(PF.Ownership().withExclusive())
                .withPolicy(PF.Reliability().withReliable().withMaxBlockingTime(3, TimeUnit.SECONDS))
                .withPolicy(org.zerodds.core.policy.PolicyFactoryImpl.class.cast(PF)
                        .TopicData().withValue(bytes(5, 6, 7, 8, 9), 0, 5));
        Topic<String> topic = ((DomainParticipantImpl) participant)
                .createTopic("Chatter", String.class, topicQos);
        Publisher publisher = participant.createPublisher(publisherQos());
        DataWriter<String> writer = publisher.createDataWriter(topic, writerQos());
        Subscriber subscriber = participant.createSubscriber(Qos.subscriber()
                .withPolicy(PF.Partition().withName(Arrays.asList("A")))
                .withPolicy(PF.GroupData().withValue(bytes(7, 7), 0, 2))
                .withPolicy(Qos.policies().Presentation().withTopic().withCoherentAccess(true).withOrderedAccess(true)));
        DataReaderQos readerQos = Qos.reader()
                .withPolicy(PF.Deadline().withPeriod(6, TimeUnit.SECONDS))
                .withPolicy(PF.LatencyBudget().withDuration(1, TimeUnit.SECONDS))
                .withPolicy(PF.Ownership().withExclusive())
                .withPolicy(PF.DestinationOrder().withSourceTimestamp())
                .withPolicy(PF.Durability().withTransientLocal())
                .withPolicy(PF.Reliability().withReliable())
                .withPolicy(PF.Liveliness().withManualByParticipant()
                        .withLeaseDuration(9, TimeUnit.SECONDS))
                .withPolicy(PF.TimeBasedFilter().withMinimumSeparation(1, TimeUnit.SECONDS))
                .withPolicy(PF.UserData().withValue(bytes(4), 0, 1));
        DataReader<String> reader = subscriber.createDataReader(topic, readerQos);
        org.omg.dds.topic.BuiltinTopicKey participantKey =
                org.zerodds.topic.BuiltinTopicKeyImpl.of(Qos.env(), participant.getInstanceHandle());

        // The subscription: PL_CDR_LE, the policies of the reader and of its subscriber and the
        // topic data on the parameter IDs of DDSI-RTPS 2.5 Table 9.13.
        SubscriptionBuiltinTopicDataImpl subscription = BuiltinData.subscription(
                Qos.env(), reader, subscriber, participantKey);
        byte[] subscriptionWire = BuiltinTopicWire.encode(subscription);
        assertArrayEquals(bytes(0x00, 0x03, 0x00, 0x00), Arrays.copyOf(subscriptionWire, 4));
        ParameterList sub = ParameterList.fromBytesLe(subscriptionWire, 4, subscriptionWire.length);
        // PID_DURABILITY 0x001d: TRANSIENT_LOCAL = 1
        assertArrayEquals(bytes(1, 0, 0, 0), sub.find(0x001d));
        // PID_LIVELINESS 0x001b: MANUAL_BY_PARTICIPANT = 1, lease { 9, 0 }
        assertArrayEquals(bytes(1, 0, 0, 0, 9, 0, 0, 0, 0, 0, 0, 0), sub.find(0x001b));
        // PID_TIME_BASED_FILTER 0x0004: minimum_separation { 1, 0 }
        assertArrayEquals(bytes(1, 0, 0, 0, 0, 0, 0, 0), sub.find(0x0004));
        // PID_USER_DATA 0x002c: sequence<octet> { 4 }, padded to four
        assertArrayEquals(bytes(1, 0, 0, 0, 4, 0, 0, 0), sub.find(0x002c));
        // PID_PARTITION 0x0029: sequence<string> { "A" }
        assertArrayEquals(bytes(1, 0, 0, 0, 2, 0, 0, 0, 'A', 0, 0, 0), sub.find(0x0029));
        // PID_TOPIC_DATA 0x002e: the topic data of the topic { 5, 6, 7, 8, 9 }
        assertArrayEquals(bytes(5, 0, 0, 0, 5, 6, 7, 8, 9, 0, 0, 0), sub.find(0x002e));
        // PID_DEADLINE 0x0023 { 6, 0 }, PID_LATENCY_BUDGET 0x0027 { 1, 0 }
        assertArrayEquals(bytes(6, 0, 0, 0, 0, 0, 0, 0), sub.find(0x0023));
        assertArrayEquals(bytes(1, 0, 0, 0, 0, 0, 0, 0), sub.find(0x0027));
        // PID_OWNERSHIP 0x001f: EXCLUSIVE = 1; PID_DESTINATION_ORDER 0x0025: BY_SOURCE_TIMESTAMP = 1
        assertArrayEquals(bytes(1, 0, 0, 0), sub.find(0x001f));
        assertArrayEquals(bytes(1, 0, 0, 0), sub.find(0x0025));
        // PID_GROUP_DATA 0x002d { 7, 7 }; PID_PRESENTATION 0x0021: TOPIC = 1, coherent, ordered
        assertArrayEquals(bytes(2, 0, 0, 0, 7, 7, 0, 0), sub.find(0x002d));
        assertArrayEquals(bytes(1, 0, 0, 0, 1, 1, 0, 0), sub.find(0x0021));
        SubscriptionBuiltinTopicData subscriptionBack = (SubscriptionBuiltinTopicData)
                BuiltinTopicWire.decode(Qos.env(), BuiltinTopics.SUBSCRIPTION,
                        subscriptionWire, 0, subscriptionWire.length);
        assertEquals(subscription.getKey(), subscriptionBack.getKey());
        assertEquals(subscription.getDurability(), subscriptionBack.getDurability());
        assertEquals(subscription.getLiveliness(), subscriptionBack.getLiveliness());
        assertEquals(subscription.getTimeBasedFilter(), subscriptionBack.getTimeBasedFilter());
        assertEquals(subscription.getUserData(), subscriptionBack.getUserData());
        assertEquals(subscription.getPartition(), subscriptionBack.getPartition());
        assertEquals(subscription.getTopicData(), subscriptionBack.getTopicData());

        // The topic: its policies on the same parameter IDs.
        TopicBuiltinTopicDataImpl topicData = BuiltinData.topic(Qos.env(), topic);
        byte[] topicWire = BuiltinTopicWire.encode(topicData);
        assertArrayEquals(bytes(0x00, 0x03, 0x00, 0x00), Arrays.copyOf(topicWire, 4));
        ParameterList top = ParameterList.fromBytesLe(topicWire, 4, topicWire.length);
        assertEquals("Chatter", ParameterList.readCdrString(top.find(0x0005)));
        // PID_DURABILITY: TRANSIENT = 2
        assertArrayEquals(bytes(2, 0, 0, 0), top.find(0x001d));
        // PID_HISTORY 0x0040: KEEP_LAST = 0, depth 5
        assertArrayEquals(bytes(0, 0, 0, 0, 5, 0, 0, 0), top.find(0x0040));
        // PID_RESOURCE_LIMITS 0x0041: max_samples 100, max_instances 10, max_samples_per_instance 20
        assertArrayEquals(bytes(100, 0, 0, 0, 10, 0, 0, 0, 20, 0, 0, 0), top.find(0x0041));
        // PID_TRANSPORT_PRIORITY 0x0049
        assertArrayEquals(bytes(3, 0, 0, 0), top.find(0x0049));
        // PID_DEADLINE 0x0023: { 4, 0 }
        assertArrayEquals(bytes(4, 0, 0, 0, 0, 0, 0, 0), top.find(0x0023));
        // PID_TOPIC_DATA 0x002e
        assertArrayEquals(bytes(5, 0, 0, 0, 5, 6, 7, 8, 9, 0, 0, 0), top.find(0x002e));
        // PID_DURABILITY_SERVICE 0x001e: service_cleanup_delay { 3, 0 }, KEEP_LAST = 0, depth 8,
        // max_samples 50, max_instances and max_samples_per_instance LENGTH_UNLIMITED (-1)
        assertArrayEquals(bytes(3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 8, 0, 0, 0, 50, 0, 0, 0,
                0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF), top.find(0x001e));
        // PID_LATENCY_BUDGET { 2, 0 }; PID_LIVELINESS MANUAL_BY_PARTICIPANT = 1, lease { 6, 0 }
        assertArrayEquals(bytes(2, 0, 0, 0, 0, 0, 0, 0), top.find(0x0027));
        assertArrayEquals(bytes(1, 0, 0, 0, 6, 0, 0, 0, 0, 0, 0, 0), top.find(0x001b));
        // PID_LIFESPAN 0x002b { 20, 0 }; PID_DESTINATION_ORDER BY_SOURCE_TIMESTAMP = 1;
        // PID_OWNERSHIP EXCLUSIVE = 1; PID_RELIABILITY 0x001a RELIABLE = 2, max_blocking_time { 3, 0 }
        assertArrayEquals(bytes(20, 0, 0, 0, 0, 0, 0, 0), top.find(0x002b));
        assertArrayEquals(bytes(1, 0, 0, 0), top.find(0x0025));
        assertArrayEquals(bytes(1, 0, 0, 0), top.find(0x001f));
        assertArrayEquals(bytes(2, 0, 0, 0, 3, 0, 0, 0, 0, 0, 0, 0), top.find(0x001a));
        TopicBuiltinTopicData topicBack = (TopicBuiltinTopicData)
                BuiltinTopicWire.decode(Qos.env(), BuiltinTopics.TOPIC, topicWire, 0,
                        topicWire.length);
        assertEquals("Chatter", topicBack.getName());
        assertEquals(topicData.getDurability(), topicBack.getDurability());
        assertEquals(topicData.getDurabilityService(), topicBack.getDurabilityService());
        assertEquals(topicData.getDeadline(), topicBack.getDeadline());
        assertEquals(5, topicBack.getHistory().getDepth());
        assertEquals(100, topicBack.getResourceLimits().getMaxSamples());
        assertEquals(topicData.getTransportPriority(), topicBack.getTransportPriority());
        assertEquals(topicData.getTopicData(), topicBack.getTopicData());
    }

    // --- A reader of the built-in topic sees the QoS ------------------------

    @Test
    void javapsm_2_a_builtin_reader_sees_the_qos_of_the_announced_writer() throws Exception {
        int domain = nextDom();
        DomainParticipant observer = Qos.factory().createParticipant(domain);
        DataReader<PublicationBuiltinTopicDataImpl> publications =
                observer.getBuiltinSubscriber().createDataReader(
                        builtin(observer, BuiltinTopics.PUBLICATION,
                                PublicationBuiltinTopicDataImpl.class));

        DomainParticipant other = Qos.factory().createParticipant(domain);
        Topic<String> topic = other.createTopic("Chatter", String.class);
        other.createPublisher(publisherQos()).createDataWriter(topic, writerQos());

        PublicationBuiltinTopicDataImpl seen = null;
        for (Sample<PublicationBuiltinTopicDataImpl> sample : Qos.take(publications)) {
            if (sample.getData() != null && "Chatter".equals(sample.getData().getTopicName())) {
                seen = sample.getData();
            }
        }
        assertNotNull(seen, "the publication is announced");
        assertEquals(Durability.Kind.TRANSIENT_LOCAL, seen.getDurability().getKind());
        assertEquals(5, seen.getDeadline().getPeriod().getDuration(TimeUnit.SECONDS));
        assertEquals(Liveliness.Kind.MANUAL_BY_TOPIC, seen.getLiveliness().getKind());
        assertEquals(7, seen.getLiveliness().getLeaseDuration().getDuration(TimeUnit.SECONDS));
        assertEquals(Reliability.Kind.RELIABLE, seen.getReliability().getKind());
        assertEquals(Ownership.Kind.EXCLUSIVE, seen.getOwnership().getKind());
        assertEquals(7, seen.getOwnershipStrength().getValue());
        assertArrayEquals(bytes(1, 2, 3), seen.getUserData().getValue());
        assertEquals(new java.util.LinkedHashSet<>(Arrays.asList("A", "B")),
                seen.getPartition().getName());
        assertArrayEquals(bytes(9, 8), seen.getGroupData().getValue());
    }

    // --- The matched entities --------------------------------------------

    @Test
    void javapsm_2_the_matched_entities_are_described_with_their_qos() throws Exception {
        DomainParticipant participant = Qos.factory().createParticipant(nextDom());
        Topic<String> topic = participant.createTopic("Chatter", String.class);
        Publisher publisher = participant.createPublisher(publisherQos());
        Subscriber subscriber = participant.createSubscriber(Qos.subscriber()
                .withPolicy(PF.Partition().withName(Arrays.asList("A"))));
        // Request-offered (DDS 1.4 §2.2.3): the writer of this test offers what
        // the default reader requests or less.
        DataWriter<String> writer = publisher.createDataWriter(topic, Qos.writer()
                .withPolicy(PF.Durability().withTransientLocal())
                .withPolicy(PF.OwnershipStrength().withValue(7))
                .withPolicy(PF.UserData().withValue(bytes(1, 2, 3), 0, 3)));
        DataReader<String> reader = subscriber.createDataReader(topic,
                Qos.reader().withPolicy(PF.UserData().withValue(bytes(4), 0, 1)));

        assertEquals(1, writer.getMatchedSubscriptions().size());
        assertEquals(1, reader.getMatchedPublications().size());

        SubscriptionBuiltinTopicData sub = writer.getMatchedSubscriptionData(
                writer.getMatchedSubscriptions().iterator().next());
        assertEquals("Chatter", sub.getTopicName());
        assertArrayEquals(bytes(4), sub.getUserData().getValue());

        PublicationBuiltinTopicData pub = reader.getMatchedPublicationData(
                reader.getMatchedPublications().iterator().next());
        assertEquals("Chatter", pub.getTopicName());
        assertEquals(Durability.Kind.TRANSIENT_LOCAL, pub.getDurability().getKind());
        assertEquals(7, pub.getOwnershipStrength().getValue());

        // §2.2.2.4.2.23: a handle that is not a matched subscription is BAD_PARAMETER.
        assertThrows(IllegalArgumentException.class,
                () -> writer.getMatchedSubscriptionData(Qos.env().getSPI().nilHandle()));
        assertThrows(IllegalArgumentException.class,
                () -> reader.getMatchedPublicationData(Qos.env().getSPI().nilHandle()));
    }

    // --- SEDP (DDSI-RTPS 2.5 §9.6.2) ----------------------------------------

    @Test
    void javapsm_2_sedp_of_an_endpoint_without_qos_parameters_takes_the_defaults() {
        byte[] payload = DiscoveryData.buildEndpoint(false, new byte[12], new byte[16], "T", "T",
                2, 1, new int[] {0}, Locator.udpV4("127.0.0.1", 7411), new byte[0]);
        DiscoveryData.Endpoint parsed = DiscoveryData.parseEndpoint(false, payload);
        assertNotNull(parsed);
        assertNull(parsed.qos.deadline, "an absent policy is null, i.e. the default");
        SubscriptionBuiltinTopicDataImpl data = BuiltinData.subscription(Qos.env(), parsed);
        // DDS 1.4 §2.2.3: the deadline default is infinite.
        assertTrue(data.getDeadline().getPeriod().isInfinite());
        assertEquals(Reliability.Kind.RELIABLE, data.getReliability().getKind());
    }

    @Test
    void javapsm_2_an_announcement_with_a_deadline_shorter_than_a_duration_is_refused() {
        ParameterList pl = new ParameterList();
        pl.add(0x005a, new byte[16]);
        pl.add(0x0005, ParameterList.cdrString("T"));
        pl.add(0x0007, ParameterList.cdrString("T"));
        pl.add(0x0023, new byte[4]); // a DEADLINE needs 8 octets
        byte[] body = pl.toBytesLe();
        byte[] payload = new byte[4 + body.length];
        payload[1] = 0x03;
        System.arraycopy(body, 0, payload, 4, body.length);
        assertNull(DiscoveryData.parseEndpoint(true, payload));
    }

    @Test
    void javapsm_7_3_5_1_the_request_offered_rules_decide_the_match_of_remote_endpoints() {
        QosParameters offered = new QosParameters();
        QosParameters requested = new QosParameters();
        assertNull(QosParameters.incompatibility(offered, requested));

        offered.reliability = new QosParameters.Reliability();
        offered.reliability.kind = 1; // BEST_EFFORT
        requested.reliability = new QosParameters.Reliability();
        requested.reliability.kind = 2; // RELIABLE
        assertEquals("RELIABILITY", QosParameters.incompatibility(offered, requested));
        offered.reliability.kind = 2;
        assertNull(QosParameters.incompatibility(offered, requested));

        offered.durability = 0; // VOLATILE
        requested.durability = 1; // TRANSIENT_LOCAL
        assertEquals("DURABILITY", QosParameters.incompatibility(offered, requested));
        offered.durability = 1;

        offered.deadline = new QosParameters.Dur(10, 0);
        requested.deadline = new QosParameters.Dur(5, 0);
        assertEquals("DEADLINE", QosParameters.incompatibility(offered, requested));
        offered.deadline = new QosParameters.Dur(5, 0);

        offered.partition = Arrays.asList("A");
        requested.partition = Arrays.asList("B");
        assertEquals("PARTITION", QosParameters.incompatibility(offered, requested));
        requested.partition = Arrays.asList("A*");
        assertNull(QosParameters.incompatibility(offered, requested));
    }

    // --- The type members of the built-in topic data (XTypes 1.3 §7.6.3.2.2) ---

    public static class BaseType {
        public int a;
    }

    public static class DerivedType extends BaseType {
        public int b;
    }

    private static final List<String> NAMES = Arrays.asList("A::One", "B::Two");

    @Test
    void javapsm_7_8_the_type_members_are_on_their_parameter_ids() {
        PublicationBuiltinTopicDataImpl data = new PublicationBuiltinTopicDataImpl(Qos.env());
        data.setKey(org.zerodds.topic.BuiltinTopicKeyImpl.of(Qos.env(),
                Qos.factory().createParticipant(nextDom()).getInstanceHandle()));
        data.setTopicName("T");
        data.setTypeName("Ty");
        data.setTypeInformation(bytes(1, 2, 3, 4, 5));
        data.setTypeObjectBytes(bytes(9, 8, 7, 6));
        data.setEquivalentTypeName(NAMES);
        data.setBaseTypeName(Arrays.asList("C::Three"));
        byte[] wire = BuiltinTopicWire.encode(data);

        ParameterList pl = ParameterList.fromBytesLe(wire, 4, wire.length);
        // @id(0x0075) type_information and @id(0x0072) type, XTypes 1.3 §7.6.3.2.2
        assertArrayEquals(bytes(1, 2, 3, 4, 5, 0, 0, 0), Arrays.copyOf(pl.find(0x0075), 8));
        assertArrayEquals(bytes(9, 8, 7, 6), pl.find(0x0072));
        // vendor-specific parameters (bit 0x8000), sequence<string>
        assertArrayEquals(bytes(2, 0, 0, 0, 7, 0, 0, 0, 'A', ':', ':', 'O', 'n', 'e', 0, 0,
                7, 0, 0, 0, 'B', ':', ':', 'T', 'w', 'o', 0, 0), pl.find(0x8003));
        assertArrayEquals(bytes(1, 0, 0, 0, 9, 0, 0, 0, 'C', ':', ':', 'T', 'h', 'r', 'e', 'e', 0,
                0, 0, 0), pl.find(0x8004));

        PublicationBuiltinTopicDataImpl back = (PublicationBuiltinTopicDataImpl)
                BuiltinTopicWire.decode(Qos.env(), BuiltinTopics.PUBLICATION, wire, 0, wire.length);
        assertArrayEquals(bytes(1, 2, 3, 4, 5), Arrays.copyOf(back.getTypeInformation(), 5));
        assertArrayEquals(bytes(9, 8, 7, 6), back.getTypeObjectBytes());
        assertEquals(NAMES, back.getEquivalentTypeName());
        assertEquals(Arrays.asList("C::Three"), back.getBaseTypeName());
    }

    @Test
    void javapsm_8_6_1_1_a_local_writer_describes_the_base_types_of_its_type() {
        DomainParticipant participant = Qos.factory().createParticipant(nextDom());
        Topic<DerivedType> topic = participant.createTopic("Derived", DerivedType.class);
        Publisher publisher = participant.createPublisher(publisherQos());
        DataWriter<DerivedType> writer = publisher.createDataWriter(topic, writerQos());
        PublicationBuiltinTopicDataImpl data = BuiltinData.publication(Qos.env(), writer,
                publisher, org.zerodds.topic.BuiltinTopicKeyImpl.of(Qos.env(),
                        participant.getInstanceHandle()));
        assertEquals(Arrays.asList(BaseType.class.getName().replace('$', '.').replace(".", "::")),
                data.getBaseTypeName());
    }

    @Test
    void javapsm_7_8_sedp_carries_the_type_information_on_its_parameter_id() {
        QosParameters qos = new QosParameters();
        qos.baseTypeName = Arrays.asList("C::Three");
        qos.equivalentTypeName = NAMES;
        byte[] payload = DiscoveryData.buildEndpoint(false, new byte[12], new byte[16], "T", "Ty",
                2, 1, new int[] {0}, Locator.udpV4("127.0.0.1", 7411), new byte[0],
                bytes(5, 6, 7, 8), qos);
        // XTypes 1.3 Annex D (DDS::SubscriptionBuiltinTopicData): type_information on @id(0x0075)
        assertArrayEquals(bytes(0x00, 0x03, 0x00, 0x00), Arrays.copyOf(payload, 4));
        assertArrayEquals(bytes(5, 6, 7, 8),
                ParameterList.fromBytesLe(payload, 4, payload.length).find(0x0075));
        DiscoveryData.Endpoint parsed = DiscoveryData.parseEndpoint(false, payload);
        assertNotNull(parsed);
        assertArrayEquals(bytes(5, 6, 7, 8), parsed.typeInformation);
        assertArrayEquals(bytes(5, 6, 7, 8), parsed.qos.typeInformation);
        assertEquals(NAMES, parsed.qos.equivalentTypeName);
        assertEquals(Arrays.asList("C::Three"), parsed.qos.baseTypeName);
        // the remote subscription's built-in data has them
        SubscriptionBuiltinTopicDataImpl data = BuiltinData.subscription(Qos.env(), parsed);
        assertEquals(NAMES, data.getEquivalentTypeName());
        assertArrayEquals(bytes(5, 6, 7, 8), data.getTypeInformation());
    }
}
