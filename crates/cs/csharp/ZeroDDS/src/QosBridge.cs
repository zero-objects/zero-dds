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
    private readonly System.Collections.Generic.List<IntPtr> _unmanaged = new();
    private bool _disposed;

    internal IntPtr Pin(byte[] data)
    {
        if (data == null || data.Length == 0) return IntPtr.Zero;
        var h = GCHandle.Alloc(data, GCHandleType.Pinned);
        _handles.Add(h);
        return h.AddrOfPinnedObject();
    }

    /// <summary>
    /// Marshals a PartitionPolicy's name list into a C-FFI
    /// <c>const char *const *</c> array (Spec §2.2.3.x PARTITION). Returns
    /// (names_ptr, count); names_ptr is <see cref="IntPtr.Zero"/> for an empty
    /// list. The native ANSI strings + the pointer array are freed on Dispose.
    /// </summary>
    internal (IntPtr namesPtr, UIntPtr count) PinPartition(
        System.Collections.Generic.IReadOnlyList<string>? names)
    {
        if (names == null || names.Count == 0) return (IntPtr.Zero, UIntPtr.Zero);
        // Array of char* pointers, one per name.
        IntPtr arr = Marshal.AllocHGlobal(IntPtr.Size * names.Count);
        _unmanaged.Add(arr);
        for (int i = 0; i < names.Count; i++)
        {
            IntPtr s = Marshal.StringToHGlobalAnsi(names[i] ?? string.Empty);
            _unmanaged.Add(s);
            Marshal.WriteIntPtr(arr, i * IntPtr.Size, s);
        }
        return (arr, (UIntPtr)names.Count);
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
        foreach (var p in _unmanaged)
        {
            if (p != IntPtr.Zero) Marshal.FreeHGlobal(p);
        }
        _unmanaged.Clear();
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

    // DDS-Security 1.2 section 7.3.21: ZeroDdsPropertyQosPolicy and
    // ZeroDdsDataTagQosPolicy, the trailing members of the DataWriter and
    // DataReader QoS structs. All-zero is no entries.
    [StructLayout(LayoutKind.Sequential)]
    internal struct NativePropertyPolicy
    {
        public IntPtr Entries;
        public UIntPtr EntriesLen;
        public IntPtr BinaryEntries;
        public UIntPtr BinaryEntriesLen;
    }
    [StructLayout(LayoutKind.Sequential)]
    internal struct NativeDataTagPolicy
    {
        public IntPtr Tags;
        public UIntPtr TagsLen;
    }

    // DDS-XTypes 1.3 Annex D: ZeroDdsDataRepresentationQosPolicy (`is_set`,
    // `count`, `ids[8]`) and ZeroDdsTypeConsistencyEnforcementQosPolicy, the
    // members that follow GroupData (TopicData for the Topic) in the Topic,
    // DataWriter and DataReader QoS structs, before Property and DataTags.
    // All-zero leaves the policies at their defaults. The eight ids are single
    // fields so the struct stays blittable without unsafe code.
    [StructLayout(LayoutKind.Sequential)]
    internal struct NativeDataRepresentationPolicy
    {
        public byte IsSet;
        public byte Count;
        public short Id0, Id1, Id2, Id3, Id4, Id5, Id6, Id7;
    }
    [StructLayout(LayoutKind.Sequential)]
    internal struct NativeTypeConsistencyPolicy
    {
        public byte IsSet;
        public byte Kind;
        public byte IgnoreSequenceBounds;
        public byte IgnoreStringBounds;
        public byte IgnoreMemberNames;
        public byte PreventTypeWidening;
        public byte ForceTypeValidation;
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
        internal NativeDataRepresentationPolicy DataRepresentation;
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
        internal NativeDataRepresentationPolicy DataRepresentation;
        internal NativePropertyPolicy Property;
        internal NativeDataTagPolicy DataTags;
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
        internal NativeDataRepresentationPolicy DataRepresentation;
        internal NativeTypeConsistencyPolicy TypeConsistency;
        internal NativePropertyPolicy Property;
        internal NativeDataTagPolicy DataTags;
    }

    private static NativeDuration N(Duration d) =>
        new() { Sec = d.Sec, Nanosec = d.Nanosec };

    private static NativePartition P(PartitionPolicy p, NativeQosScope scope)
    {
        var (names, count) = scope.PinPartition(p?.Names);
        return new NativePartition { Names = names, NamesLen = count };
    }

    /// The octets of UserData, TopicData or GroupData, pinned for the call.
    private static NativeBytesData Bytes(byte[] value, NativeQosScope scope) =>
        new() { Value = scope.Pin(value), ValueLen = (UIntPtr)value.Length };

    private static NativeReliability R(ReliabilityPolicy p) =>
        new() { Kind = (uint)p.Kind, MaxBlockingTime = N(p.MaxBlockingTime) };

    private static NativeDurabilityService DS(DurabilityServicePolicy p) => new()
    {
        ServiceCleanupDelay = N(p.ServiceCleanupDelay),
        HistoryKind = (uint)p.HistoryKind,
        HistoryDepth = p.HistoryDepth,
        MaxSamples = p.MaxSamples,
        MaxInstances = p.MaxInstances,
        MaxSamplesPerInstance = p.MaxSamplesPerInstance,
    };

    private static NativeLiveliness L(LivelinessPolicy p) =>
        new() { Kind = (uint)p.Kind, LeaseDuration = N(p.LeaseDuration) };

    private static NativePresentation Pres(PresentationPolicy p) => new()
    {
        AccessScope = (uint)p.AccessScope,
        CoherentAccess = p.CoherentAccess,
        OrderedAccess = p.OrderedAccess,
    };

    private static NativeHistory H(HistoryPolicy p) => new() { Kind = (uint)p.Kind, Depth = p.Depth };

    private static NativeResourceLimits RL(ResourceLimitsPolicy p) => new()
    {
        MaxSamples = p.MaxSamples,
        MaxInstances = p.MaxInstances,
        MaxSamplesPerInstance = p.MaxSamplesPerInstance,
    };

    private static NativeEntityFactory EF(EntityFactoryPolicy p) =>
        new() { AutoenableCreatedEntities = p.AutoenableCreatedEntities };

    public static NativeDomainParticipantQos ToNative(DomainParticipantQos q, NativeQosScope scope)
    {
        return new NativeDomainParticipantQos
        {
            UserData = Bytes(q.UserData.Value, scope),
            EntityFactory = EF(q.EntityFactory),
        };
    }

    public static NativeTopicQos ToNative(TopicQos q, NativeQosScope scope)
    {
        return new NativeTopicQos
        {
            Durability = new NativeDurability { Kind = (uint)q.Durability.Kind },
            DurabilityService = DS(q.DurabilityService),
            Deadline = new NativeDeadline { Period = N(q.Deadline.Period) },
            LatencyBudget = new NativeLatencyBudget { Duration = N(q.LatencyBudget.Duration) },
            Liveliness = L(q.Liveliness),
            Reliability = R(q.Reliability),
            DestinationOrder = new NativeDestinationOrder { Kind = (uint)q.DestinationOrder.Kind },
            History = H(q.History),
            ResourceLimits = RL(q.ResourceLimits),
            TransportPriority = new NativeTransportPriority { Value = q.TransportPriority.Value },
            Lifespan = new NativeLifespan { Duration = N(q.Lifespan.Duration) },
            Ownership = new NativeOwnership { Kind = (uint)q.Ownership.Kind },
            TopicData = Bytes(q.TopicData.Value, scope),
        };
    }

    public static NativePublisherQos ToNative(PublisherQos q, NativeQosScope scope)
    {
        return new NativePublisherQos
        {
            Presentation = Pres(q.Presentation),
            Partition = P(q.Partition, scope),
            GroupData = Bytes(q.GroupData.Value, scope),
            EntityFactory = EF(q.EntityFactory),
        };
    }

    public static NativePublisherQos ToNative(SubscriberQos q, NativeQosScope scope)
    {
        // Identical layout — Pub/Sub QoS have the same fields per Spec §2.2.3.x.
        return new NativePublisherQos
        {
            Presentation = Pres(q.Presentation),
            Partition = P(q.Partition, scope),
            GroupData = Bytes(q.GroupData.Value, scope),
            EntityFactory = EF(q.EntityFactory),
        };
    }

    public static NativeDataWriterQos ToNative(DataWriterQos q, NativeQosScope scope)
    {
        return new NativeDataWriterQos
        {
            Reliability = R(q.Reliability),
            Durability = new NativeDurability { Kind = (uint)q.Durability.Kind },
            DurabilityService = DS(q.DurabilityService),
            Deadline = new NativeDeadline { Period = N(q.Deadline.Period) },
            LatencyBudget = new NativeLatencyBudget { Duration = N(q.LatencyBudget.Duration) },
            Liveliness = L(q.Liveliness),
            DestinationOrder = new NativeDestinationOrder { Kind = (uint)q.DestinationOrder.Kind },
            Lifespan = new NativeLifespan { Duration = N(q.Lifespan.Duration) },
            Ownership = new NativeOwnership { Kind = (uint)q.Ownership.Kind },
            OwnershipStrength = new NativeOwnershipStrength { Value = q.OwnershipStrength.Value },
            Partition = P(q.Partition, scope),
            Presentation = Pres(q.Presentation),
            History = H(q.History),
            ResourceLimits = RL(q.ResourceLimits),
            TransportPriority = new NativeTransportPriority { Value = q.TransportPriority.Value },
            WriterDataLifecycle = new NativeWriterDataLifecycle
            {
                AutodisposeUnregisteredInstances = q.WriterDataLifecycle.AutodisposeUnregisteredInstances,
            },
            UserData = Bytes(q.UserData.Value, scope),
            TopicData = Bytes(q.TopicData.Value, scope),
            GroupData = Bytes(q.GroupData.Value, scope),
        };
    }

    public static NativeDataReaderQos ToNative(DataReaderQos q, NativeQosScope scope)
    {
        return new NativeDataReaderQos
        {
            Reliability = R(q.Reliability),
            Durability = new NativeDurability { Kind = (uint)q.Durability.Kind },
            Deadline = new NativeDeadline { Period = N(q.Deadline.Period) },
            LatencyBudget = new NativeLatencyBudget { Duration = N(q.LatencyBudget.Duration) },
            Liveliness = L(q.Liveliness),
            DestinationOrder = new NativeDestinationOrder { Kind = (uint)q.DestinationOrder.Kind },
            Ownership = new NativeOwnership { Kind = (uint)q.Ownership.Kind },
            Partition = P(q.Partition, scope),
            Presentation = Pres(q.Presentation),
            History = H(q.History),
            ResourceLimits = RL(q.ResourceLimits),
            TimeBasedFilter = new NativeTimeBasedFilter
            {
                MinimumSeparation = N(q.TimeBasedFilter.MinimumSeparation),
            },
            ReaderDataLifecycle = new NativeReaderDataLifecycle
            {
                AutopurgeNoWriterSamplesDelay = N(q.ReaderDataLifecycle.AutopurgeNoWriterSamplesDelay),
                AutopurgeDisposedSamplesDelay = N(q.ReaderDataLifecycle.AutopurgeDisposedSamplesDelay),
            },
            UserData = Bytes(q.UserData.Value, scope),
            TopicData = Bytes(q.TopicData.Value, scope),
            GroupData = Bytes(q.GroupData.Value, scope),
        };
    }
}
