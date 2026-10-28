// SPDX-License-Identifier: Apache-2.0
package org.omg.dds;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.omg.dds.core.ImmutablePolicyException;
import org.omg.dds.core.InconsistentPolicyException;
import org.omg.dds.core.policy.History;
import org.omg.dds.core.policy.Reliability;
import org.omg.dds.domain.DomainParticipant;
import org.omg.dds.domain.DomainParticipantFactoryQos;
import org.omg.dds.domain.DomainParticipantQos;
import org.omg.dds.pub.DataWriter;
import org.omg.dds.pub.DataWriterQos;
import org.omg.dds.pub.Publisher;
import org.omg.dds.pub.PublisherQos;
import org.omg.dds.sub.DataReader;
import org.omg.dds.sub.DataReaderQos;
import org.omg.dds.sub.Subscriber;
import org.omg.dds.sub.SubscriberQos;
import org.omg.dds.topic.Topic;
import org.omg.dds.topic.TopicQos;
import org.zerodds.core.qos.DomainParticipantQosImpl;
import org.zerodds.domain.DomainParticipantImpl;
import org.zerodds.internal.InProcessBus;

import java.util.Arrays;
import java.util.concurrent.TimeUnit;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotEquals;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertThrows;

/**
 * {@code set_qos} of DDS 1.4 §2.2.2.1.1.1 and the rules of §2.2.3: after the entity is enabled
 * only the policies whose "Changeable" property is "Yes" can change (IMMUTABLE_POLICY), a set
 * of policies that contradict each other is refused (INCONSISTENT_POLICY), and a refused
 * {@code set_qos} leaves the QoS as it was. Domain ids 1400 to 1499.
 */
class QosSetQosTest {

    @BeforeEach
    void setUp() {
        InProcessBus.instance().reset();
    }

    @AfterEach
    void tearDown() {
        InProcessBus.instance().reset();
    }

    // --- DataWriter ---------------------------------------------------------------

    @Test
    void anEnabledWriterTakesNewValuesOfTheChangeablePolicies() throws Exception {
        try (QosFixtures.Rig rig = new QosFixtures.Rig()) {
            DataWriter<QosFixtures.Item> writer = rig.writer(Qos.writer());
            DataWriterQos changed = writer.getQos()
                    .withPolicy(Qos.pf().Deadline().withPeriod(5, TimeUnit.SECONDS))
                    .withPolicy(Qos.pf().LatencyBudget().withDuration(20, TimeUnit.MILLISECONDS))
                    .withPolicy(Qos.pf().OwnershipStrength().withValue(9))
                    .withPolicy(Qos.pf().Lifespan().withDuration(30, TimeUnit.SECONDS))
                    .withPolicy(Qos.pf().TransportPriority().withValue(4))
                    .withPolicy(Qos.pf().UserData().withValue(new byte[] {1, 2, 3}, 0, 3))
                    .withPolicy(Qos.pf().WriterDataLifecycle().withAutDisposeUnregisteredInstances(false));
            writer.setQos(changed);
            assertSame(changed, writer.getQos());
            assertEquals(9, writer.getQos().getOwnershipStrength().getValue());
            assertEquals(5, writer.getQos().getDeadline().getPeriod().getDuration(TimeUnit.SECONDS));
        }
    }

    @Test
    void anEnabledWriterRefusesAChangeOfAnImmutablePolicyAndKeepsItsQos() throws Exception {
        try (QosFixtures.Rig rig = new QosFixtures.Rig()) {
            DataWriter<QosFixtures.Item> writer = rig.writer(Qos.writer());
            DataWriterQos before = writer.getQos();
            DataWriterQos[] refused = {
                before.withPolicy(Qos.policies().DataWriterReliability().withBestEffort()),
                before.withPolicy(Qos.pf().Durability().withTransientLocal()),
                before.withPolicy(Qos.pf().Ownership().withExclusive()),
                before.withPolicy(Qos.pf().History().withKeepAll()),
                before.withPolicy(Qos.pf().DestinationOrder().withSourceTimestamp()),
                before.withPolicy(Qos.pf().Liveliness().withManualByTopic()),
                before.withPolicy(Qos.pf().ResourceLimits().withMaxInstances(7)),
                before.withPolicy(Qos.pf().DurabilityService().withHistoryDepth(4)),
            };
            for (DataWriterQos proposed : refused) {
                assertThrows(ImmutablePolicyException.class, () -> writer.setQos(proposed));
                assertSame(before, writer.getQos(), "a refused set_qos changes nothing");
            }
        }
    }

    @Test
    void settingTheSameValueOfAnImmutablePolicyIsNoChange() throws Exception {
        try (QosFixtures.Rig rig = new QosFixtures.Rig()) {
            DataWriter<QosFixtures.Item> writer = rig.writer(Qos.writer());
            DataWriterQos same = writer.getQos()
                    .withPolicy(Qos.policies().DataWriterReliability())
                    .withPolicy(Qos.pf().History().withKeepLast(1));
            writer.setQos(same);
            assertSame(same, writer.getQos());
        }
    }

    @Test
    void aWriterIsNotCreatedWithPoliciesThatContradictEachOther() throws Exception {
        try (QosFixtures.Rig rig = new QosFixtures.Rig()) {
            // HISTORY depth 5 > max_samples_per_instance 3 (§2.2.3.18)
            DataWriterQos depth = Qos.writer()
                    .withPolicy(Qos.pf().History().withKeepLast(5))
                    .withPolicy(Qos.pf().ResourceLimits().withMaxSamplesPerInstance(3));
            assertThrows(InconsistentPolicyException.class, () -> rig.writer(depth));
            // max_samples 2 < max_samples_per_instance 3 (§2.2.3.19)
            DataWriterQos limits = Qos.writer()
                    .withPolicy(Qos.pf().ResourceLimits().withMaxSamples(2).withMaxSamplesPerInstance(3));
            assertThrows(InconsistentPolicyException.class, () -> rig.writer(limits));
            // DURABILITY_SERVICE: the same two rules for its own history and limits
            DataWriterQos service = Qos.writer().withPolicy(Qos.pf().DurabilityService()
                    .withHistoryDepth(6).withMaxSamplesPerInstance(5));
            assertThrows(InconsistentPolicyException.class, () -> rig.writer(service));
        }
    }

    @Test
    void theBoundariesOfTheConsistencyRulesAreConsistent() throws Exception {
        try (QosFixtures.Rig rig = new QosFixtures.Rig()) {
            // depth == max_samples_per_instance, max_samples == max_samples_per_instance
            DataWriterQos edge = Qos.writer()
                    .withPolicy(Qos.pf().History().withKeepLast(3))
                    .withPolicy(Qos.pf().ResourceLimits().withMaxSamples(3).withMaxSamplesPerInstance(3));
            rig.writer(edge);
            // KEEP_ALL has no depth to compare
            DataWriterQos keepAll = Qos.writer()
                    .withPolicy(Qos.pf().History().withKeepAll().withDepth(50))
                    .withPolicy(Qos.pf().ResourceLimits().withMaxSamplesPerInstance(3));
            rig.writer(keepAll);
            // a limit of LENGTH_UNLIMITED is never exceeded
            DataWriterQos unlimited = Qos.writer()
                    .withPolicy(Qos.pf().History().withKeepLast(100));
            rig.writer(unlimited);
        }
    }

    // --- DataReader ---------------------------------------------------------------

    @Test
    void anEnabledReaderTakesATimeBasedFilterAndRefusesAnotherHistory() throws Exception {
        try (QosFixtures.Rig rig = new QosFixtures.Rig()) {
            DataReader<QosFixtures.Item> reader = rig.reader(Qos.reader());
            DataReaderQos before = reader.getQos();
            DataReaderQos filtered = before.withPolicy(
                    Qos.pf().TimeBasedFilter().withMinimumSeparation(50, TimeUnit.MILLISECONDS));
            reader.setQos(filtered);
            assertSame(filtered, reader.getQos());

            DataReaderQos history = filtered.withPolicy(Qos.pf().History().withKeepAll());
            assertThrows(ImmutablePolicyException.class, () -> reader.setQos(history));
            DataReaderQos ownership = filtered.withPolicy(Qos.pf().Ownership().withExclusive());
            assertThrows(ImmutablePolicyException.class, () -> reader.setQos(ownership));
            DataReaderQos limits = filtered.withPolicy(Qos.pf().ResourceLimits().withMaxSamples(10));
            assertThrows(ImmutablePolicyException.class, () -> reader.setQos(limits));
            assertSame(filtered, reader.getQos());
        }
    }

    @Test
    void aDeadlineShorterThanTheMinimumSeparationIsInconsistent() throws Exception {
        try (QosFixtures.Rig rig = new QosFixtures.Rig()) {
            DataReaderQos inconsistent = Qos.reader()
                    .withPolicy(Qos.pf().Deadline().withPeriod(100, TimeUnit.MILLISECONDS))
                    .withPolicy(Qos.pf().TimeBasedFilter().withMinimumSeparation(1, TimeUnit.SECONDS));
            assertThrows(InconsistentPolicyException.class, () -> rig.reader(inconsistent));

            DataReaderQos fine = Qos.reader()
                    .withPolicy(Qos.pf().Deadline().withPeriod(100, TimeUnit.MILLISECONDS));
            DataReader<QosFixtures.Item> reader = rig.reader(fine);
            // set_qos: the result of adding the new policies on the previous is checked (§2.2.3)
            DataReaderQos tooLong = reader.getQos().withPolicy(
                    Qos.pf().TimeBasedFilter().withMinimumSeparation(1, TimeUnit.SECONDS));
            assertThrows(InconsistentPolicyException.class, () -> reader.setQos(tooLong));
            assertSame(fine, reader.getQos());
            DataReaderQos equalToTheDeadline = reader.getQos().withPolicy(
                    Qos.pf().TimeBasedFilter().withMinimumSeparation(100, TimeUnit.MILLISECONDS));
            reader.setQos(equalToTheDeadline);
            assertSame(equalToTheDeadline, reader.getQos());
        }
    }

    // --- Topic, Publisher, Subscriber, DomainParticipant ---------------------------

    @Test
    void anEnabledTopicTakesTopicDataAndRefusesAnotherReliability() throws Exception {
        try (QosFixtures.Rig rig = new QosFixtures.Rig()) {
            Topic<QosFixtures.Item> topic = rig.topic;
            TopicQos before = topic.getQos();
            TopicQos withData = before
                    .withPolicy(Qos.pf().TransportPriority().withValue(2))
                    .withPolicy(Qos.pf().Lifespan().withDuration(1, TimeUnit.MINUTES))
                    .withPolicy(Qos.pf().Deadline().withPeriod(3, TimeUnit.SECONDS));
            topic.setQos(withData);
            assertSame(withData, topic.getQos());

            TopicQos reliable = withData.withPolicy(Qos.policies().DataWriterReliability());
            // the topic default is BEST_EFFORT; RELIABLE differs
            assertNotEquals(withData.getReliability(), reliable.getReliability());
            assertThrows(ImmutablePolicyException.class, () -> topic.setQos(reliable));
            assertThrows(ImmutablePolicyException.class, () -> topic.setQos(
                    withData.withPolicy(Qos.pf().Durability().withTransient())));
            assertSame(withData, topic.getQos());
        }
    }

    @Test
    void aTopicIsNotCreatedWithPoliciesThatContradictEachOther() throws Exception {
        try (QosFixtures.Rig rig = new QosFixtures.Rig()) {
            TopicQos inconsistent = Qos.topic()
                    .withPolicy(Qos.pf().History().withKeepLast(9))
                    .withPolicy(Qos.pf().ResourceLimits().withMaxSamplesPerInstance(4));
            assertThrows(InconsistentPolicyException.class,
                    () -> ((DomainParticipantImpl) rig.participant).createTopic(
                            "Inconsistent", QosFixtures.Item.class, inconsistent));
            assertThrows(InconsistentPolicyException.class,
                    () -> rig.participant.setDefaultTopicQos(inconsistent));
        }
    }

    @Test
    void aPublisherTakesAPartitionAndGroupDataAndRefusesAnotherPresentation() throws Exception {
        try (QosFixtures.Rig rig = new QosFixtures.Rig()) {
            Publisher publisher = rig.publisher;
            PublisherQos before = publisher.getQos();
            PublisherQos changed = before
                    .withPolicy(Qos.pf().Partition().withName(Arrays.asList("A", "B")))
                    .withPolicy(Qos.pf().GroupData().withValue(new byte[] {9, 8}, 0, 2))
                    .withPolicy(Qos.pf().EntityFactory().withAutoEnableCreatedEntities(false));
            publisher.setQos(changed);
            assertSame(changed, publisher.getQos());
            PublisherQos scoped = changed.withPolicy(Qos.policies().Presentation().withGroup());
            assertThrows(ImmutablePolicyException.class, () -> publisher.setQos(scoped));
            assertSame(changed, publisher.getQos());
        }
    }

    @Test
    void aSubscriberTakesAPartitionAndRefusesAnotherPresentation() throws Exception {
        try (QosFixtures.Rig rig = new QosFixtures.Rig()) {
            Subscriber subscriber = rig.subscriber;
            SubscriberQos changed = subscriber.getQos()
                    .withPolicy(Qos.pf().Partition().withName(Arrays.asList("A")))
                    .withPolicy(Qos.pf().GroupData().withValue(new byte[] {1}, 0, 1));
            subscriber.setQos(changed);
            assertSame(changed, subscriber.getQos());
            SubscriberQos ordered = changed.withPolicy(Qos.policies().Presentation().withOrderedAccess(true));
            assertThrows(ImmutablePolicyException.class, () -> subscriber.setQos(ordered));
            assertSame(changed, subscriber.getQos());
        }
    }

    @Test
    void aParticipantAndTheFactoryTakeNewUserDataAndEntityFactory() throws Exception {
        try (QosFixtures.Rig rig = new QosFixtures.Rig()) {
            DomainParticipant participant = rig.participant;
            DomainParticipantQos changed = participant.getQos()
                    .withPolicy(Qos.pf().UserData().withValue(new byte[] {7}, 0, 1))
                    .withPolicy(Qos.pf().EntityFactory().withAutoEnableCreatedEntities(true));
            participant.setQos(changed);
            assertSame(changed, participant.getQos());

            DomainParticipantFactoryQos factoryQos = Qos.factory().getQos();
            Qos.factory().setQos(factoryQos);
            assertSame(factoryQos, Qos.factory().getQos());
        }
    }

    // --- before enable -------------------------------------------------------------

    @Test
    void beforeEnableAnyPolicyChangesAndAfterEnableTheImmutableOnesDoNot() throws Exception {
        DomainParticipantQos lazy = DomainParticipantQosImpl.defaults(Qos.env())
                .withPolicy(Qos.pf().EntityFactory().withAutoEnableCreatedEntities(false));
        DomainParticipant participant = Qos.factory().createParticipant(
                QosFixtures.nextDomain(), lazy, null, null);
        try {
            Topic<QosFixtures.Item> topic = participant.createTopic("Items", QosFixtures.Item.class);
            Publisher publisher = participant.createPublisher();
            assertFalse(((org.zerodds.pub.PublisherImpl) publisher).isEnabled());
            DataWriter<QosFixtures.Item> writer = Qos.writer(publisher, topic, QosFixtures.SUPPORT, Qos.writer());

            DataWriterQos bestEffort = Qos.writer()
                    .withPolicy(Qos.policies().DataWriterReliability().withBestEffort())
                    .withPolicy(Qos.pf().History().withKeepLast(2));
            writer.setQos(bestEffort); // not enabled yet: every policy can change
            assertSame(bestEffort, writer.getQos());

            publisher.enable(); // enables what the publisher created while it was disabled
            assertThrows(ImmutablePolicyException.class,
                    () -> writer.setQos(bestEffort.withPolicy(Qos.pf().History().withKeepLast(3))));
            assertEquals(Reliability.Kind.BEST_EFFORT, writer.getQos().getReliability().getKind());
            assertEquals(2, writer.getQos().getHistory().getDepth());
            assertEquals(History.Kind.KEEP_LAST, writer.getQos().getHistory().getKind());
        } finally {
            participant.close();
        }
    }
}
