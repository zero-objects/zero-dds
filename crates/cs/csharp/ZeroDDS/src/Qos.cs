// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Qos.cs — public C# counterparts of the OMG DDS 1.4 §2.2.3 QoS policies
// and the 6 QoS sets (Spec §2.2.2.x.y get_qos/set_qos).
//
// API style: idiomatic C# with record structs for value-type
// policies and classes for QoS sets (mutable, builder-friendly).

using System;
using System.Collections.Generic;

namespace ZeroDDS.Qos;

// ======================================================================
// Enums
// ======================================================================

public enum ReliabilityKind : uint
{
    BestEffort = 1,
    Reliable = 2,
}

public enum DurabilityKind : uint
{
    Volatile = 0,
    TransientLocal = 1,
    Transient = 2,
    Persistent = 3,
}

public enum HistoryKind : uint
{
    KeepLast = 0,
    KeepAll = 1,
}

public enum OwnershipKind : uint
{
    Shared = 0,
    Exclusive = 1,
}

public enum LivelinessKind : uint
{
    Automatic = 0,
    ManualByParticipant = 1,
    ManualByTopic = 2,
}

public enum DestinationOrderKind : uint
{
    ByReceptionTimestamp = 0,
    BySourceTimestamp = 1,
}

public enum PresentationAccessScopeKind : uint
{
    Instance = 0,
    Topic = 1,
    Group = 2,
}

// ======================================================================
// Policy Records (Spec §2.2.3)
// ======================================================================

// The parameterless and the one-argument constructors give the defaults of
// DDS 1.4 §2.2.3 (the table of QoS policies). A `Core.Duration` parameter
// cannot default to anything but zero, and zero is the default of none of the
// durations below except LATENCY_BUDGET, TIME_BASED_FILTER and
// DURABILITY_SERVICE's cleanup delay: a reader that requested a DEADLINE of
// zero or a LIVELINESS lease of zero matched no writer with the defaults.

public sealed record class ReliabilityPolicy(ReliabilityKind Kind, Core.Duration MaxBlockingTime)
{
    /// <summary>DDS 1.4 §2.2.3.14: max_blocking_time 100 ms.</summary>
    public ReliabilityPolicy(ReliabilityKind kind = ReliabilityKind.Reliable)
        : this(kind, Core.Duration.FromMillis(100)) { }
}

public sealed record class DurabilityPolicy(DurabilityKind Kind = DurabilityKind.Volatile);

public sealed record class HistoryPolicy(
    HistoryKind Kind = HistoryKind.KeepLast,
    int Depth = 1);

public sealed record class DeadlinePolicy(Core.Duration Period)
{
    /// <summary>DDS 1.4 §2.2.3.7: an infinite period.</summary>
    public DeadlinePolicy() : this(Core.Duration.Infinite) { }
}

public sealed record class LatencyBudgetPolicy(Core.Duration Duration = default);

public sealed record class LifespanPolicy(Core.Duration Duration)
{
    /// <summary>DDS 1.4 §2.2.3.16: an infinite duration.</summary>
    public LifespanPolicy() : this(Core.Duration.Infinite) { }
}

public sealed record class TimeBasedFilterPolicy(Core.Duration MinimumSeparation = default);

public sealed record class LivelinessPolicy(LivelinessKind Kind, Core.Duration LeaseDuration)
{
    /// <summary>DDS 1.4 §2.2.3.11: AUTOMATIC with an infinite lease.</summary>
    public LivelinessPolicy(LivelinessKind kind = LivelinessKind.Automatic)
        : this(kind, Core.Duration.Infinite) { }
}

public sealed record class OwnershipPolicy(OwnershipKind Kind = OwnershipKind.Shared);

public sealed record class OwnershipStrengthPolicy(int Value = 0);

public sealed record class DestinationOrderPolicy(
    DestinationOrderKind Kind = DestinationOrderKind.ByReceptionTimestamp);

public sealed record class PresentationPolicy(
    PresentationAccessScopeKind AccessScope = PresentationAccessScopeKind.Instance,
    bool CoherentAccess = false,
    bool OrderedAccess = false);

/// <summary>DDS 1.4 §2.2.3.19: LENGTH_UNLIMITED (-1) for all three.</summary>
public sealed record class ResourceLimitsPolicy(
    int MaxSamples = -1,
    int MaxInstances = -1,
    int MaxSamplesPerInstance = -1);

public sealed record class TransportPriorityPolicy(int Value = 0);

public sealed record class EntityFactoryPolicy(bool AutoenableCreatedEntities = true);

public sealed record class WriterDataLifecyclePolicy(bool AutodisposeUnregisteredInstances = true);

public sealed record class ReaderDataLifecyclePolicy(
    Core.Duration AutopurgeNoWriterSamplesDelay,
    Core.Duration AutopurgeDisposedSamplesDelay)
{
    /// <summary>DDS 1.4 §2.2.3.22: both delays infinite.</summary>
    public ReaderDataLifecyclePolicy() : this(Core.Duration.Infinite, Core.Duration.Infinite) { }
}

public sealed record class DurabilityServicePolicy(
    Core.Duration ServiceCleanupDelay = default,
    HistoryKind HistoryKind = HistoryKind.KeepLast,
    int HistoryDepth = 1,
    int MaxSamples = -1,
    int MaxInstances = -1,
    int MaxSamplesPerInstance = -1);

public sealed class UserDataPolicy
{
    public byte[] Value { get; set; } = Array.Empty<byte>();
}

public sealed class TopicDataPolicy
{
    public byte[] Value { get; set; } = Array.Empty<byte>();
}

public sealed class GroupDataPolicy
{
    public byte[] Value { get; set; } = Array.Empty<byte>();
}

public sealed class PartitionPolicy
{
    public List<string> Names { get; set; } = new();
}

// ======================================================================
// QoS-Sets (Spec §2.2.3.3 .. §2.2.3.x)
// ======================================================================

public sealed class DomainParticipantFactoryQos
{
    public EntityFactoryPolicy EntityFactory { get; set; } = new();
}

public sealed class DomainParticipantQos
{
    public UserDataPolicy UserData { get; set; } = new();
    public EntityFactoryPolicy EntityFactory { get; set; } = new();
}

public sealed class TopicQos
{
    public DurabilityPolicy Durability { get; set; } = new();
    public DurabilityServicePolicy DurabilityService { get; set; } = new();
    public DeadlinePolicy Deadline { get; set; } = new();
    public LatencyBudgetPolicy LatencyBudget { get; set; } = new();
    public LivelinessPolicy Liveliness { get; set; } = new();
    public ReliabilityPolicy Reliability { get; set; } = new();
    public DestinationOrderPolicy DestinationOrder { get; set; } = new();
    public HistoryPolicy History { get; set; } = new();
    public ResourceLimitsPolicy ResourceLimits { get; set; } = new();
    public TransportPriorityPolicy TransportPriority { get; set; } = new();
    public LifespanPolicy Lifespan { get; set; } = new();
    public OwnershipPolicy Ownership { get; set; } = new();
    public TopicDataPolicy TopicData { get; set; } = new();
}

public sealed class PublisherQos
{
    public PresentationPolicy Presentation { get; set; } = new();
    public PartitionPolicy Partition { get; set; } = new();
    public GroupDataPolicy GroupData { get; set; } = new();
    public EntityFactoryPolicy EntityFactory { get; set; } = new();
}

public sealed class SubscriberQos
{
    public PresentationPolicy Presentation { get; set; } = new();
    public PartitionPolicy Partition { get; set; } = new();
    public GroupDataPolicy GroupData { get; set; } = new();
    public EntityFactoryPolicy EntityFactory { get; set; } = new();
}

public sealed class DataWriterQos
{
    public ReliabilityPolicy Reliability { get; set; } =
        new(ReliabilityKind.Reliable, Core.Duration.FromMillis(100));
    public DurabilityPolicy Durability { get; set; } = new();
    public DurabilityServicePolicy DurabilityService { get; set; } = new();
    public DeadlinePolicy Deadline { get; set; } = new();
    public LatencyBudgetPolicy LatencyBudget { get; set; } = new();
    public LivelinessPolicy Liveliness { get; set; } = new();
    public DestinationOrderPolicy DestinationOrder { get; set; } = new();
    public LifespanPolicy Lifespan { get; set; } = new();
    public OwnershipPolicy Ownership { get; set; } = new();
    public OwnershipStrengthPolicy OwnershipStrength { get; set; } = new();
    public PartitionPolicy Partition { get; set; } = new();
    public PresentationPolicy Presentation { get; set; } = new();
    public HistoryPolicy History { get; set; } = new();
    public ResourceLimitsPolicy ResourceLimits { get; set; } = new();
    public TransportPriorityPolicy TransportPriority { get; set; } = new();
    public WriterDataLifecyclePolicy WriterDataLifecycle { get; set; } = new();
    public UserDataPolicy UserData { get; set; } = new();
    public TopicDataPolicy TopicData { get; set; } = new();
    public GroupDataPolicy GroupData { get; set; } = new();
}

public sealed class DataReaderQos
{
    public ReliabilityPolicy Reliability { get; set; } =
        new(ReliabilityKind.BestEffort, Core.Duration.FromMillis(100));
    public DurabilityPolicy Durability { get; set; } = new();
    public DeadlinePolicy Deadline { get; set; } = new();
    public LatencyBudgetPolicy LatencyBudget { get; set; } = new();
    public LivelinessPolicy Liveliness { get; set; } = new();
    public DestinationOrderPolicy DestinationOrder { get; set; } = new();
    public OwnershipPolicy Ownership { get; set; } = new();
    public PartitionPolicy Partition { get; set; } = new();
    public PresentationPolicy Presentation { get; set; } = new();
    public HistoryPolicy History { get; set; } = new();
    public ResourceLimitsPolicy ResourceLimits { get; set; } = new();
    public TimeBasedFilterPolicy TimeBasedFilter { get; set; } = new();
    public ReaderDataLifecyclePolicy ReaderDataLifecycle { get; set; } = new();
    public UserDataPolicy UserData { get; set; } = new();
    public TopicDataPolicy TopicData { get; set; } = new();
    public GroupDataPolicy GroupData { get; set; } = new();
}
