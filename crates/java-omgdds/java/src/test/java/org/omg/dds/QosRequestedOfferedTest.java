// SPDX-License-Identifier: Apache-2.0
package org.omg.dds;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.omg.dds.core.policy.Deadline;
import org.omg.dds.core.policy.DestinationOrder;
import org.omg.dds.core.policy.Durability;
import org.omg.dds.core.policy.LatencyBudget;
import org.omg.dds.core.policy.Liveliness;
import org.omg.dds.core.policy.Ownership;
import org.omg.dds.core.policy.Presentation;
import org.omg.dds.core.policy.QosPolicy;
import org.omg.dds.core.policy.QosPolicyCount;
import org.omg.dds.core.policy.Reliability;
import org.omg.dds.pub.DataWriter;
import org.omg.dds.pub.DataWriterQos;
import org.omg.dds.sub.DataReader;
import org.omg.dds.sub.DataReaderQos;
import org.omg.dds.sub.SubscriberQos;
import org.omg.dds.pub.PublisherQos;
import org.omg.dds.core.status.OfferedIncompatibleQosStatus;
import org.omg.dds.core.status.RequestedIncompatibleQosStatus;
import org.zerodds.core.qos.QosCompatibility;
import org.zerodds.internal.InProcessBus;
import org.zerodds.rtps.QosParameters;

import java.util.Arrays;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.concurrent.TimeUnit;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * The Request/Offer rules of DDS 1.4 §2.2.3 as {@code QosCompatibility} evaluates them, and
 * their effect on the match of a reader and a writer (OFFERED_INCOMPATIBLE_QOS and
 * REQUESTED_INCOMPATIBLE_QOS, §2.2.4.1). Domain ids 1400 to 1499.
 */
class QosRequestedOfferedTest {

    @BeforeEach
    void setUp() {
        InProcessBus.instance().reset();
    }

    @AfterEach
    void tearDown() {
        InProcessBus.instance().reset();
    }

    private static boolean compatible(DataReaderQos requested, DataWriterQos offered) {
        return QosCompatibility.isCompatible(
                QosCompatibility.policiesOf(requested), QosCompatibility.policiesOf(offered));
    }

    private static Class<? extends QosPolicy> refusedBy(DataReaderQos requested, DataWriterQos offered) {
        return QosCompatibility.incompatiblePolicyWith(
                QosCompatibility.policiesOf(requested), QosCompatibility.policiesOf(offered));
    }

    // --- DURABILITY: offered kind >= requested kind (§2.2.3.4) --------------------

    @Test
    void durabilityOfferedMustBeAtLeastRequestedInTheOrderVolatileToPersistent() {
        // VOLATILE < TRANSIENT_LOCAL < TRANSIENT < PERSISTENT: the declaration order of the enum.
        assertEquals(Arrays.asList(Durability.Kind.VOLATILE, Durability.Kind.TRANSIENT_LOCAL,
                Durability.Kind.TRANSIENT, Durability.Kind.PERSISTENT),
                Arrays.asList(Durability.Kind.values()));
        for (Durability.Kind requested : Durability.Kind.values()) {
            for (Durability.Kind offered : Durability.Kind.values()) {
                DataReaderQos reader = Qos.reader().withPolicy(Qos.pf().Durability().withKind(requested));
                DataWriterQos writer = Qos.writer().withPolicy(Qos.pf().Durability().withKind(offered));
                boolean expected = offered.ordinal() >= requested.ordinal();
                assertEquals(expected, compatible(reader, writer),
                        "requested " + requested + ", offered " + offered);
                if (!expected) {
                    assertEquals(Durability.class, refusedBy(reader, writer));
                }
            }
        }
    }

    // --- RELIABILITY: the kind only (§2.2.3.14) ------------------------------------

    @Test
    void reliabilityIsComparedByKindAndNotByMaxBlockingTime() {
        DataReaderQos reader = Qos.reader().withPolicy(Qos.policies().DataReaderReliability()
                .withReliable().withMaxBlockingTime(10, TimeUnit.SECONDS));
        DataWriterQos writer = Qos.writer().withPolicy(Qos.policies().DataWriterReliability()
                .withMaxBlockingTime(1, TimeUnit.MILLISECONDS));
        // Both RELIABLE; the reader's max_blocking_time (10 s) is longer than the writer's (1 ms).
        assertTrue(compatible(reader, writer));

        DataWriterQos bestEffort = Qos.writer().withPolicy(Qos.policies().DataWriterReliability().withBestEffort());
        assertFalse(compatible(reader, bestEffort));
        assertEquals(Reliability.class, refusedBy(reader, bestEffort));

        // A BEST_EFFORT reader accepts what a RELIABLE writer offers.
        DataReaderQos lenient = Qos.reader().withPolicy(Qos.policies().DataReaderReliability().withBestEffort());
        assertTrue(compatible(lenient, writer));
    }

    // --- LATENCY_BUDGET: offered <= requested (§2.2.3.7) ----------------------------

    @Test
    void latencyBudgetOfferedMustNotExceedRequested() {
        DataReaderQos reader = Qos.reader().withPolicy(Qos.pf().LatencyBudget().withDuration(500, TimeUnit.MILLISECONDS));
        DataWriterQos longer = Qos.writer().withPolicy(Qos.pf().LatencyBudget().withDuration(1000, TimeUnit.MILLISECONDS));
        DataWriterQos equal = Qos.writer().withPolicy(Qos.pf().LatencyBudget().withDuration(500, TimeUnit.MILLISECONDS));
        DataWriterQos shorter = Qos.writer().withPolicy(Qos.pf().LatencyBudget().withDuration(100, TimeUnit.MILLISECONDS));
        assertFalse(compatible(reader, longer));
        assertEquals(LatencyBudget.class, refusedBy(reader, longer));
        assertTrue(compatible(reader, equal));
        assertTrue(compatible(reader, shorter));
        // The default is zero on both sides.
        assertTrue(compatible(Qos.reader(), Qos.writer()));
    }

    // --- DESTINATION_ORDER: offered >= requested (§2.2.3.17) ------------------------

    @Test
    void destinationOrderOfferedMustBeAtLeastRequested() {
        DataReaderQos bySource = Qos.reader().withPolicy(Qos.pf().DestinationOrder().withSourceTimestamp());
        DataReaderQos byReception = Qos.reader().withPolicy(Qos.pf().DestinationOrder().withReceptionTimestamp());
        DataWriterQos writerBySource = Qos.writer().withPolicy(Qos.pf().DestinationOrder().withSourceTimestamp());
        DataWriterQos writerByReception = Qos.writer().withPolicy(Qos.pf().DestinationOrder().withReceptionTimestamp());

        assertFalse(compatible(bySource, writerByReception));
        assertEquals(DestinationOrder.class, refusedBy(bySource, writerByReception));
        assertTrue(compatible(bySource, writerBySource));
        assertTrue(compatible(byReception, writerBySource));
        assertTrue(compatible(byReception, writerByReception));
    }

    // --- OWNERSHIP: the same kind (§2.2.3.9) ------------------------------------------

    @Test
    void ownershipKindsMustBeEqualInBothDirections() {
        DataReaderQos sharedReader = Qos.reader().withPolicy(Qos.pf().Ownership().withShared());
        DataReaderQos exclusiveReader = Qos.reader().withPolicy(Qos.pf().Ownership().withExclusive());
        DataWriterQos sharedWriter = Qos.writer().withPolicy(Qos.pf().Ownership().withShared());
        DataWriterQos exclusiveWriter = Qos.writer().withPolicy(Qos.pf().Ownership().withExclusive());

        assertTrue(compatible(sharedReader, sharedWriter));
        assertTrue(compatible(exclusiveReader, exclusiveWriter));
        assertFalse(compatible(sharedReader, exclusiveWriter));
        assertEquals(Ownership.class, refusedBy(sharedReader, exclusiveWriter));
        assertFalse(compatible(exclusiveReader, sharedWriter));
        assertEquals(Ownership.class, refusedBy(exclusiveReader, sharedWriter));
    }

    @Test
    void aSharedReaderAndAnExclusiveWriterDoNotMatchAndBothSideRecordIt() throws Exception {
        try (QosFixtures.Rig rig = new QosFixtures.Rig()) {
            DataReader<QosFixtures.Item> reader = rig.reader(
                    Qos.reader().withPolicy(Qos.pf().Ownership().withShared()));
            DataWriter<QosFixtures.Item> writer = rig.writer(
                    Qos.writer().withPolicy(Qos.pf().Ownership().withExclusive()));
            writer.write(new QosFixtures.Item(1, 1));

            assertEquals(0, Qos.take(reader).size(), "the writer is not a match, nothing arrives");
            RequestedIncompatibleQosStatus requested = reader.getRequestedIncompatibleQosStatus();
            assertEquals(1, requested.getTotalCount());
            assertEquals(Ownership.class, requested.getLastPolicyClass());
            OfferedIncompatibleQosStatus offered = writer.getOfferedIncompatibleQosStatus();
            assertEquals(1, offered.getTotalCount());
            assertEquals(Ownership.class, offered.getLastPolicyClass());
            assertEquals(0, reader.getSubscriptionMatchedStatus().getCurrentCount());
        }
    }

    // --- DEADLINE and LIVELINESS (§2.2.3.6, §2.2.3.11) ---------------------------------

    @Test
    void deadlineOfferedPeriodMustNotExceedRequested() {
        DataReaderQos reader = Qos.reader().withPolicy(Qos.pf().Deadline().withPeriod(100, TimeUnit.MILLISECONDS));
        DataWriterQos longer = Qos.writer().withPolicy(Qos.pf().Deadline().withPeriod(200, TimeUnit.MILLISECONDS));
        DataWriterQos shorter = Qos.writer().withPolicy(Qos.pf().Deadline().withPeriod(50, TimeUnit.MILLISECONDS));
        assertFalse(compatible(reader, longer));
        assertEquals(Deadline.class, refusedBy(reader, longer));
        assertTrue(compatible(reader, shorter));
        // The default period is infinite: a reader that asks for 100 ms is not served by it.
        assertFalse(compatible(reader, Qos.writer()));
        assertTrue(compatible(Qos.reader(), longer));
    }

    @Test
    void livelinessOfferedKindMustBeAtLeastRequestedAndTheLeaseNotLonger() {
        DataReaderQos byTopic = Qos.reader().withPolicy(Qos.pf().Liveliness().withManualByTopic());
        DataWriterQos automatic = Qos.writer().withPolicy(Qos.pf().Liveliness().withAutomatic());
        DataWriterQos byParticipant = Qos.writer().withPolicy(Qos.pf().Liveliness().withManualByParticipant());
        DataWriterQos writerByTopic = Qos.writer().withPolicy(Qos.pf().Liveliness().withManualByTopic());
        // AUTOMATIC < MANUAL_BY_PARTICIPANT < MANUAL_BY_TOPIC
        assertFalse(compatible(byTopic, automatic));
        assertEquals(Liveliness.class, refusedBy(byTopic, automatic));
        assertFalse(compatible(byTopic, byParticipant));
        assertTrue(compatible(byTopic, writerByTopic));
        assertTrue(compatible(Qos.reader().withPolicy(Qos.pf().Liveliness().withAutomatic()), writerByTopic));

        DataReaderQos fiveSeconds = Qos.reader().withPolicy(
                Qos.pf().Liveliness().withManualByTopic().withLeaseDuration(5, TimeUnit.SECONDS));
        DataWriterQos tenSeconds = Qos.writer().withPolicy(
                Qos.pf().Liveliness().withManualByTopic().withLeaseDuration(10, TimeUnit.SECONDS));
        DataWriterQos twoSeconds = Qos.writer().withPolicy(
                Qos.pf().Liveliness().withManualByTopic().withLeaseDuration(2, TimeUnit.SECONDS));
        assertFalse(compatible(fiveSeconds, tenSeconds));
        assertTrue(compatible(fiveSeconds, twoSeconds));
    }

    @Test
    void theWireSideRefusesAnOfferedLivelinessKindBelowTheRequestedOne() {
        // PID_LIVELINESS kinds on the wire: AUTOMATIC 0, MANUAL_BY_PARTICIPANT 1, MANUAL_BY_TOPIC 2.
        QosParameters offered = new QosParameters();
        QosParameters requested = new QosParameters();
        offered.liveliness = new QosParameters.Liveliness();
        requested.liveliness = new QosParameters.Liveliness();
        offered.liveliness.lease = new QosParameters.Dur(1, 0);
        requested.liveliness.lease = new QosParameters.Dur(1, 0);

        offered.liveliness.kind = 0;
        requested.liveliness.kind = 2;
        assertEquals("LIVELINESS", QosParameters.incompatibility(offered, requested));
        offered.liveliness.kind = 2;
        requested.liveliness.kind = 0;
        assertNull(QosParameters.incompatibility(offered, requested));
        offered.liveliness.kind = 1;
        requested.liveliness.kind = 1;
        assertNull(QosParameters.incompatibility(offered, requested));
    }

    // --- PRESENTATION: scope, coherent, ordered (§2.2.3.6) -----------------------------

    private static Map<Class<? extends QosPolicy>, QosPolicy> withPresentation(Presentation presentation) {
        return QosCompatibility.withPresentation(new HashMap<>(), presentation);
    }

    private static Presentation presentation(Presentation.AccessScopeKind scope, boolean coherent, boolean ordered) {
        return Qos.policies().Presentation()
                .withAccessScope(scope).withCoherentAccess(coherent).withOrderedAccess(ordered);
    }

    @Test
    void presentationOfferedScopeMustBeAtLeastRequestedAndCoherentAndOrderedMustBeOffered() {
        Presentation.AccessScopeKind instance = Presentation.AccessScopeKind.INSTANCE;
        Presentation.AccessScopeKind topic = Presentation.AccessScopeKind.TOPIC;
        Presentation.AccessScopeKind group = Presentation.AccessScopeKind.GROUP;
        // scope: INSTANCE < TOPIC < GROUP
        for (Presentation.AccessScopeKind requested : Presentation.AccessScopeKind.values()) {
            for (Presentation.AccessScopeKind offered : Presentation.AccessScopeKind.values()) {
                boolean expected = offered.ordinal() >= requested.ordinal();
                assertEquals(expected, QosCompatibility.isCompatible(
                        withPresentation(presentation(requested, false, false)),
                        withPresentation(presentation(offered, false, false))),
                        "requested " + requested + ", offered " + offered);
            }
        }
        assertEquals(Presentation.class, QosCompatibility.incompatiblePolicyWith(
                withPresentation(presentation(topic, false, false)),
                withPresentation(presentation(instance, false, false))));

        // coherent_access and ordered_access: requested true needs offered true.
        assertFalse(QosCompatibility.isCompatible(
                withPresentation(presentation(group, true, false)),
                withPresentation(presentation(group, false, false))));
        assertTrue(QosCompatibility.isCompatible(
                withPresentation(presentation(group, false, false)),
                withPresentation(presentation(group, true, false))));
        assertFalse(QosCompatibility.isCompatible(
                withPresentation(presentation(topic, false, true)),
                withPresentation(presentation(topic, false, false))));
        assertTrue(QosCompatibility.isCompatible(
                withPresentation(presentation(topic, true, true)),
                withPresentation(presentation(group, true, true))));
    }

    @Test
    void aSubscriberThatRequestsTopicScopeDoesNotMatchAPublisherThatOffersInstanceScope() throws Exception {
        PublisherQos publisherQos = Qos.publisher().withPolicy(
                Qos.policies().Presentation().withInstance());
        SubscriberQos subscriberQos = Qos.subscriber().withPolicy(
                Qos.policies().Presentation().withTopic());
        try (QosFixtures.Rig rig = new QosFixtures.Rig(publisherQos, subscriberQos)) {
            DataReader<QosFixtures.Item> reader = rig.reader(Qos.reader());
            DataWriter<QosFixtures.Item> writer = rig.writer(Qos.writer());
            writer.write(new QosFixtures.Item(1, 1));

            assertEquals(0, Qos.take(reader).size());
            RequestedIncompatibleQosStatus requested = reader.getRequestedIncompatibleQosStatus();
            assertEquals(1, requested.getTotalCount());
            assertEquals(Presentation.class, requested.getLastPolicyClass());
            assertEquals(1, writer.getOfferedIncompatibleQosStatus().getTotalCount());
        }
    }

    @Test
    void aSubscriberAndAPublisherWithTheSamePresentationMatch() throws Exception {
        PublisherQos publisherQos = Qos.publisher().withPolicy(
                Qos.policies().Presentation().withGroup().withCoherentAccess(true).withOrderedAccess(true));
        SubscriberQos subscriberQos = Qos.subscriber().withPolicy(
                Qos.policies().Presentation().withTopic().withCoherentAccess(true));
        try (QosFixtures.Rig rig = new QosFixtures.Rig(publisherQos, subscriberQos)) {
            DataReader<QosFixtures.Item> reader = rig.reader(Qos.reader());
            DataWriter<QosFixtures.Item> writer = rig.writer(Qos.writer());
            writer.write(new QosFixtures.Item(1, 1));
            assertEquals(1, Qos.take(reader).size());
            assertEquals(0, reader.getRequestedIncompatibleQosStatus().getTotalCount());
        }
    }

    // --- several incompatible policies are all counted (§2.2.4.1) ------------------------

    @Test
    void everyIncompatiblePolicyIsListedInTheOrderTheStatusCountsThem() {
        DataReaderQos reader = Qos.reader()
                .withPolicy(Qos.policies().DataReaderReliability().withReliable())
                .withPolicy(Qos.pf().Durability().withTransientLocal())
                .withPolicy(Qos.pf().Deadline().withPeriod(100, TimeUnit.MILLISECONDS));
        DataWriterQos writer = Qos.writer()
                .withPolicy(Qos.policies().DataWriterReliability().withBestEffort());
        List<Class<? extends QosPolicy>> refused = QosCompatibility.incompatiblePolicies(
                QosCompatibility.policiesOf(reader), QosCompatibility.policiesOf(writer));
        // RELIABILITY (BEST_EFFORT < RELIABLE), DURABILITY (VOLATILE < TRANSIENT_LOCAL),
        // DEADLINE (infinite > 100 ms).
        assertEquals(Arrays.<Class<? extends QosPolicy>>asList(
                Reliability.class, Durability.class, Deadline.class), refused);
        assertTrue(QosCompatibility.incompatiblePolicies(
                QosCompatibility.policiesOf(Qos.reader()), QosCompatibility.policiesOf(Qos.writer())).isEmpty());
    }

    @Test
    void aRefusedWriterCountsOnceAndEveryIncompatiblePolicyOnceOnBothSides() throws Exception {
        try (QosFixtures.Rig rig = new QosFixtures.Rig()) {
            DataReader<QosFixtures.Item> reader = rig.reader(Qos.reader()
                    .withPolicy(Qos.policies().DataReaderReliability().withReliable())
                    .withPolicy(Qos.pf().Durability().withTransientLocal())
                    .withPolicy(Qos.pf().Deadline().withPeriod(100, TimeUnit.MILLISECONDS)));
            DataWriter<QosFixtures.Item> writer = rig.writer(Qos.writer()
                    .withPolicy(Qos.policies().DataWriterReliability().withBestEffort()));

            RequestedIncompatibleQosStatus requested = reader.getRequestedIncompatibleQosStatus();
            assertEquals(1, requested.getTotalCount(), "one writer was refused");
            assertEquals(3, requested.getPolicies().size());
            for (QosPolicyCount count : requested.getPolicies()) {
                assertEquals(1, count.getCount(), count.getPolicyClass().getSimpleName());
            }
            OfferedIncompatibleQosStatus offered = writer.getOfferedIncompatibleQosStatus();
            assertEquals(1, offered.getTotalCount(), "one reader was refused");
            assertEquals(3, offered.getPolicies().size());
            for (QosPolicyCount count : offered.getPolicies()) {
                assertEquals(1, count.getCount(), count.getPolicyClass().getSimpleName());
            }
        }
    }

    @Test
    void aTransientLocalReaderIsRefusedByAVolatileWriter() throws Exception {
        try (QosFixtures.Rig rig = new QosFixtures.Rig()) {
            DataReader<QosFixtures.Item> reader = rig.reader(
                    Qos.reader().withPolicy(Qos.pf().Durability().withTransientLocal()));
            DataWriter<QosFixtures.Item> writer = rig.writer(Qos.writer());
            writer.write(new QosFixtures.Item(1, 1));
            assertEquals(0, Qos.take(reader).size());
            RequestedIncompatibleQosStatus requested = reader.getRequestedIncompatibleQosStatus();
            assertEquals(1, requested.getTotalCount());
            assertEquals(Durability.class, requested.getLastPolicyClass());
        }
    }
}
