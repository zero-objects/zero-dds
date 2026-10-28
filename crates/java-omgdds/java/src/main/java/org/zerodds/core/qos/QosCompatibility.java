// SPDX-License-Identifier: Apache-2.0
package org.zerodds.core.qos;

import org.omg.dds.core.policy.Deadline;
import org.omg.dds.core.policy.DestinationOrder;
import org.omg.dds.core.policy.Durability;
import org.omg.dds.core.policy.LatencyBudget;
import org.omg.dds.core.policy.Liveliness;
import org.omg.dds.core.policy.Ownership;
import org.omg.dds.core.policy.Presentation;
import org.omg.dds.core.policy.QosPolicy;
import org.omg.dds.core.policy.Reliability;
import org.omg.dds.core.policy.RequestedOffered;

import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.concurrent.TimeUnit;

/**
 * The Requested/Offered compatibility of two sets of QoS policies
 * (DDS 1.2 §7.1.3, "QoS Compatibility").
 *
 * <p>A reader's QoS is the <em>requested</em> side and a writer's the
 * <em>offered</em> one; communication is established only where the offered
 * value is at least the requested one. The refusing policy is what the
 * OFFERED_INCOMPATIBLE_QOS and REQUESTED_INCOMPATIBLE_QOS statuses of DDS 1.2
 * §7.1.4.1 report.
 */
public final class QosCompatibility {

    private QosCompatibility() {}

    /**
     * Whether the {@code requested} policies of a reader are met by the
     * {@code offered} policies of a writer.
     *
     * @param requested  the reader's policies, by policy class.
     * @param offered    the writer's policies, by policy class.
     */
    public static boolean isCompatible(Map<Class<? extends QosPolicy>, ? extends QosPolicy> requested,
            Map<Class<? extends QosPolicy>, ? extends QosPolicy> offered) {
        return incompatiblePolicyWith(requested, offered) == null;
    }

    /**
     * The first RxO policy whose offered value does not meet the requested one,
     * or {@code null} when the two are compatible.
     */
    public static Class<? extends QosPolicy> incompatiblePolicyWith(
            Map<Class<? extends QosPolicy>, ? extends QosPolicy> requested,
            Map<Class<? extends QosPolicy>, ? extends QosPolicy> offered) {
        List<Class<? extends QosPolicy>> all = incompatiblePolicies(requested, offered);
        return all.isEmpty() ? null : all.get(0);
    }

    /**
     * Every RxO policy whose offered value does not meet the requested one, in
     * the order RELIABILITY, DURABILITY, DEADLINE, LATENCY_BUDGET, LIVELINESS,
     * OWNERSHIP, DESTINATION_ORDER, PRESENTATION; empty when the two are
     * compatible. DDS 1.4 §2.2.4.1 counts each incompatible policy in the
     * {@code policies} of REQUESTED_INCOMPATIBLE_QOS and OFFERED_INCOMPATIBLE_QOS,
     * so a caller records all of them, not only the first.
     */
    public static List<Class<? extends QosPolicy>> incompatiblePoliciesWith(
            Map<Class<? extends QosPolicy>, ? extends QosPolicy> requested,
            Map<Class<? extends QosPolicy>, ? extends QosPolicy> offered) {
        return incompatiblePolicies(requested, offered);
    }

    public static List<Class<? extends QosPolicy>> incompatiblePolicies(
            Map<Class<? extends QosPolicy>, ? extends QosPolicy> requested,
            Map<Class<? extends QosPolicy>, ? extends QosPolicy> offered) {
        List<Class<? extends QosPolicy>> out = new ArrayList<>();
        if (offered == null || offered.isEmpty() || requested == null) {
            return out;
        }
        for (Class<? extends QosPolicy> policy : RXO_POLICIES) {
            if (incompatible(requested.get(policy), offered.get(policy))) {
                out.add(policy);
            }
        }
        return out;
    }

    /**
     * The policies of DDS 1.4 §2.2.3 whose RxO property is "Yes" and that a
     * reader can request, in the order the status reports them.
     */
    private static final List<Class<? extends QosPolicy>> RXO_POLICIES =
            java.util.Collections.unmodifiableList(java.util.Arrays.<Class<? extends QosPolicy>>asList(
                    Reliability.class, Durability.class, Deadline.class, LatencyBudget.class,
                    Liveliness.class, Ownership.class, DestinationOrder.class, Presentation.class));

    /**
     * {@code policies} with the PRESENTATION of the Publisher or Subscriber the
     * endpoint belongs to: PRESENTATION is not a DataWriter or DataReader policy
     * (DDS-Java-PSM 1.0 §7.3.5.2) but takes part in the Request/Offer match of
     * the endpoints (DDS 1.4 §2.2.3.6).
     */
    public static Map<Class<? extends QosPolicy>, QosPolicy> withPresentation(
            Map<Class<? extends QosPolicy>, QosPolicy> policies, Presentation presentation) {
        if (presentation != null) {
            policies.put(Presentation.class, presentation);
        }
        return policies;
    }

    /**
     * One Request/Offer comparison: the offered policy must be at least the
     * requested one. Policies that do not implement
     * {@link RequestedOffered} have no Request/Offer contract (DDS 1.2 §7.1.3),
     * so every combination of their values is compatible.
     */
    @SuppressWarnings({"rawtypes", "unchecked"})
    private static boolean incompatible(QosPolicy requested, QosPolicy offered) {
        if (!(requested instanceof RequestedOffered) || offered == null) {
            return false;
        }
        Comparable contract = ((RequestedOffered) requested).requestedOfferedContract();
        return contract.compareTo(offered) > 0;
    }

    /**
     * The policies of any {@code EntityQos}, widened to the {@code QosPolicy}
     * key and value types, so a caller can compare two QoS of different
     * entity kinds (DDS 1.2 §2.2.2.3.7 {@code copy_from_topic_qos}).
     */
    public static Map<Class<? extends QosPolicy>, ? extends QosPolicy> wide(
            org.omg.dds.core.EntityQos<? extends QosPolicy> qos) {
        Map<Class<? extends QosPolicy>, org.omg.dds.core.policy.QosPolicy> out =
                new LinkedHashMap<>();
        for (Map.Entry<? extends Class<? extends QosPolicy>, ? extends QosPolicy> entry
                : qos.entrySet()) {
            out.put(castKey(entry.getKey()), entry.getValue());
        }
        return out;
    }

    @SuppressWarnings("unchecked")
    private static Class<? extends QosPolicy> castKey(
            Class<? extends QosPolicy> key) {
        return (Class<? extends QosPolicy>) key;
    }

    /**
     * The QoS policies of {@code qos} as a map from policy class to policy,
     * which is the shape {@link #isCompatible} compares.
     */
    @SuppressWarnings("unchecked") // every key of a QoS map is the Class of a policy (§7.3.5.2)
    public static Map<Class<? extends QosPolicy>, QosPolicy> policiesOf(
            java.util.Map<?, ? extends QosPolicy> qos) {
        Map<Class<? extends QosPolicy>, QosPolicy> out = new LinkedHashMap<>();
        for (Map.Entry<?, ? extends QosPolicy> entry : qos.entrySet()) {
            out.put((Class<? extends QosPolicy>) entry.getKey(), entry.getValue());
        }
        return out;
    }

    /** The deadline period of a policy map, in milliseconds; {@code -1} when infinite. */
    public static long periodMillis(Map<Class<? extends QosPolicy>, ? extends QosPolicy> qos) {
        Deadline deadline = (Deadline) qos.get(Deadline.class);
        if (deadline == null) {
            return -1;
        }
        return deadline.getPeriod().isInfinite() ? -1
                : deadline.getPeriod().getDuration(TimeUnit.MILLISECONDS);
    }
}