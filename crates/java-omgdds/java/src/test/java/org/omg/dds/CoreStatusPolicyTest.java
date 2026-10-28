// SPDX-License-Identifier: Apache-2.0
package org.omg.dds;

import org.junit.jupiter.api.Test;
import org.omg.dds.core.InstanceHandle;
import org.omg.dds.core.ServiceEnvironment;
import org.omg.dds.core.policy.GroupData;
import org.omg.dds.core.policy.Partition;
import org.omg.dds.core.policy.QosPolicy;
import org.omg.dds.core.policy.QosPolicyCount;
import org.omg.dds.core.policy.Reliability;
import org.omg.dds.core.policy.UserData;
import org.omg.dds.core.status.SampleRejectedStatus;
import org.zerodds.core.policy.TopicDataImpl;
import org.zerodds.core.status.DataAvailableStatusImpl;
import org.zerodds.core.status.DataOnReadersStatusImpl;
import org.zerodds.core.status.InconsistentTopicStatusImpl;
import org.zerodds.core.status.LivelinessChangedStatusImpl;
import org.zerodds.core.status.LivelinessLostStatusImpl;
import org.zerodds.core.status.OfferedDeadlineMissedStatusImpl;
import org.zerodds.core.status.OfferedIncompatibleQosStatusImpl;
import org.zerodds.core.status.PublicationMatchedStatusImpl;
import org.zerodds.core.status.QosPolicyCountImpl;
import org.zerodds.core.status.RequestedDeadlineMissedStatusImpl;
import org.zerodds.core.status.RequestedIncompatibleQosStatusImpl;
import org.zerodds.core.status.SampleLostStatusImpl;
import org.zerodds.core.status.SampleRejectedStatusImpl;
import org.zerodds.core.status.SubscriptionMatchedStatusImpl;

import java.lang.reflect.Method;
import java.util.Arrays;
import java.util.Collections;
import java.util.LinkedHashSet;
import java.util.Set;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotEquals;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * DDS-Java-PSM 1.0 §7.3.3 and §7.3.5.1: statuses have value semantics
 * (equals and hashCode over the counters and handles), and policies are
 * immutable: no getter hands out state that can be changed.
 */
class CoreStatusPolicyTest {

    private static final ServiceEnvironment ENV = Qos.env();

    private static InstanceHandle handle(int tag) {
        byte[] octets = new byte[16];
        octets[15] = (byte) tag;
        return org.zerodds.core.Values.of(ENV, octets);
    }

    private static Set<QosPolicyCount> counts(int reliabilityCount) {
        Set<QosPolicyCount> set = new LinkedHashSet<>();
        set.add(new QosPolicyCountImpl(ENV, Reliability.class, reliabilityCount));
        return set;
    }

    /** Two statuses built from the same values, and a third that differs in one value. */
    private static void assertValueSemantics(Object a, Object same, Object different) {
        assertEquals(a, same);
        assertEquals(same, a);
        assertEquals(a.hashCode(), same.hashCode());
        assertNotEquals(a, different);
        assertNotEquals(different, a);
        assertNotEquals(a, null);
        assertNotEquals(a, "a string");
    }

    @Test
    void everyStatusComparesByItsCountersAndHandles() {
        assertValueSemantics(
                new InconsistentTopicStatusImpl(ENV, 3, 1),
                new InconsistentTopicStatusImpl(ENV, 3, 1),
                new InconsistentTopicStatusImpl(ENV, 3, 2));
        assertValueSemantics(
                new LivelinessLostStatusImpl(ENV, 3, 1),
                new LivelinessLostStatusImpl(ENV, 3, 1),
                new LivelinessLostStatusImpl(ENV, 4, 1));
        assertValueSemantics(
                new SampleLostStatusImpl(ENV, 5, 2),
                new SampleLostStatusImpl(ENV, 5, 2),
                new SampleLostStatusImpl(ENV, 5, 3));
        assertValueSemantics(
                new LivelinessChangedStatusImpl(ENV, 1, 2, 3, 4, handle(1)),
                new LivelinessChangedStatusImpl(ENV, 1, 2, 3, 4, handle(1)),
                new LivelinessChangedStatusImpl(ENV, 1, 2, 3, 4, handle(2)));
        assertValueSemantics(
                new OfferedDeadlineMissedStatusImpl(ENV, 2, 1, handle(1)),
                new OfferedDeadlineMissedStatusImpl(ENV, 2, 1, handle(1)),
                new OfferedDeadlineMissedStatusImpl(ENV, 2, 1, handle(2)));
        assertValueSemantics(
                new RequestedDeadlineMissedStatusImpl(ENV, 2, 1, handle(1)),
                new RequestedDeadlineMissedStatusImpl(ENV, 2, 1, handle(1)),
                new RequestedDeadlineMissedStatusImpl(ENV, 3, 1, handle(1)));
        assertValueSemantics(
                new PublicationMatchedStatusImpl(ENV, 4, 1, 2, 1, handle(7)),
                new PublicationMatchedStatusImpl(ENV, 4, 1, 2, 1, handle(7)),
                new PublicationMatchedStatusImpl(ENV, 4, 1, 3, 1, handle(7)));
        assertValueSemantics(
                new SubscriptionMatchedStatusImpl(ENV, 4, 1, 2, 1, handle(7)),
                new SubscriptionMatchedStatusImpl(ENV, 4, 1, 2, 1, handle(7)),
                new SubscriptionMatchedStatusImpl(ENV, 4, 2, 2, 1, handle(7)));
        assertValueSemantics(
                new SampleRejectedStatusImpl(ENV, 1, 1,
                        SampleRejectedStatus.Kind.REJECTED_BY_SAMPLES_LIMIT, handle(1)),
                new SampleRejectedStatusImpl(ENV, 1, 1,
                        SampleRejectedStatus.Kind.REJECTED_BY_SAMPLES_LIMIT, handle(1)),
                new SampleRejectedStatusImpl(ENV, 1, 1,
                        SampleRejectedStatus.Kind.REJECTED_BY_INSTANCES_LIMIT, handle(1)));
        assertValueSemantics(
                new OfferedIncompatibleQosStatusImpl(ENV, 2, 1, Reliability.class, counts(2)),
                new OfferedIncompatibleQosStatusImpl(ENV, 2, 1, Reliability.class, counts(2)),
                new OfferedIncompatibleQosStatusImpl(ENV, 2, 1, Reliability.class, counts(3)));
        assertValueSemantics(
                new RequestedIncompatibleQosStatusImpl(ENV, 2, 1, Reliability.class, counts(2)),
                new RequestedIncompatibleQosStatusImpl(ENV, 2, 1, Reliability.class, counts(2)),
                new RequestedIncompatibleQosStatusImpl(ENV, 2, 1, UserData.class, counts(2)));
        assertValueSemantics(
                new QosPolicyCountImpl(ENV, Reliability.class, 2),
                new QosPolicyCountImpl(ENV, Reliability.class, 2),
                new QosPolicyCountImpl(ENV, Reliability.class, 3));
    }

    @Test
    void statusesWithoutACounterAreEqualWithinTheirKindOnly() {
        assertEquals(new DataAvailableStatusImpl(ENV), new DataAvailableStatusImpl(ENV));
        assertEquals(new DataAvailableStatusImpl(ENV).hashCode(), new DataAvailableStatusImpl(ENV).hashCode());
        assertEquals(new DataOnReadersStatusImpl(ENV), new DataOnReadersStatusImpl(ENV));
        assertNotEquals(new DataAvailableStatusImpl(ENV), new DataOnReadersStatusImpl(ENV));
    }

    @Test
    void theHandleOfAStatusIsComparedByValueNotByIdentity() {
        // two distinct handle objects of the same octets
        assertEquals(
                new PublicationMatchedStatusImpl(ENV, 1, 1, 1, 1, handle(9)),
                new PublicationMatchedStatusImpl(ENV, 1, 1, 1, 1, handle(9).modifiableCopy()));
    }

    @Test
    void theIncompatiblePoliciesOfAStatusCannotBeChanged() {
        OfferedIncompatibleQosStatusImpl status =
                new OfferedIncompatibleQosStatusImpl(ENV, 1, 1, Reliability.class, counts(1));
        Set<QosPolicyCount> policies = status.getPolicies();
        assertThrows(UnsupportedOperationException.class,
                () -> policies.add(new QosPolicyCountImpl(ENV, UserData.class, 1)));
        assertThrows(UnsupportedOperationException.class, policies::clear);
        assertEquals(1, status.getPolicies().size());
    }

    // ---- policies ----------------------------------------------------------

    @Test
    void userDataKeepsACopyOfTheArrayItWasGivenAndHandsOutCopies() {
        byte[] source = {1, 2, 3};
        UserData data = new org.zerodds.core.policy.UserDataImpl(ENV, source);
        source[0] = 99;
        assertArrayEquals(new byte[] {1, 2, 3}, data.getValue());
        byte[] returned = data.getValue();
        returned[1] = 77;
        assertArrayEquals(new byte[] {1, 2, 3}, data.getValue());
    }

    @Test
    void topicDataAndGroupDataHandOutCopies() {
        TopicDataImpl topicData = new TopicDataImpl(ENV, new byte[] {4, 5});
        topicData.getValue()[0] = 0;
        assertArrayEquals(new byte[] {4, 5}, topicData.getValue());
        GroupData groupData = Qos.policies().GroupData().withValue(new byte[] {6, 7}, 0, 2);
        groupData.getValue()[0] = 0;
        assertArrayEquals(new byte[] {6, 7}, groupData.getValue());
    }

    @Test
    void withValueLeavesTheOriginalUnchanged() {
        UserData original = Qos.pf().UserData().withValue(new byte[] {1, 2, 3, 4}, 0, 4);
        UserData changed = original.withValue(new byte[] {9, 8, 7, 6}, 1, 2);
        // offset 1, length 2 of {9,8,7,6} is {8,7}
        assertArrayEquals(new byte[] {8, 7}, changed.getValue());
        assertArrayEquals(new byte[] {1, 2, 3, 4}, original.getValue());
        assertEquals(Qos.pf().UserData().withValue(new byte[] {1, 2, 3, 4}, 0, 4), original);
        assertEquals(Qos.pf().UserData().withValue(new byte[] {1, 2, 3, 4}, 0, 4).hashCode(),
                original.hashCode());
    }

    @Test
    void aPartitionCannotBeChangedThroughItsNames() {
        Set<String> names = new LinkedHashSet<>(Arrays.asList("a", "b"));
        Partition partition = Qos.pf().Partition().withName(names);
        names.add("c");
        assertEquals(2, partition.getName().size());
        Set<String> returned = partition.getName();
        assertThrows(UnsupportedOperationException.class, () -> returned.add("c"));
        assertThrows(UnsupportedOperationException.class, returned::clear);
        Partition other = partition.withName(Collections.singleton("z"));
        assertEquals(Collections.singleton("z"), other.getName());
        assertEquals(new LinkedHashSet<>(Arrays.asList("a", "b")), partition.getName());
    }

    @Test
    void withPolicyLeavesTheOriginalQosUnchanged() {
        org.omg.dds.pub.DataWriterQos original = Qos.writer();
        Reliability before = original.getReliability();
        org.omg.dds.pub.DataWriterQos changed = original.withPolicy(Qos.pf().Reliability().withBestEffort());
        assertEquals(before, original.getReliability());
        assertEquals(Reliability.Kind.BEST_EFFORT, changed.getReliability().getKind());
        assertNotEquals(original, changed);
    }

    @Test
    void equalQosHaveEqualHashCodes() {
        assertEquals(Qos.writer(), Qos.writer());
        assertEquals(Qos.writer().hashCode(), Qos.writer().hashCode());
        assertEquals(Qos.reader(), Qos.reader());
        assertEquals(Qos.reader().hashCode(), Qos.reader().hashCode());
        assertEquals(Qos.topic().hashCode(), Qos.topic().hashCode());
    }

    @Test
    void equalPoliciesHaveEqualHashCodes() throws Exception {
        for (Method factory : org.omg.dds.core.policy.PolicyFactory.class.getDeclaredMethods()) {
            if (factory.getParameterCount() != 0 || !QosPolicy.class.isAssignableFrom(factory.getReturnType())) {
                continue;
            }
            Object a = factory.invoke(Qos.pf());
            Object b = factory.invoke(Qos.pf());
            assertEquals(a, b, factory.getName());
            assertEquals(a.hashCode(), b.hashCode(), factory.getName());
        }
    }

    @Test
    void thePolicyNameIsTheSimpleNameOfItsClass() throws Exception {
        // §7.3.5.1 Table 7.2: the name of a policy is the name of its type
        String[] names = {"UserData", "TopicData", "GroupData", "TransportPriority", "Lifespan",
            "Durability", "Presentation", "Deadline", "LatencyBudget", "Ownership", "OwnershipStrength",
            "Liveliness", "TimeBasedFilter", "Partition", "Reliability", "DestinationOrder", "History",
            "ResourceLimits", "EntityFactory", "WriterDataLifecycle", "ReaderDataLifecycle",
            "DurabilityService"};
        for (String name : names) {
            Class<?> type = Class.forName("org.omg.dds.core.policy." + name);
            assertEquals(name, type.getSimpleName());
            assertTrue(QosPolicy.class.isAssignableFrom(type), name + " is a QosPolicy");
            assertTrue(type.isInterface(), name + " is an interface");
            if (!"TopicData".equals(name) && !"Presentation".equals(name)) {
                // the factory method of the policy carries the same name
                Method factory = org.omg.dds.core.policy.PolicyFactory.class.getMethod(name);
                assertEquals(type, factory.getReturnType());
            }
        }
    }
}
