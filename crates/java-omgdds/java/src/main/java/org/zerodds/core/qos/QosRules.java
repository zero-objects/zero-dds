// SPDX-License-Identifier: Apache-2.0
package org.zerodds.core.qos;

import org.omg.dds.core.EntityQos;
import org.omg.dds.core.Duration;
import org.omg.dds.core.policy.DataRepresentation;
import org.omg.dds.core.policy.Deadline;
import org.omg.dds.core.policy.DestinationOrder;
import org.omg.dds.core.policy.Durability;
import org.omg.dds.core.policy.DurabilityService;
import org.omg.dds.core.policy.History;
import org.omg.dds.core.policy.Liveliness;
import org.omg.dds.core.policy.Ownership;
import org.omg.dds.core.policy.Presentation;
import org.omg.dds.core.policy.QosPolicy;
import org.omg.dds.core.policy.Reliability;
import org.omg.dds.core.policy.ResourceLimits;
import org.omg.dds.core.policy.TimeBasedFilter;
import org.omg.dds.core.policy.TypeConsistencyEnforcement;
import org.zerodds.core.ImmutablePolicyException;
import org.zerodds.core.InconsistentPolicyException;

import java.util.Arrays;
import java.util.Collections;
import java.util.List;
import java.util.Map;

/**
 * The rules DDS 1.4 §2.2.3 puts on a set of QoS policies, apart from the
 * Request/Offer match of {@link QosCompatibility}: which policies can no longer
 * change once the entity is enabled ("Changeable" column), and which values
 * are inconsistent with each other.
 *
 * <p>An entity applies {@link #checkChange} in {@code setQos} and
 * {@link #checkConsistent} where it is created from a QoS; the exceptions are
 * the {@code ImmutablePolicyException} and {@code InconsistentPolicyException}
 * of DDS-Java-PSM 1.0 §7.3.2 (RETCODE_IMMUTABLE_POLICY, RETCODE_INCONSISTENT_POLICY).
 */
public final class QosRules {

    private QosRules() {}

    /**
     * The policies whose "Changeable" property is "No" in the tables of DDS 1.4
     * §2.2.3, plus the two XTypes 1.3 policies DATA_REPRESENTATION and
     * TYPE_CONSISTENCY_ENFORCEMENT, which are fixed when the entity is enabled.
     */
    private static final List<Class<? extends QosPolicy>> IMMUTABLE =
            Collections.unmodifiableList(Arrays.<Class<? extends QosPolicy>>asList(
                    Durability.class, DurabilityService.class, Presentation.class,
                    Ownership.class, Liveliness.class, Reliability.class,
                    DestinationOrder.class, History.class, ResourceLimits.class,
                    DataRepresentation.class, TypeConsistencyEnforcement.class));

    /** Whether DDS 1.4 §2.2.3 lets the application change {@code policy} after enable. */
    public static boolean isChangeable(Class<? extends QosPolicy> policy) {
        return !IMMUTABLE.contains(policy);
    }

    /**
     * The check of a {@code set_qos}: {@code proposed} must be consistent (checked first) and, when
     * the entity is {@code enabled}, must not differ from {@code current} in a policy
     * that is not changeable. Either exception leaves the entity's QoS as it was
     * (DDS 1.4 §2.2.3: "the previous values are retained").
     *
     * @throws IllegalArgumentException     if {@code proposed} is null.
     * @throws ImmutablePolicyException     if an immutable policy would change.
     * @throws InconsistentPolicyException  if the policies contradict each other.
     */
    public static void checkChange(boolean enabled, EntityQos<?> current, EntityQos<?> proposed) {
        if (proposed == null) {
            throw new IllegalArgumentException("the QoS must not be null");
        }
        // an inconsistent set is refused as such, whether or not it also changes an immutable policy
        checkConsistent(proposed);
        if (enabled && current != null) {
            for (Class<? extends QosPolicy> policy : IMMUTABLE) {
                Object before = at(current, policy);
                Object after = at(proposed, policy);
                if (before != null && after != null && !before.equals(after)) {
                    throw new ImmutablePolicyException("the policy " + policy.getSimpleName()
                            + " of an enabled entity cannot change from " + before
                            + " to " + after);
                }
            }
        }
    }

    /**
     * DDS 1.4 §2.2.3: the policies of {@code qos} must be consistent with one
     * another.
     * <ul>
     *   <li>HISTORY KEEP_LAST: depth &lt;= RESOURCE_LIMITS max_samples_per_instance
     *       (§2.2.3.18);</li>
     *   <li>RESOURCE_LIMITS: max_samples &gt;= max_samples_per_instance
     *       (§2.2.3.19);</li>
     *   <li>DEADLINE period &gt;= TIME_BASED_FILTER minimum_separation (§2.2.3.12);</li>
     *   <li>DURABILITY_SERVICE: the same two rules for its history and limits
     *       (§2.2.3.5).</li>
     * </ul>
     * A limit of {@link ResourceLimits#LENGTH_UNLIMITED} is never exceeded.
     *
     * @throws InconsistentPolicyException  if a rule is broken.
     */
    public static void checkConsistent(EntityQos<?> qos) {
        History history = at(qos, History.class);
        ResourceLimits limits = at(qos, ResourceLimits.class);
        if (limits != null) {
            requireLimits("RESOURCE_LIMITS", limits.getMaxSamples(),
                    limits.getMaxSamplesPerInstance());
            if (history != null && history.getKind() == History.Kind.KEEP_LAST) {
                requireDepth("HISTORY", history.getDepth(), limits.getMaxSamplesPerInstance());
            }
        }
        DurabilityService service = at(qos, DurabilityService.class);
        if (service != null) {
            requireLimits("DURABILITY_SERVICE", service.getMaxSamples(),
                    service.getMaxSamplesPerInstance());
            if (service.getHistoryKind() == History.Kind.KEEP_LAST) {
                requireDepth("DURABILITY_SERVICE history", service.getHistoryDepth(),
                        service.getMaxSamplesPerInstance());
            }
        }
        Deadline deadline = at(qos, Deadline.class);
        TimeBasedFilter filter = at(qos, TimeBasedFilter.class);
        if (deadline != null && filter != null) {
            Duration period = deadline.getPeriod();
            if (!period.isInfinite() && period.compareTo(filter.getMinimumSeparation()) < 0) {
                throw new InconsistentPolicyException("DEADLINE period " + period
                        + " is shorter than TIME_BASED_FILTER minimum_separation "
                        + filter.getMinimumSeparation());
            }
        }
    }

    private static void requireLimits(String policy, int maxSamples, int maxSamplesPerInstance) {
        if (maxSamples != ResourceLimits.LENGTH_UNLIMITED
                && maxSamplesPerInstance != ResourceLimits.LENGTH_UNLIMITED
                && maxSamples < maxSamplesPerInstance) {
            throw new InconsistentPolicyException(policy + " max_samples " + maxSamples
                    + " is less than max_samples_per_instance " + maxSamplesPerInstance);
        }
    }

    private static void requireDepth(String policy, int depth, int maxSamplesPerInstance) {
        if (maxSamplesPerInstance != ResourceLimits.LENGTH_UNLIMITED
                && depth > maxSamplesPerInstance) {
            throw new InconsistentPolicyException(policy + " depth " + depth
                    + " is greater than RESOURCE_LIMITS max_samples_per_instance "
                    + maxSamplesPerInstance);
        }
    }

    /** The policy of type {@code type} in {@code qos}, or null when the QoS holds none. */
    private static <P> P at(EntityQos<?> qos, Class<P> type) {
        Object value = ((Map<?, ?>) qos).get(type);
        return type.cast(value);
    }
}
