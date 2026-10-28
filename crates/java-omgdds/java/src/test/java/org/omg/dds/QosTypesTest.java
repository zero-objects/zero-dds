// SPDX-License-Identifier: Apache-2.0
package org.omg.dds;

import org.junit.jupiter.api.Test;
import org.omg.dds.core.EntityQos;
import org.omg.dds.core.ServiceEnvironment;
import org.omg.dds.core.policy.Deadline;
import org.omg.dds.core.policy.DestinationOrder;
import org.omg.dds.core.policy.Durability;
import org.omg.dds.core.policy.DurabilityService;
import org.omg.dds.core.policy.History;
import org.omg.dds.core.policy.LatencyBudget;
import org.omg.dds.core.policy.Liveliness;
import org.omg.dds.core.policy.Ownership;
import org.omg.dds.core.policy.OwnershipStrength;
import org.omg.dds.core.policy.Partition;
import org.omg.dds.core.policy.Presentation;
import org.omg.dds.core.policy.QosPolicy;
import org.omg.dds.core.policy.Reliability;
import org.omg.dds.core.policy.ResourceLimits;
import org.omg.dds.core.policy.UserData;
import org.omg.dds.pub.DataWriterQos;
import org.omg.dds.sub.DataReaderQos;
import org.zerodds.core.qos.DomainParticipantFactoryQosImpl;
import org.zerodds.core.qos.DomainParticipantQosImpl;

import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.concurrent.TimeUnit;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertNotSame;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * The QoS types of DDS-Java-PSM 1.0 §7.3.5: unmodifiable, changed only through the
 * {@code with...} DSL, values with equality and a name. Domain ids are not used; nothing here
 * creates an entity.
 */
class QosTypesTest {

    /** A policy for readers that is none of the policies of Table 7.2. */
    private static final class ForeignReaderPolicy implements QosPolicy.ForDataReader {
        private static final long serialVersionUID = 1L;

        @Override
        public ServiceEnvironment getEnvironment() {
            return Qos.env();
        }
    }

    private static List<EntityQos<?>> allQos() {
        List<EntityQos<?>> all = new ArrayList<>();
        all.add(Qos.writer());
        all.add(Qos.reader());
        all.add(Qos.topic());
        all.add(Qos.publisher());
        all.add(Qos.subscriber());
        all.add(DomainParticipantQosImpl.defaults(Qos.env()));
        all.add(DomainParticipantFactoryQosImpl.defaults(Qos.env()));
        return all;
    }

    // --- Defaults of every policy (DDS 1.4 §2.2.3, tables of the policies) ------------

    @Test
    void theDefaultsOfTheDataWriterQos() {
        DataWriterQos qos = Qos.writer();
        assertEquals(Durability.Kind.VOLATILE, qos.getDurability().getKind());
        assertEquals(0, qos.getDurabilityService().getServiceCleanupDelay().getDuration(TimeUnit.NANOSECONDS));
        assertEquals(History.Kind.KEEP_LAST, qos.getDurabilityService().getHistoryKind());
        assertEquals(1, qos.getDurabilityService().getHistoryDepth());
        assertEquals(ResourceLimits.LENGTH_UNLIMITED, qos.getDurabilityService().getMaxSamples());
        assertEquals(ResourceLimits.LENGTH_UNLIMITED, qos.getDurabilityService().getMaxInstances());
        assertEquals(ResourceLimits.LENGTH_UNLIMITED, qos.getDurabilityService().getMaxSamplesPerInstance());
        assertTrue(qos.getDeadline().getPeriod().isInfinite());
        assertTrue(qos.getLatencyBudget().getDuration().isZero());
        assertEquals(Liveliness.Kind.AUTOMATIC, qos.getLiveliness().getKind());
        assertTrue(qos.getLiveliness().getLeaseDuration().isInfinite());
        assertEquals(Reliability.Kind.RELIABLE, qos.getReliability().getKind());
        assertEquals(100, qos.getReliability().getMaxBlockingTime().getDuration(TimeUnit.MILLISECONDS));
        assertEquals(DestinationOrder.Kind.BY_RECEPTION_TIMESTAMP, qos.getDestinationOrder().getKind());
        assertEquals(History.Kind.KEEP_LAST, qos.getHistory().getKind());
        assertEquals(1, qos.getHistory().getDepth());
        assertEquals(ResourceLimits.LENGTH_UNLIMITED, qos.getResourceLimits().getMaxSamples());
        assertEquals(ResourceLimits.LENGTH_UNLIMITED, qos.getResourceLimits().getMaxInstances());
        assertEquals(ResourceLimits.LENGTH_UNLIMITED, qos.getResourceLimits().getMaxSamplesPerInstance());
        assertEquals(0, qos.getTransportPriority().getValue());
        assertTrue(qos.getLifespan().getDuration().isInfinite());
        assertEquals(0, qos.getUserData().getValue().length);
        assertEquals(Ownership.Kind.SHARED, qos.getOwnership().getKind());
        assertEquals(0, qos.getOwnershipStrength().getValue());
        assertTrue(qos.getWriterDataLifecycle().isAutDisposeUnregisteredInstances());
    }

    @Test
    void theDefaultsOfTheDataReaderQos() {
        DataReaderQos qos = Qos.reader();
        assertEquals(Durability.Kind.VOLATILE, qos.getDurability().getKind());
        assertTrue(qos.getDeadline().getPeriod().isInfinite());
        assertTrue(qos.getLatencyBudget().getDuration().isZero());
        assertEquals(Liveliness.Kind.AUTOMATIC, qos.getLiveliness().getKind());
        assertTrue(qos.getLiveliness().getLeaseDuration().isInfinite());
        assertEquals(Reliability.Kind.BEST_EFFORT, qos.get(Reliability.class).getKind());
        assertEquals(DestinationOrder.Kind.BY_RECEPTION_TIMESTAMP, qos.getDestinationOrder().getKind());
        assertEquals(History.Kind.KEEP_LAST, qos.getHistory().getKind());
        assertEquals(1, qos.getHistory().getDepth());
        assertEquals(ResourceLimits.LENGTH_UNLIMITED, qos.getResourceLimits().getMaxSamples());
        assertEquals(0, qos.getUserData().getValue().length);
        assertEquals(Ownership.Kind.SHARED, qos.getOwnership().getKind());
        assertTrue(qos.getTimeBasedFilter().getMinimumSeparation().isZero());
        assertTrue(qos.getReaderDataLifecycle().getAutoPurgeNoWriterSamplesDelay().isInfinite());
        assertTrue(qos.getReaderDataLifecycle().getAutoPurgeDisposedSamplesDelay().isInfinite());
    }

    @Test
    void theDefaultsOfTopicPublisherAndSubscriberQos() {
        assertEquals(0, Qos.topic().getTopicData().getValue().length);
        assertEquals(Reliability.Kind.BEST_EFFORT, Qos.topic().getReliability().getKind());
        assertEquals(Presentation.AccessScopeKind.INSTANCE, Qos.publisher().getPresentation().getAccessScope());
        assertFalse(Qos.publisher().getPresentation().isCoherentAccess());
        assertFalse(Qos.publisher().getPresentation().isOrderedAccess());
        assertEquals(Collections.singleton(""), Qos.publisher().getPartition().getName());
        assertEquals(0, Qos.publisher().getGroupData().getValue().length);
        assertTrue(Qos.publisher().getEntityFactory().isAutoEnableCreatedEntities());
        assertEquals(Presentation.AccessScopeKind.INSTANCE, Qos.subscriber().getPresentation().getAccessScope());
        assertEquals(Collections.singleton(""), Qos.subscriber().getPartition().getName());
        assertEquals(0, Qos.subscriber().getGroupData().getValue().length);
        assertTrue(Qos.subscriber().getEntityFactory().isAutoEnableCreatedEntities());
    }

    @Test
    void theDefaultOwnershipStrengthAndDurabilityServiceOfTheFactoryAreTheTableValues() {
        OwnershipStrength strength = Qos.pf().OwnershipStrength();
        assertEquals(0, strength.getValue());
        DurabilityService service = Qos.pf().DurabilityService();
        assertTrue(service.getServiceCleanupDelay().isZero());
        assertEquals(1, service.getHistoryDepth());
        assertEquals(ResourceLimits.LENGTH_UNLIMITED, service.getMaxSamples());
    }

    // --- The accessors of the QoS interfaces (§7.3.5.2) --------------------------------

    @Test
    void theTopicQosAnswersWhatItWasGivenForEveryPolicyOfTable72() {
        LatencyBudget budget = Qos.pf().LatencyBudget().withDuration(40, TimeUnit.MILLISECONDS);
        Liveliness liveliness = Qos.pf().Liveliness().withManualByParticipant().withLeaseDuration(3, TimeUnit.SECONDS);
        DestinationOrder order = Qos.pf().DestinationOrder().withSourceTimestamp();
        org.omg.dds.core.policy.Lifespan lifespan = Qos.pf().Lifespan().withDuration(9, TimeUnit.SECONDS);
        Ownership ownership = Qos.pf().Ownership().withExclusive();
        org.omg.dds.topic.TopicQos qos = Qos.topic()
                .withPolicy(budget).withPolicy(liveliness).withPolicy(order)
                .withPolicy(lifespan).withPolicy(ownership);
        assertEquals(budget, qos.getLatencyBudget());
        assertEquals(liveliness, qos.getLiveliness());
        assertEquals(order, qos.getDestinationOrder());
        assertEquals(lifespan, qos.getLifespan());
        assertEquals(ownership, qos.getOwnership());
        // the defaults are what the accessors answer before
        assertTrue(Qos.topic().getLatencyBudget().getDuration().isZero());
        assertEquals(Liveliness.Kind.AUTOMATIC, Qos.topic().getLiveliness().getKind());
        assertEquals(DestinationOrder.Kind.BY_RECEPTION_TIMESTAMP, Qos.topic().getDestinationOrder().getKind());
        assertTrue(Qos.topic().getLifespan().getDuration().isInfinite());
        assertEquals(Ownership.Kind.SHARED, Qos.topic().getOwnership().getKind());
    }

    @Test
    void theDataWriterQosAnswersWhatItWasGivenForTheRemainingPolicies() {
        ResourceLimits limits = Qos.pf().ResourceLimits().withMaxSamples(10).withMaxInstances(4);
        org.omg.dds.core.policy.TransportPriority priority = Qos.pf().TransportPriority().withValue(5);
        DurabilityService service = Qos.pf().DurabilityService().withHistoryDepth(2).withMaxSamples(8);
        org.omg.dds.core.policy.WriterDataLifecycle lifecycle =
                Qos.pf().WriterDataLifecycle().withAutDisposeUnregisteredInstances(false);
        DataWriterQos qos = Qos.writer().withPolicy(limits).withPolicy(priority)
                .withPolicy(service).withPolicy(lifecycle);
        assertEquals(limits, qos.getResourceLimits());
        assertEquals(10, qos.getResourceLimits().getMaxSamples());
        assertEquals(4, qos.getResourceLimits().getMaxInstances());
        assertEquals(ResourceLimits.LENGTH_UNLIMITED, qos.getResourceLimits().getMaxSamplesPerInstance());
        assertEquals(priority, qos.getTransportPriority());
        assertEquals(service, qos.getDurabilityService());
        assertEquals(lifecycle, qos.getWriterDataLifecycle());
    }

    @Test
    void theDataReaderQosAnswersWhatItWasGivenForTheRemainingPolicies() {
        LatencyBudget budget = Qos.pf().LatencyBudget().withDuration(15, TimeUnit.MILLISECONDS);
        DestinationOrder order = Qos.pf().DestinationOrder().withSourceTimestamp();
        org.omg.dds.core.policy.ReaderDataLifecycle lifecycle = Qos.pf().ReaderDataLifecycle()
                .withAutoPurgeNoWriterSamplesDelay(2, TimeUnit.SECONDS)
                .withAutoPurgeDisposedSamplesDelay(4, TimeUnit.SECONDS);
        DataReaderQos qos = Qos.reader().withPolicy(budget).withPolicy(order).withPolicy(lifecycle);
        assertEquals(budget, qos.getLatencyBudget());
        assertEquals(order, qos.getDestinationOrder());
        assertEquals(lifecycle, qos.getReaderDataLifecycle());
        assertEquals(2, qos.getReaderDataLifecycle().getAutoPurgeNoWriterSamplesDelay().getDuration(TimeUnit.SECONDS));
        assertEquals(4, qos.getReaderDataLifecycle().getAutoPurgeDisposedSamplesDelay().getDuration(TimeUnit.SECONDS));
    }

    @Test
    void thePublisherAndSubscriberQosAnswerPresentationEntityFactoryAndGroupData() {
        Presentation presentation = Qos.policies().Presentation().withGroup().withCoherentAccess(true);
        org.omg.dds.core.policy.EntityFactory factory = Qos.pf().EntityFactory().withAutoEnableCreatedEntities(false);
        org.omg.dds.core.policy.GroupData data = Qos.pf().GroupData().withValue(new byte[] {4, 5, 6}, 0, 3);
        org.omg.dds.pub.PublisherQos publisher = Qos.publisher()
                .withPolicy(presentation).withPolicy(factory).withPolicy(data);
        assertEquals(presentation, publisher.getPresentation());
        assertFalse(publisher.getEntityFactory().isAutoEnableCreatedEntities());
        assertArrayEquals(new byte[] {4, 5, 6}, publisher.getGroupData().getValue());
        org.omg.dds.sub.SubscriberQos subscriber = Qos.subscriber()
                .withPolicy(presentation).withPolicy(factory).withPolicy(data);
        assertEquals(presentation, subscriber.getPresentation());
        assertFalse(subscriber.getEntityFactory().isAutoEnableCreatedEntities());
        assertArrayEquals(new byte[] {4, 5, 6}, subscriber.getGroupData().getValue());
    }

    // --- Unmodifiable (§7.3.5.2) ------------------------------------------------------

    @Test
    void theMapOfAQosCannotBeChanged() {
        for (EntityQos<?> qos : allQos()) {
            @SuppressWarnings("unchecked")
            Map<Class<?>, QosPolicy> map = (Map<Class<?>, QosPolicy>) (Map<?, ?>) qos;
            String name = qos.getClass().getSimpleName();
            Class<?> someKey = qos.keySet().iterator().next();
            QosPolicy somePolicy = (QosPolicy) qos.values().iterator().next();
            assertThrows(UnsupportedOperationException.class, () -> map.put(someKey, somePolicy), name);
            assertThrows(UnsupportedOperationException.class, () -> map.remove(someKey), name);
            assertThrows(UnsupportedOperationException.class, () -> map.clear(), name);
            assertThrows(UnsupportedOperationException.class,
                    () -> map.putAll(Collections.singletonMap(someKey, somePolicy)), name);
            assertThrows(UnsupportedOperationException.class, () -> map.keySet().remove(someKey), name);
            assertThrows(UnsupportedOperationException.class, () -> map.values().clear(), name);
            assertThrows(UnsupportedOperationException.class,
                    () -> map.entrySet().iterator().remove(), name);
        }
    }

    @Test
    void withPolicyReturnsAnotherQosAndLeavesTheSourceAsItWas() {
        DataWriterQos original = Qos.writer();
        Reliability before = original.getReliability();
        DataWriterQos changed = original.withPolicy(Qos.policies().DataWriterReliability().withBestEffort());
        assertNotSame(original, changed);
        assertEquals(Reliability.Kind.RELIABLE, original.getReliability().getKind());
        assertSame(before, original.getReliability());
        assertEquals(Reliability.Kind.BEST_EFFORT, changed.getReliability().getKind());
        assertNotEquals(original, changed);
        // the policies that were not named are the ones of the source
        assertSame(original.getHistory(), changed.getHistory());
        assertEquals(original.size(), changed.size());
    }

    @Test
    void aQosFromAnEntityIsNotChangedByDerivingFromIt() throws Exception {
        try (QosFixtures.Rig rig = new QosFixtures.Rig()) {
            org.omg.dds.pub.DataWriter<QosFixtures.Item> writer = rig.writer(Qos.writer());
            DataWriterQos fromTheEntity = writer.getQos();
            DataWriterQos derived = fromTheEntity.withPolicy(Qos.pf().OwnershipStrength().withValue(77));
            assertNotSame(fromTheEntity, derived);
            assertEquals(0, writer.getQos().getOwnershipStrength().getValue());
            assertEquals(0, fromTheEntity.getOwnershipStrength().getValue());
            assertEquals(77, derived.getOwnershipStrength().getValue());
            assertSame(fromTheEntity, writer.getQos());
        }
    }

    // --- DSL: withPolicies and the policy classes (§7.3.5.3) ----------------------------

    @Test
    void withPoliciesAppliesSeveralPoliciesAtOnce() {
        DataWriterQos original = Qos.writer();
        DataWriterQos changed = original.withPolicies(
                Qos.pf().Deadline().withPeriod(2, TimeUnit.SECONDS),
                Qos.pf().OwnershipStrength().withValue(3),
                Qos.pf().Lifespan().withDuration(5, TimeUnit.SECONDS));
        assertNotSame(original, changed);
        assertEquals(2, changed.getDeadline().getPeriod().getDuration(TimeUnit.SECONDS));
        assertEquals(3, changed.getOwnershipStrength().getValue());
        assertEquals(5, changed.getLifespan().getDuration().getDuration(TimeUnit.SECONDS));
        assertTrue(original.getDeadline().getPeriod().isInfinite());
        assertEquals(0, original.getOwnershipStrength().getValue());
        // no policies: an equal QoS
        assertEquals(original, original.withPolicies());
    }

    @Test
    void aPolicyThatIsNoPolicyOfTheQosIsRefused() {
        DataReaderQos reader = Qos.reader();
        // a policy that names itself only as "for readers", not as one of Table 7.2
        assertThrows(IllegalArgumentException.class,
                () -> reader.withPolicy(new ForeignReaderPolicy()));
        assertThrows(IllegalArgumentException.class,
                () -> reader.withPolicies(new ForeignReaderPolicy()));
    }

    @Test
    void everyWithOfAPolicyReturnsANewObjectAndLeavesTheSource() {
        Reliability reliability = Qos.pf().Reliability();
        Reliability bestEffort = reliability.withBestEffort();
        assertNotSame(reliability, bestEffort);
        assertEquals(Reliability.Kind.RELIABLE, reliability.getKind());
        assertEquals(Reliability.Kind.BEST_EFFORT, bestEffort.getKind());

        History history = Qos.pf().History();
        History keepAll = history.withKeepAll();
        assertNotSame(history, keepAll);
        assertEquals(History.Kind.KEEP_LAST, history.getKind());
        History deeper = history.withDepth(6);
        assertEquals(1, history.getDepth());
        assertEquals(6, deeper.getDepth());

        ResourceLimits limits = Qos.pf().ResourceLimits();
        ResourceLimits changed = limits.withMaxSamples(3);
        assertNotSame(limits, changed);
        assertEquals(ResourceLimits.LENGTH_UNLIMITED, limits.getMaxSamples());
        assertEquals(3, changed.getMaxSamples());

        Deadline deadline = Qos.pf().Deadline();
        Deadline shorter = deadline.withPeriod(1, TimeUnit.SECONDS);
        assertNotSame(deadline, shorter);
        assertTrue(deadline.getPeriod().isInfinite());

        Partition partition = Qos.pf().Partition();
        Partition named = partition.withName(Arrays.asList("A", "B"));
        assertNotSame(partition, named);
        assertEquals(Collections.singleton(""), partition.getName());
        assertEquals(2, named.getName().size());

        UserData data = Qos.pf().UserData();
        UserData some = data.withValue(new byte[] {1, 2}, 0, 2);
        assertNotSame(data, some);
        assertEquals(0, data.getValue().length);
        assertEquals(2, some.getValue().length);

        Presentation presentation = Qos.policies().Presentation();
        Presentation group = presentation.withGroup();
        assertNotSame(presentation, group);
        assertEquals(Presentation.AccessScopeKind.INSTANCE, presentation.getAccessScope());

        Durability durability = Qos.pf().Durability();
        assertNotSame(durability, durability.withTransient());
        assertEquals(Durability.Kind.VOLATILE, durability.getKind());
    }

    @Test
    void withCallsChainOnePolicyIntoTheNext() {
        // the example of §7.3.5.3
        ResourceLimits limits = Qos.pf().ResourceLimits().withMaxSamples(20).withMaxInstances(6);
        assertEquals(20, limits.getMaxSamples());
        assertEquals(6, limits.getMaxInstances());
        assertEquals(ResourceLimits.LENGTH_UNLIMITED, limits.getMaxSamplesPerInstance());
        Liveliness liveliness = Qos.pf().Liveliness().withManualByTopic().withLeaseDuration(7, TimeUnit.SECONDS);
        assertEquals(Liveliness.Kind.MANUAL_BY_TOPIC, liveliness.getKind());
        assertEquals(7, liveliness.getLeaseDuration().getDuration(TimeUnit.SECONDS));
        // and a QoS chains too
        DataWriterQos qos = Qos.writer().withPolicy(limits).withPolicy(liveliness);
        assertEquals(limits, qos.getResourceLimits());
        assertEquals(liveliness, qos.getLiveliness());
    }

    // --- equals, hashCode, compareTo, names --------------------------------------------

    @Test
    void qosWithTheSamePoliciesAreEqualAndHaveTheSameHashCode() {
        assertEquals(Qos.writer(), Qos.writer());
        assertEquals(Qos.writer().hashCode(), Qos.writer().hashCode());
        assertEquals(Qos.reader(), Qos.reader());
        assertEquals(Qos.topic().hashCode(), Qos.topic().hashCode());
        DataWriterQos a = Qos.writer().withPolicy(Qos.pf().OwnershipStrength().withValue(2));
        DataWriterQos b = Qos.writer().withPolicy(Qos.pf().OwnershipStrength().withValue(2));
        assertEquals(a, b);
        assertEquals(a.hashCode(), b.hashCode());
        assertNotEquals(a, Qos.writer());
        assertNotEquals(Qos.writer(), null);
        // the QoS of two entity kinds are two kinds of value, whatever policies they share
        assertNotEquals(Qos.reader(), Qos.writer());
        Map<Object, String> byQos = new HashMap<>();
        byQos.put(a, "a");
        assertEquals("a", byQos.get(b));
    }

    @Test
    void policiesWithTheSameValueAreEqualAndHaveTheSameHashCode() {
        for (EntityQos<?> first : allQos()) {
            EntityQos<?> second = allQos().get(allQos().indexOf(first));
            for (Map.Entry<?, ?> entry : first.entrySet()) {
                Object a = entry.getValue();
                Object b = ((Map<?, ?>) second).get(entry.getKey());
                assertNotNull(b, String.valueOf(entry.getKey()));
                assertNotSame(a, b);
                assertEquals(a, b, String.valueOf(entry.getKey()));
                assertEquals(a.hashCode(), b.hashCode(), String.valueOf(entry.getKey()));
                assertNotEquals(a, null);
            }
        }
        assertNotEquals(Qos.pf().History().withKeepLast(2), Qos.pf().History().withKeepLast(3));
        assertNotEquals(Qos.pf().Deadline().withPeriod(1, TimeUnit.SECONDS),
                Qos.pf().Deadline().withPeriod(2, TimeUnit.SECONDS));
        assertNotEquals(Qos.pf().Ownership().withShared(), Qos.pf().Ownership().withExclusive());
    }

    @Test
    void everyPolicyIsNamedAfterItsInterfaceInItsStringForm() {
        for (EntityQos<?> qos : allQos()) {
            for (Map.Entry<?, ?> entry : qos.entrySet()) {
                Class<?> key = (Class<?>) entry.getKey();
                assertTrue(entry.getValue().toString().startsWith(key.getSimpleName() + "["),
                        key.getSimpleName() + " is " + entry.getValue());
                assertTrue(key.isInstance(entry.getValue()), key.getSimpleName());
            }
            assertTrue(qos.toString().startsWith(qos.getClass().getSimpleName()), qos.toString());
        }
    }

    @Test
    void theRequestOfferedPoliciesCompareByTheirOrder() {
        Durability volatileKind = Qos.pf().Durability().withVolatile();
        Durability transientKind = Qos.pf().Durability().withTransient();
        assertTrue(volatileKind.compareTo(transientKind) < 0);
        assertTrue(transientKind.compareTo(volatileKind) > 0);
        assertEquals(0, volatileKind.compareTo(Qos.pf().Durability()));

        Deadline shorter = Qos.pf().Deadline().withPeriod(1, TimeUnit.SECONDS);
        Deadline longer = Qos.pf().Deadline().withPeriod(2, TimeUnit.SECONDS);
        assertTrue(shorter.compareTo(longer) < 0);
        assertTrue(longer.compareTo(Qos.pf().Deadline()) < 0, "infinite is the longest");

        Liveliness automatic = Qos.pf().Liveliness().withAutomatic();
        Liveliness manual = Qos.pf().Liveliness().withManualByTopic();
        assertTrue(automatic.compareTo(manual) < 0);
        assertTrue(Qos.pf().Liveliness().withManualByTopic().withLeaseDuration(1, TimeUnit.SECONDS)
                .compareTo(Qos.pf().Liveliness().withManualByTopic().withLeaseDuration(2, TimeUnit.SECONDS)) < 0);

        Reliability bestEffort = Qos.pf().Reliability().withBestEffort();
        assertTrue(bestEffort.compareTo(Qos.pf().Reliability().withReliable()) < 0);

        assertTrue(Qos.pf().Ownership().withShared().compareTo(Qos.pf().Ownership().withExclusive()) < 0);
        assertTrue(Qos.pf().DestinationOrder().withReceptionTimestamp()
                .compareTo(Qos.pf().DestinationOrder().withSourceTimestamp()) < 0);
        assertTrue(Qos.pf().LatencyBudget().withDuration(1, TimeUnit.SECONDS)
                .compareTo(Qos.pf().LatencyBudget().withDuration(2, TimeUnit.SECONDS)) < 0);
        assertTrue(Qos.policies().Presentation().withInstance()
                .compareTo(Qos.policies().Presentation().withGroup()) < 0);
    }
}
