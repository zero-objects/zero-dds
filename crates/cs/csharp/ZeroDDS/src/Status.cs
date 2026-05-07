// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Status.cs — Public Status-Strukturen (DDS-PSM-Cxx 1.0 §7.5.7).

using ZeroDDS.Core;

namespace ZeroDDS.Status;

/// <summary>StatusKind-Bits (Spec §7.5.7).</summary>
[System.Flags]
public enum StatusKind : uint
{
    None = 0,
    InconsistentTopic = 1u << 0,
    OfferedDeadlineMissed = 1u << 1,
    RequestedDeadlineMissed = 1u << 2,
    OfferedIncompatibleQos = 1u << 5,
    RequestedIncompatibleQos = 1u << 6,
    SampleLost = 1u << 7,
    SampleRejected = 1u << 8,
    DataOnReaders = 1u << 9,
    DataAvailable = 1u << 10,
    LivelinessLost = 1u << 11,
    LivelinessChanged = 1u << 12,
    PublicationMatched = 1u << 13,
    SubscriptionMatched = 1u << 14,
}

public readonly record struct PublicationMatchedStatus(
    int TotalCount,
    int TotalCountChange,
    int CurrentCount,
    int CurrentCountChange,
    InstanceHandle LastSubscriptionHandle);

public readonly record struct SubscriptionMatchedStatus(
    int TotalCount,
    int TotalCountChange,
    int CurrentCount,
    int CurrentCountChange,
    InstanceHandle LastPublicationHandle);

public readonly record struct SampleLostStatus(int TotalCount, int TotalCountChange);

public readonly record struct LivelinessLostStatus(int TotalCount, int TotalCountChange);

public readonly record struct OfferedDeadlineMissedStatus(
    int TotalCount, int TotalCountChange, InstanceHandle LastInstanceHandle);

public readonly record struct RequestedDeadlineMissedStatus(
    int TotalCount, int TotalCountChange, InstanceHandle LastInstanceHandle);

public readonly record struct OfferedIncompatibleQosStatus(
    int TotalCount, int TotalCountChange, uint LastPolicyId);

public readonly record struct RequestedIncompatibleQosStatus(
    int TotalCount, int TotalCountChange, uint LastPolicyId);

public readonly record struct LivelinessChangedStatus(
    int AliveCount, int NotAliveCount, int AliveCountChange, int NotAliveCountChange,
    InstanceHandle LastPublicationHandle);

public readonly record struct InconsistentTopicStatus(int TotalCount, int TotalCountChange);

public readonly record struct SampleRejectedStatus(
    int TotalCount, int TotalCountChange, uint LastReason,
    InstanceHandle LastInstanceHandle);
