// SPDX-License-Identifier: Apache-2.0
package org.omg.dds;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.omg.dds.domain.DomainParticipant;
import org.omg.dds.pub.DataWriter;
import org.omg.dds.pub.Publisher;
import org.omg.dds.sub.DataReader;
import org.omg.dds.sub.Sample;
import org.omg.dds.sub.Subscriber;
import org.omg.dds.topic.Topic;
import org.zerodds.internal.InProcessBus;
import org.zerodds.topic.BuiltinTopics;
import org.zerodds.topic.PublicationBuiltinTopicDataImpl;
import org.zerodds.topic.SubscriptionBuiltinTopicDataImpl;

import java.util.Arrays;
import java.util.Collections;
import java.util.List;
import java.util.concurrent.TimeUnit;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;

/**
 * The built-in topic data of a writer and of a reader carries its QoS (DDS 1.4 §2.2.5), so a
 * {@code set_qos} on the entity or a changed PARTITION or GROUP_DATA of its Publisher or
 * Subscriber is a new sample of the same instance. Domain ids 1400 to 1499.
 */
class QosBuiltinUpdateTest {

    @BeforeEach
    void setUp() {
        InProcessBus.instance().reset();
    }

    @AfterEach
    void tearDown() {
        InProcessBus.instance().reset();
    }

    @SuppressWarnings("unchecked")
    private static <T> Topic<T> builtin(DomainParticipant participant, String name) {
        return (Topic<T>) participant.lookupTopicDescription(name);
    }

    /** The last sample of the topic {@code Items} in {@code samples}, or null. */
    private static <T> Sample<T> last(List<Sample<T>> samples, java.util.function.Predicate<T> matches) {
        Sample<T> found = null;
        for (Sample<T> sample : samples) {
            if (sample.getData() != null && matches.test(sample.getData())) {
                found = sample;
            }
        }
        return found;
    }

    @Test
    void aChangedQosOfAWriterAndOfItsPublisherIsANewSampleOnThePublicationTopic() throws Exception {
        int domain = QosFixtures.nextDomain();
        DomainParticipant observer = Qos.factory().createParticipant(domain);
        DomainParticipant other = Qos.factory().createParticipant(domain);
        try {
            DataReader<PublicationBuiltinTopicDataImpl> publications = observer.getBuiltinSubscriber()
                    .createDataReader(QosBuiltinUpdateTest.<PublicationBuiltinTopicDataImpl>builtin(
                            observer, BuiltinTopics.PUBLICATION));
            Topic<QosFixtures.Item> topic = other.createTopic("Items", QosFixtures.Item.class);
            Publisher publisher = other.createPublisher();
            DataWriter<QosFixtures.Item> writer = Qos.writer(publisher, topic, QosFixtures.SUPPORT, Qos.writer());

            Sample<PublicationBuiltinTopicDataImpl> first = last(Qos.take(publications),
                    data -> "Items".equals(data.getTopicName()));
            assertNotNull(first, "the writer is announced");
            assertEquals(0, first.getData().getOwnershipStrength().getValue());

            writer.setQos(writer.getQos().withPolicy(Qos.pf().OwnershipStrength().withValue(9)));
            Sample<PublicationBuiltinTopicDataImpl> second = last(Qos.take(publications),
                    data -> "Items".equals(data.getTopicName()));
            assertNotNull(second, "the changed QoS is announced");
            assertEquals(9, second.getData().getOwnershipStrength().getValue());

            publisher.setQos(publisher.getQos().withPolicy(Qos.pf().Partition().withName(Arrays.asList("P"))));
            Sample<PublicationBuiltinTopicDataImpl> third = last(Qos.take(publications),
                    data -> "Items".equals(data.getTopicName()));
            assertNotNull(third, "the changed PARTITION of the publisher is announced");
            assertEquals(Collections.singleton("P"), third.getData().getPartition().getName());
            assertEquals(9, third.getData().getOwnershipStrength().getValue());
        } finally {
            other.close();
            observer.close();
        }
    }

    @Test
    void aChangedQosOfAReaderAndOfItsSubscriberIsANewSampleOnTheSubscriptionTopic() throws Exception {
        int domain = QosFixtures.nextDomain();
        DomainParticipant observer = Qos.factory().createParticipant(domain);
        DomainParticipant other = Qos.factory().createParticipant(domain);
        try {
            DataReader<SubscriptionBuiltinTopicDataImpl> subscriptions = observer.getBuiltinSubscriber()
                    .createDataReader(QosBuiltinUpdateTest.<SubscriptionBuiltinTopicDataImpl>builtin(
                            observer, BuiltinTopics.SUBSCRIPTION));
            Topic<QosFixtures.Item> topic = other.createTopic("Items", QosFixtures.Item.class);
            Subscriber subscriber = other.createSubscriber();
            DataReader<QosFixtures.Item> reader = Qos.reader(subscriber, topic, QosFixtures.SUPPORT, Qos.reader());

            Sample<SubscriptionBuiltinTopicDataImpl> first = last(Qos.take(subscriptions),
                    data -> "Items".equals(data.getTopicName()));
            assertNotNull(first, "the reader is announced");
            assertEquals(0, first.getData().getTimeBasedFilter().getMinimumSeparation()
                    .getDuration(TimeUnit.MILLISECONDS));

            reader.setQos(reader.getQos().withPolicy(
                    Qos.pf().TimeBasedFilter().withMinimumSeparation(50, TimeUnit.MILLISECONDS)));
            Sample<SubscriptionBuiltinTopicDataImpl> second = last(Qos.take(subscriptions),
                    data -> "Items".equals(data.getTopicName()));
            assertNotNull(second, "the changed QoS is announced");
            assertEquals(50, second.getData().getTimeBasedFilter().getMinimumSeparation()
                    .getDuration(TimeUnit.MILLISECONDS));

            subscriber.setQos(subscriber.getQos().withPolicy(
                    Qos.pf().GroupData().withValue(new byte[] {4, 2}, 0, 2)));
            Sample<SubscriptionBuiltinTopicDataImpl> third = last(Qos.take(subscriptions),
                    data -> "Items".equals(data.getTopicName()));
            assertNotNull(third, "the changed GROUP_DATA of the subscriber is announced");
            assertEquals(Arrays.asList((byte) 4, (byte) 2),
                    Arrays.asList(third.getData().getGroupData().getValue()[0],
                            third.getData().getGroupData().getValue()[1]));
        } finally {
            other.close();
            observer.close();
        }
    }
}
