// SPDX-License-Identifier: Apache-2.0
package org.omg.dds;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.omg.dds.core.event.PublicationMatchedEvent;
import org.omg.dds.pub.DataWriter;
import org.omg.dds.pub.DataWriterAdapter;
import org.omg.dds.pub.PublisherQos;
import org.omg.dds.sub.DataReader;
import org.omg.dds.sub.Sample;
import org.omg.dds.sub.SubscriberQos;
import org.zerodds.internal.InProcessBus;

import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import java.util.concurrent.CopyOnWriteArrayList;

import static org.junit.jupiter.api.Assertions.assertEquals;

/**
 * PARTITION is a policy that can change (DDS 1.4 §2.2.3.13, "Changeable: Yes"): the writers
 * of a Publisher and the readers of a Subscriber match again when the names change. Domain ids
 * 1400 to 1499.
 */
class QosPartitionChangeTest {

    @BeforeEach
    void setUp() {
        InProcessBus.instance().reset();
    }

    @AfterEach
    void tearDown() {
        InProcessBus.instance().reset();
    }

    private static PublisherQos publisherIn(String... names) {
        return Qos.publisher().withPolicy(Qos.pf().Partition().withName(Arrays.asList(names)));
    }

    private static SubscriberQos subscriberIn(String... names) {
        return Qos.subscriber().withPolicy(Qos.pf().Partition().withName(Arrays.asList(names)));
    }

    private static List<Integer> seqs(List<Sample<QosFixtures.Item>> samples) {
        List<Integer> seqs = new ArrayList<>();
        for (Sample<QosFixtures.Item> sample : samples) {
            if (sample.getData() != null) {
                seqs.add(sample.getData().seq());
            }
        }
        return seqs;
    }

    @Test
    void aPublisherThatChangesItsPartitionEndsAndStartsMatches() throws Exception {
        try (QosFixtures.Rig rig = new QosFixtures.Rig(publisherIn("A"), subscriberIn("A"))) {
            DataReader<QosFixtures.Item> reader = rig.reader(Qos.readerKeepAll());
            DataWriter<QosFixtures.Item> writer = rig.writer(Qos.writer());
            assertEquals(1, reader.getSubscriptionMatchedStatus().getCurrentCount());
            assertEquals(1, writer.getPublicationMatchedStatus().getCurrentCount());
            writer.write(new QosFixtures.Item(1, 1));
            assertEquals(Arrays.asList(1), seqs(Qos.take(reader)));

            rig.publisher.setQos(publisherIn("B"));
            assertEquals(0, reader.getSubscriptionMatchedStatus().getCurrentCount());
            assertEquals(0, writer.getPublicationMatchedStatus().getCurrentCount());
            writer.write(new QosFixtures.Item(1, 2));
            assertEquals(0, Qos.take(reader).size(), "the partitions do not overlap");

            rig.publisher.setQos(publisherIn("A"));
            assertEquals(1, reader.getSubscriptionMatchedStatus().getCurrentCount());
            assertEquals(2, reader.getSubscriptionMatchedStatus().getTotalCount(), "matched twice");
            assertEquals(1, writer.getPublicationMatchedStatus().getCurrentCount());
            writer.write(new QosFixtures.Item(1, 3));
            assertEquals(Arrays.asList(3), seqs(Qos.take(reader)));
        }
    }

    @Test
    void aSubscriberThatChangesItsPartitionEndsAndStartsMatches() throws Exception {
        try (QosFixtures.Rig rig = new QosFixtures.Rig(publisherIn("A"), subscriberIn("B"))) {
            DataReader<QosFixtures.Item> reader = rig.reader(Qos.readerKeepAll());
            DataWriter<QosFixtures.Item> writer = rig.writer(Qos.writer());
            assertEquals(0, reader.getSubscriptionMatchedStatus().getCurrentCount());
            writer.write(new QosFixtures.Item(1, 1));
            assertEquals(0, Qos.take(reader).size());

            rig.subscriber.setQos(subscriberIn("A", "C"));
            assertEquals(1, reader.getSubscriptionMatchedStatus().getCurrentCount());
            assertEquals(1, writer.getPublicationMatchedStatus().getCurrentCount());
            writer.write(new QosFixtures.Item(1, 2));
            assertEquals(Arrays.asList(2), seqs(Qos.take(reader)));

            rig.subscriber.setQos(subscriberIn("C"));
            assertEquals(0, reader.getSubscriptionMatchedStatus().getCurrentCount());
            assertEquals(0, writer.getPublicationMatchedStatus().getCurrentCount());
            writer.write(new QosFixtures.Item(1, 3));
            assertEquals(0, Qos.take(reader).size());
        }
    }

    @Test
    void aChangeThatKeepsTheOverlapIsNoMatchEvent() throws Exception {
        try (QosFixtures.Rig rig = new QosFixtures.Rig(publisherIn("A*"), subscriberIn("AB"))) {
            List<Boolean> matchedEvents = new CopyOnWriteArrayList<>();
            DataWriter<QosFixtures.Item> writer = rig.writer(Qos.writer());
            writer.setListener(new DataWriterAdapter<QosFixtures.Item>() {
                @Override
                public void onPublicationMatched(PublicationMatchedEvent<QosFixtures.Item> status) {
                    matchedEvents.add(status.getStatus().getCurrentCount() > 0);
                }
            });
            rig.reader(Qos.reader());
            assertEquals(Arrays.asList(true), matchedEvents);

            // "A*" and "?B" both match "AB": nothing changes for the pair
            rig.publisher.setQos(publisherIn("?B"));
            assertEquals(Arrays.asList(true), matchedEvents);

            rig.publisher.setQos(publisherIn("C"));
            assertEquals(Arrays.asList(true, false), matchedEvents);
            rig.publisher.setQos(publisherIn("AB"));
            assertEquals(Arrays.asList(true, false, true), matchedEvents);
        }
    }
}
