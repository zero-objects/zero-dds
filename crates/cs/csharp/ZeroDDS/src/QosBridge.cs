// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// QosBridge.cs — converts ZeroDDS.Qos.* → C-FFI Native.* structs.

using System;
using System.Runtime.InteropServices;
using ZeroDDS.Core;
using ZeroDDS.Qos;

namespace ZeroDDS.QosBridge;

/// <summary>Holds GCHandles for byte[] arrays passed via pointer to native.
/// Caller MUST keep the bridge alive for the duration of the native call,
/// then dispose it to free the GCHandles.</summary>
public sealed class NativeQosScope : IDisposable
{
    private readonly System.Collections.Generic.List<GCHandle> _handles = new();
    private bool _disposed;

    internal IntPtr Pin(byte[] data)
    {
        if (data == null || data.Length == 0) return IntPtr.Zero;
        var h = GCHandle.Alloc(data, GCHandleType.Pinned);
        _handles.Add(h);
        return h.AddrOfPinnedObject();
    }

    public void Dispose()
    {
        if (_disposed) return;
        _disposed = true;
        foreach (var h in _handles)
        {
            if (h.IsAllocated) h.Free();
        }
        _handles.Clear();
        GC.SuppressFinalize(this);
    }
    ~NativeQosScope() { Dispose(); }
}

internal static class DurationConv
{
    internal static (int sec, uint nanosec) ToNative(Duration d) => (d.Sec, d.Nanosec);
}

public static class QosBridge
{
    [StructLayout(LayoutKind.Sequential)]
    internal struct NativeDuration
    {
        public int Sec;
        public uint Nanosec;
    }

    [StructLayout(LayoutKind.Sequential)]
    internal struct NativeReliability
    {
        public uint Kind;
        public NativeDuration MaxBlockingTime;
    }
    [StructLayout(LayoutKind.Sequential)]
    internal struct NativeDurability { public uint Kind; }
    [StructLayout(LayoutKind.Sequential)]
    internal struct NativeHistory { public uint Kind; public int Depth; }
    [StructLayout(LayoutKind.Sequential)]
    internal struct NativeDeadline { public NativeDuration Period; }
    [StructLayout(LayoutKind.Sequential)]
    internal struct NativeLifespan { public NativeDuration Duration; }
    [StructLayout(LayoutKind.Sequential)]
    internal struct NativeLatencyBudget { public NativeDuration Duration; }
    [StructLayout(LayoutKind.Sequential)]
    internal struct NativeTimeBasedFilter { public NativeDuration MinimumSeparation; }
    [StructLayout(LayoutKind.Sequential)]
    internal struct NativeLiveliness { public uint Kind; public NativeDuration LeaseDuration; }
    [StructLayout(LayoutKind.Sequential)]
    internal struct NativeOwnership { public uint Kind; }
    [StructLayout(LayoutKind.Sequential)]
    internal struct NativeOwnershipStrength { public int Value; }
    [StructLayout(LayoutKind.Sequential)]
    internal struct NativeDestinationOrder { public uint Kind; }
    [StructLayout(LayoutKind.Sequential)]
    internal struct NativePresentation
    {
        public uint AccessScope;
        [MarshalAs(UnmanagedType.U1)] public bool CoherentAccess;
        [MarshalAs(UnmanagedType.U1)] public bool OrderedAccess;
    }
    [StructLayout(LayoutKind.Sequential)]
    internal struct NativeResourceLimits
    {
        public int MaxSamples;
        public int MaxInstances;
        public int MaxSamplesPerInstance;
    }
    [StructLayout(LayoutKind.Sequential)]
    internal struct NativeTransportPriority { public int Value; }
    [StructLayout(LayoutKind.Sequential)]
    internal struct NativeEntityFactory
    {
        [MarshalAs(UnmanagedType.U1)] public bool AutoenableCreatedEntities;
    }
    [StructLayout(LayoutKind.Sequential)]
    internal struct NativeWriterDataLifecycle
    {
        [MarshalAs(UnmanagedType.U1)] public bool AutodisposeUnregisteredInstances;
    }
    [StructLayout(LayoutKind.Sequential)]
    internal struct NativeReaderDataLifecycle
    {
        public NativeDuration AutopurgeNoWriterSamplesDelay;
        public NativeDuration AutopurgeDisposedSamplesDelay;
    }
    [StructLayout(LayoutKind.Sequential)]
    internal struct NativeDurabilityService
    {
        public NativeDuration ServiceCleanupDelay;
        public uint HistoryKind;
        public int HistoryDepth;
        public int MaxSamples;
        public int MaxInstances;
        public int MaxSamplesPerInstance;
    }
    [StructLayout(LayoutKind.Sequential)]
    internal struct NativeBytesData
    {
        public IntPtr Value;
        public UIntPtr ValueLen;
    }
    [StructLayout(LayoutKind.Sequential)]
    internal struct NativePartition
    {
        public IntPtr Names;
        public UIntPtr NamesLen;
    }

    [StructLayout(LayoutKind.Sequential)]
    public struct NativeDomainParticipantQos
    {
        internal NativeBytesData UserData;
        internal NativeEntityFactory EntityFactory;
    }

    [StructLayout(LayoutKind.Sequential)]
    public struct NativeTopicQos
    {
        internal NativeDurability Durability;
        internal NativeDurabilityService DurabilityService;
        internal NativeDeadline Deadline;
        internal NativeLatencyBudget LatencyBudget;
        internal NativeLiveliness Liveliness;
        internal NativeReliability Reliability;
        internal NativeDestinationOrder DestinationOrder;
        internal NativeHistory History;
        internal NativeResourceLimits ResourceLimits;
        internal NativeTransportPriority TransportPriority;
        internal NativeLifespan Lifespan;
        internal NativeOwnership Ownership;
        internal NativeBytesData TopicData;
    }

    [StructLayout(LayoutKind.Sequential)]
    public struct NativePublisherQos
    {
        internal NativePresentation Presentation;
        internal NativePartition Partition;
        internal NativeBytesData GroupData;
        internal NativeEntityFactory EntityFactory;
    }

    [StructLayout(LayoutKind.Sequential)]
    public struct NativeDataWriterQos
    {
        internal NativeReliability Reliability;
        internal NativeDurability Durability;
        internal NativeDurabilityService DurabilityService;
        internal NativeDeadline Deadline;
        internal NativeLatencyBudget LatencyBudget;
        internal NativeLiveliness Liveliness;
        internal NativeDestinationOrder DestinationOrder;
        internal NativeLifespan Lifespan;
        internal NativeOwnership Ownership;
        internal NativeOwnershipStrength OwnershipStrength;
        internal NativePartition Partition;
        internal NativePresentation Presentation;
        internal NativeHistory History;
        internal NativeResourceLimits ResourceLimits;
        internal NativeTransportPriority TransportPriority;
        internal NativeWriterDataLifecycle WriterDataLifecycle;
        internal NativeBytesData UserData;
        internal NativeBytesData TopicData;
        internal NativeBytesData GroupData;
    }

    [StructLayout(LayoutKind.Sequential)]
    public struct NativeDataReaderQos
    {
        internal NativeReliability Reliability;
        internal NativeDurability Durability;
        internal NativeDeadline Deadline;
        internal NativeLatencyBudget LatencyBudget;
        internal NativeLiveliness Liveliness;
        internal NativeDestinationOrder DestinationOrder;
        internal NativeOwnership Ownership;
        internal NativePartition Partition;
        internal NativePresentation Presentation;
        internal NativeHistory History;
        internal NativeResourceLimits ResourceLimits;
        internal NativeTimeBasedFilter TimeBasedFilter;
        internal NativeReaderDataLifecycle ReaderDataLifecycle;
        internal NativeBytesData UserData;
        internal NativeBytesData TopicData;
        internal NativeBytesData GroupData;
    }

    private static NativeDuration N(Duration d) =>
        new() { Sec = d.Sec, Nanosec = d.Nanosec };

    public static NativeDomainParticipantQos ToNative(DomainParticipantQos q, NativeQosScope scope)
    {
        return new NativeDomainParticipantQos
        {
            UserData = new NativeBytesData
            {
                Value = scope.Pin(q.UserData.Value),
                ValueLen = (UIntPtr)q.UserData.Value.Length,
            },
            EntityFactory = new NativeEntityFactory
            {
                AutoenableCreatedEntities = q.EntityFactory.AutoenableCreatedEntities,
            },
        };
    }

    public static NativeTopicQos ToNative(TopicQos q, NativeQosScope scope)
    {
        return new NativeTopicQos
        {
            Durability = new NativeDurability { Kind = (uint)q.Durability.Kind },
            DurabilityService = new NativeDurabilityService
            {
                ServiceCleanupDelay = N(q.DurabilityService.ServiceCleanupDelay),
                HistoryKind = (uint)q.DurabilityService.HistoryKind,
                HistoryDepth = q.DurabilityService.HistoryDepth,
                MaxSamples = q.DurabilityService.MaxSamples,
                MaxInstances = q.DurabilityService.MaxInstances,
                MaxSamplesPerInstance = q.DurabilityService.MaxSamplesPerInstance,
            },
            Deadline = new NativeDeadline { Period = N(q.Deadline.Period) },
            LatencyBudget = new NativeLatencyBudget { Duration = N(q.LatencyBudget.Duration) },
            Liveliness = new NativeLiveliness
            {
                Kind = (uint)q.Liveliness.Kind,
                LeaseDuration = N(q.Liveliness.LeaseDuration),
            },
            Reliability = new NativeReliability
            {
                Kind = (uint)q.Reliability.Kind,
                MaxBlockingTime = N(q.Reliability.MaxBlockingTime),
            },
            DestinationOrder = new NativeDestinationOrder { Kind = (uint)q.DestinationOrder.Kind },
            History = new NativeHistory
            {
                Kind = (uint)q.History.Kind,
                Depth = q.History.Depth,
            },
            ResourceLimits = new NativeResourceLimits
            {
                MaxSamples = q.ResourceLimits.MaxSamples,
                MaxInstances = q.ResourceLimits.MaxInstances,
                MaxSamplesPerInstance = q.ResourceLimits.MaxSamplesPerInstance,
            },
            TransportPriority = new NativeTransportPriority { Value = q.TransportPriority.Value },
            Lifespan = new NativeLifespan { Duration = N(q.Lifespan.Duration) },
            Ownership = new NativeOwnership { Kind = (uint)q.Ownership.Kind },
            TopicData = new NativeBytesData
            {
                Value = scope.Pin(q.TopicData.Value),
                ValueLen = (UIntPtr)q.TopicData.Value.Length,
            },
        };
    }

    public static NativePublisherQos ToNative(PublisherQos q, NativeQosScope scope)
    {
        return new NativePublisherQos
        {
            Presentation = new NativePresentation
            {
                AccessScope = (uint)q.Presentation.AccessScope,
                CoherentAccess = q.Presentation.CoherentAccess,
                OrderedAccess = q.Presentation.OrderedAccess,
            },
            Partition = new NativePartition { Names = IntPtr.Zero, NamesLen = UIntPtr.Zero },
            GroupData = new NativeBytesData
            {
                Value = scope.Pin(q.GroupData.Value),
                ValueLen = (UIntPtr)q.GroupData.Value.Length,
            },
            EntityFactory = new NativeEntityFactory
            {
                AutoenableCreatedEntities = q.EntityFactory.AutoenableCreatedEntities,
            },
        };
    }

    public static NativePublisherQos ToNative(SubscriberQos q, NativeQosScope scope)
    {
        // Identical layout — Pub/Sub QoS have the same fields per Spec §2.2.3.x.
        return new NativePublisherQos
        {
            Presentation = new NativePresentation
            {
                AccessScope = (uint)q.Presentation.AccessScope,
                CoherentAccess = q.Presentation.CoherentAccess,
                OrderedAccess = q.Presentation.OrderedAccess,
            },
            Partition = new NativePartition { Names = IntPtr.Zero, NamesLen = UIntPtr.Zero },
            GroupData = new NativeBytesData
            {
                Value = scope.Pin(q.GroupData.Value),
                ValueLen = (UIntPtr)q.GroupData.Value.Length,
            },
            EntityFactory = new NativeEntityFactory
            {
                AutoenableCreatedEntities = q.EntityFactory.AutoenableCreatedEntities,
            },
        };
    }

    public static NativeDataWriterQos ToNative(DataWriterQos q, NativeQosScope scope)
    {
        return new NativeDataWriterQos
        {
            Reliability = new NativeReliability
            {
                Kind = (uint)q.Reliability.Kind,
                MaxBlockingTime = N(q.Reliability.MaxBlockingTime),
            },
            Durability = new NativeDurability { Kind = (uint)q.Durability.Kind },
            DurabilityService = new NativeDurabilityService
            {
                ServiceCleanupDelay = N(q.DurabilityService.ServiceCleanupDelay),
                HistoryKind = (uint)q.DurabilityService.HistoryKind,
                HistoryDepth = q.DurabilityService.HistoryDepth,
                MaxSamples = q.DurabilityService.MaxSamples,
                MaxInstances = q.DurabilityService.MaxInstances,
                MaxSamplesPerInstance = q.DurabilityService.MaxSamplesPerInstance,
            },
            Deadline = new NativeDeadline { Period = N(q.Deadline.Period) },
            LatencyBudget = new NativeLatencyBudget { Duration = N(q.LatencyBudget.Duration) },
            Liveliness = new NativeLiveliness
            {
                Kind = (uint)q.Liveliness.Kind,
                LeaseDuration = N(q.Liveliness.LeaseDuration),
            },
            DestinationOrder = new NativeDestinationOrder { Kind = (uint)q.DestinationOrder.Kind },
            Lifespan = new NativeLifespan { Duration = N(q.Lifespan.Duration) },
            Ownership = new NativeOwnership { Kind = (uint)q.Ownership.Kind },
            OwnershipStrength = new NativeOwnershipStrength { Value = q.OwnershipStrength.Value },
            Partition = new NativePartition { Names = IntPtr.Zero, NamesLen = UIntPtr.Zero },
            Presentation = new NativePresentation
            {
                AccessScope = (uint)q.Presentation.AccessScope,
                CoherentAccess = q.Presentation.CoherentAccess,
                OrderedAccess = q.Presentation.OrderedAccess,
            },
            History = new NativeHistory
            {
                Kind = (uint)q.History.Kind,
                Depth = q.History.Depth,
            },
            ResourceLimits = new NativeResourceLimits
            {
                MaxSamples = q.ResourceLimits.MaxSamples,
                MaxInstances = q.ResourceLimits.MaxInstances,
                MaxSamplesPerInstance = q.ResourceLimits.MaxSamplesPerInstance,
            },
            TransportPriority = new NativeTransportPriority { Value = q.TransportPriority.Value },
            WriterDataLifecycle = new NativeWriterDataLifecycle
            {
                AutodisposeUnregisteredInstances = q.WriterDataLifecycle.AutodisposeUnregisteredInstances,
            },
            UserData = new NativeBytesData
            {
                Value = scope.Pin(q.UserData.Value),
                ValueLen = (UIntPtr)q.UserData.Value.Length,
            },
            TopicData = new NativeBytesData
            {
                Value = scope.Pin(q.TopicData.Value),
                ValueLen = (UIntPtr)q.TopicData.Value.Length,
            },
            GroupData = new NativeBytesData
            {
                Value = scope.Pin(q.GroupData.Value),
                ValueLen = (UIntPtr)q.GroupData.Value.Length,
            },
        };
    }

    public static NativeDataReaderQos ToNative(DataReaderQos q, NativeQosScope scope)
    {
        return new NativeDataReaderQos
        {
            Reliability = new NativeReliability
            {
                Kind = (uint)q.Reliability.Kind,
                MaxBlockingTime = N(q.Reliability.MaxBlockingTime),
            },
            Durability = new NativeDurability { Kind = (uint)q.Durability.Kind },
            Deadline = new NativeDeadline { Period = N(q.Deadline.Period) },
            LatencyBudget = new NativeLatencyBudget { Duration = N(q.LatencyBudget.Duration) },
            Liveliness = new NativeLiveliness
            {
                Kind = (uint)q.Liveliness.Kind,
                LeaseDuration = N(q.Liveliness.LeaseDuration),
            },
            DestinationOrder = new NativeDestinationOrder { Kind = (uint)q.DestinationOrder.Kind },
            Ownership = new NativeOwnership { Kind = (uint)q.Ownership.Kind },
            Partition = new NativePartition { Names = IntPtr.Zero, NamesLen = UIntPtr.Zero },
            Presentation = new NativePresentation
            {
                AccessScope = (uint)q.Presentation.AccessScope,
                CoherentAccess = q.Presentation.CoherentAccess,
                OrderedAccess = q.Presentation.OrderedAccess,
            },
            History = new NativeHistory
            {
                Kind = (uint)q.History.Kind,
                Depth = q.History.Depth,
            },
            ResourceLimits = new NativeResourceLimits
            {
                MaxSamples = q.ResourceLimits.MaxSamples,
                MaxInstances = q.ResourceLimits.MaxInstances,
                MaxSamplesPerInstance = q.ResourceLimits.MaxSamplesPerInstance,
            },
            TimeBasedFilter = new NativeTimeBasedFilter
            {
                MinimumSeparation = N(q.TimeBasedFilter.MinimumSeparation),
            },
            ReaderDataLifecycle = new NativeReaderDataLifecycle
            {
                AutopurgeNoWriterSamplesDelay = N(q.ReaderDataLifecycle.AutopurgeNoWriterSamplesDelay),
                AutopurgeDisposedSamplesDelay = N(q.ReaderDataLifecycle.AutopurgeDisposedSamplesDelay),
            },
            UserData = new NativeBytesData
            {
                Value = scope.Pin(q.UserData.Value),
                ValueLen = (UIntPtr)q.UserData.Value.Length,
            },
            TopicData = new NativeBytesData
            {
                Value = scope.Pin(q.TopicData.Value),
                ValueLen = (UIntPtr)q.TopicData.Value.Length,
            },
            GroupData = new NativeBytesData
            {
                Value = scope.Pin(q.GroupData.Value),
                ValueLen = (UIntPtr)q.GroupData.Value.Length,
            },
        };
    }
}
