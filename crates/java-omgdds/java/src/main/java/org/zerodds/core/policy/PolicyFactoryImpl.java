// SPDX-License-Identifier: Apache-2.0
package org.zerodds.core.policy;

import org.omg.dds.core.Duration;
import org.omg.dds.core.ServiceEnvironment;
import org.omg.dds.core.policy.DataRepresentation;
import org.omg.dds.core.policy.Deadline;
import org.omg.dds.core.policy.DestinationOrder;
import org.omg.dds.core.policy.Durability;
import org.omg.dds.core.policy.DurabilityService;
import org.omg.dds.core.policy.EntityFactory;
import org.omg.dds.core.policy.GroupData;
import org.omg.dds.core.policy.History;
import org.omg.dds.core.policy.LatencyBudget;
import org.omg.dds.core.policy.Lifespan;
import org.omg.dds.core.policy.Liveliness;
import org.omg.dds.core.policy.Ownership;
import org.omg.dds.core.policy.OwnershipStrength;
import org.omg.dds.core.policy.Partition;
import org.omg.dds.core.policy.PolicyFactory;
import org.omg.dds.core.policy.Presentation;
import org.omg.dds.core.policy.ReaderDataLifecycle;
import org.omg.dds.core.policy.Reliability;
import org.omg.dds.core.policy.ResourceLimits;
import org.omg.dds.core.policy.TimeBasedFilter;
import org.omg.dds.core.policy.TopicData;
import org.omg.dds.core.policy.TransportPriority;
import org.omg.dds.core.policy.TypeConsistencyEnforcement;
import org.omg.dds.core.policy.UserData;
import org.omg.dds.core.policy.WriterDataLifecycle;
import org.zerodds.core.DurationImpl;

import java.util.ArrayList;
import java.util.Collections;
import java.util.concurrent.TimeUnit;

/**
 * ZeroDDS {@link PolicyFactory} (DDS-Java-PSM 1.0 §7.3.5.1, Table 7.2).
 *
 * <p>Each operation returns a freshly allocated policy holding the value DDS
 * 1.2 §7.1.3 prescribes as the default. RELIABILITY and HISTORY have entity
 * dependent defaults, so {@link #DataWriterReliability()},
 * {@link #DataReaderReliability()} and {@link #DataReaderHistory()} name them
 * explicitly; the {@code Reliability()} and {@code History()} operations of
 * this class return the writer default.
 */
public final class PolicyFactoryImpl extends PolicyFactory {

    private final ServiceEnvironment environment;

    public PolicyFactoryImpl(ServiceEnvironment environment) {
        this.environment = environment;
    }

    @Override
    public ServiceEnvironment getEnvironment() {
        return environment;
    }

    @Override
    public Durability Durability() {
        return new DurabilityImpl(environment, Durability.Kind.VOLATILE);
    }

    @Override
    public Deadline Deadline() {
        return new DeadlineImpl(environment, DurationImpl.infinite(environment));
    }

    @Override
    public LatencyBudget LatencyBudget() {
        return new LatencyBudgetImpl(environment, DurationImpl.zero(environment));
    }

    @Override
    public Liveliness Liveliness() {
        return new LivelinessImpl(environment, Liveliness.Kind.AUTOMATIC,
                DurationImpl.infinite(environment));
    }

    @Override
    public DestinationOrder DestinationOrder() {
        return new DestinationOrderImpl(environment, DestinationOrder.Kind.BY_RECEPTION_TIMESTAMP);
    }

    @Override
    public History History() {
        return new HistoryImpl(environment, History.Kind.KEEP_LAST, 1);
    }

    @Override
    public ResourceLimits ResourceLimits() {
        return new ResourceLimitsImpl(environment, ResourceLimits.LENGTH_UNLIMITED,
                ResourceLimits.LENGTH_UNLIMITED, ResourceLimits.LENGTH_UNLIMITED);
    }

    @Override
    public UserData UserData() {
        return new UserDataImpl(environment, new byte[0]);
    }

    @Override
    public Ownership Ownership() {
        return new OwnershipImpl(environment, Ownership.Kind.SHARED);
    }

    @Override
    public TimeBasedFilter TimeBasedFilter() {
        return new TimeBasedFilterImpl(environment, DurationImpl.zero(environment));
    }

    @Override
    public ReaderDataLifecycle ReaderDataLifecycle() {
        org.omg.dds.core.Duration infinite = DurationImpl.infinite(environment);
        return new ReaderDataLifecycleImpl(environment, infinite, infinite);
    }

    @Override
    public DataRepresentation Representation() {
        return new DataRepresentationImpl(environment,
                new ArrayList<>(Collections.singletonList(
                        DataRepresentation.Id.XCDR_DATA_REPRESENTATION)));
    }

    @Override
    public TypeConsistencyEnforcement TypeConsistency() {
        return new TypeConsistencyEnforcementImpl(environment,
                TypeConsistencyEnforcement.Kind.EXACT_TYPE_TYPE_CONSISTENCY);
    }

    @Override
    public DurabilityService DurabilityService() {
        // DDS 1.4 §2.2.3 (Table of QoS policies): service_cleanup_delay 0, history KEEP_LAST
        // with depth 1, and the three resource limits LENGTH_UNLIMITED.
        return new DurabilityServiceImpl(environment, DurationImpl.zero(environment),
                History.Kind.KEEP_LAST, 1, ResourceLimits.LENGTH_UNLIMITED,
                ResourceLimits.LENGTH_UNLIMITED, ResourceLimits.LENGTH_UNLIMITED);
    }

    @Override
    public Reliability Reliability() {
        return DataWriterReliability();
    }

    @Override
    public TransportPriority TransportPriority() {
        return new TransportPriorityImpl(environment, 0);
    }

    @Override
    public Lifespan Lifespan() {
        return new LifespanImpl(environment, DurationImpl.infinite(environment));
    }

    @Override
    public OwnershipStrength OwnershipStrength() {
        return new OwnershipStrengthImpl(environment, 0);
    }

    @Override
    public WriterDataLifecycle WriterDataLifecycle() {
        return new WriterDataLifecycleImpl(environment, true);
    }

    @Override
    public Partition Partition() {
        return new PartitionImpl(environment, Collections.singleton(""));
    }

    @Override
    public GroupData GroupData() {
        return new GroupDataImpl(environment, new byte[0]);
    }

    @Override
    public EntityFactory EntityFactory() {
        return new EntityFactoryImpl(environment, true);
    }

    /** The PRESENTATION default (DDS 1.2 §7.1.3.8). */
    public Presentation Presentation() {
        return new PresentationImpl(environment, Presentation.AccessScopeKind.INSTANCE, false, false);
    }

    /** The TOPIC_DATA default (DDS 1.2 §7.1.3.3): a zero-length byte sequence. */
    public TopicData TopicData() {
        return new TopicDataImpl(environment, new byte[0]);
    }

    /** The RELIABILITY default of a DataWriter: RELIABLE (DDS 1.2 §7.1.3.6). */
    public Reliability DataWriterReliability() {
        return new ReliabilityImpl(environment, Reliability.Kind.RELIABLE,
                DurationImpl.of(environment, 100, TimeUnit.MILLISECONDS));
    }

    /** The RELIABILITY default of a DataReader and a Topic: BEST_EFFORT (DDS 1.2 §7.1.3.6). */
    public Reliability DataReaderReliability() {
        return new ReliabilityImpl(environment, Reliability.Kind.BEST_EFFORT,
                DurationImpl.of(environment, 100, TimeUnit.MILLISECONDS));
    }

    /** The HISTORY default of a DataReader or DataWriter: KEEP_LAST depth 1 (DDS 1.2 §7.1.3.7). */
    public History DataReaderHistory() {
        return new HistoryImpl(environment, History.Kind.KEEP_LAST, 1);
    }
}