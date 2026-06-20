// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Fluent.cs — Idiomatic .NET convenience layer over the constructor-based
// DDS-PSM-Cxx API. There is no OMG C# PSM, so this is the ZeroDDS-native
// "5-minute onramp" surface: a fluent factory, byte-topic shortcuts,
// TimeSpan-based wait helpers, and an event-driven WaitForData.
//
// These helpers are thin wrappers; the spec-faithful constructor API
// (`new Publisher(dp)`, `new DataWriter<T>(pub, topic)`, …) remains the
// canonical surface and is unchanged.

using System;
using System.Collections.Generic;
using ZeroDDS.Cond;
using ZeroDDS.Core;
using ZeroDDS.Domain;
using ZeroDDS.Pub;
using ZeroDDS.Sub;
using ZeroDDS.SubCond;
using ZeroDDS.Topic;

namespace ZeroDDS;

/// <summary>
/// Singleton accessor for the DDS factory, mirroring the OMG
/// `DomainParticipantFactory.get_instance()` shape as an idiomatic
/// .NET <c>Instance</c> property. Returned object exposes instance-style
/// <see cref="CreateParticipant(uint)"/>.
/// </summary>
public sealed class DomainParticipantFactory
{
    private static readonly DomainParticipantFactory _instance = new();

    private DomainParticipantFactory() { }

    /// <summary>The process-wide factory singleton.</summary>
    public static DomainParticipantFactory Instance => _instance;

    /// <summary>Creates a participant with default QoS on <paramref name="domainId"/>.</summary>
    public DomainParticipant CreateParticipant(uint domainId) =>
        ZeroDDS.Domain.DomainParticipantFactory.CreateParticipant(domainId);

    /// <summary>Creates a participant with explicit QoS.</summary>
    public DomainParticipant CreateParticipant(uint domainId,
        ZeroDDS.Qos.DomainParticipantQos qos) =>
        ZeroDDS.Domain.DomainParticipantFactory.CreateParticipant(domainId, qos);
}

/// <summary>
/// Fluent shortcuts on <see cref="DomainParticipant"/>, <see cref="Publisher"/>,
/// <see cref="Subscriber"/> and the byte/typed entities. All methods compose
/// the existing constructor-based entities; no behaviour beyond convenience.
/// </summary>
public static class FluentExtensions
{
    // ---- DomainParticipant ----

    /// <summary>Creates a raw-bytes <see cref="Topic{T}"/> (pre-wired <see cref="ByteSeqTraits"/>).</summary>
    public static Topic<byte[]> CreateBytesTopic(this DomainParticipant dp, string name) =>
        new Topic<byte[]>(dp, name, new ByteSeqTraits());

    /// <summary>Creates a default <see cref="Publisher"/>.</summary>
    public static Publisher CreatePublisher(this DomainParticipant dp) => new Publisher(dp);

    /// <summary>Creates a default <see cref="Subscriber"/>.</summary>
    public static Subscriber CreateSubscriber(this DomainParticipant dp) => new Subscriber(dp);

    // ---- Publisher ----

    /// <summary>Creates a <see cref="BytesWriter"/> on a byte topic.</summary>
    public static BytesWriter CreateBytesWriter(this Publisher pub, Topic<byte[]> topic) =>
        new BytesWriter(new DataWriter<byte[]>(pub, topic));

    // ---- Subscriber ----

    /// <summary>Creates a <see cref="BytesReader"/> on a byte topic.</summary>
    public static BytesReader CreateBytesReader(this Subscriber sub, Topic<byte[]> topic) =>
        new BytesReader(new DataReader<byte[]>(sub, topic));
}

/// <summary>
/// Convenience writer for raw <see cref="byte"/> payloads. Wraps a
/// <see cref="DataWriter{T}"/> of <c>byte[]</c> and adds TimeSpan-based
/// match waiting so the quickstart needs no <see cref="Duration"/> import.
/// </summary>
public sealed class BytesWriter : IDisposable
{
    private readonly DataWriter<byte[]> _inner;

    internal BytesWriter(DataWriter<byte[]> inner) => _inner = inner;

    /// <summary>Underlying spec-faithful writer.</summary>
    public DataWriter<byte[]> Writer => _inner;

    /// <summary>Writes a raw payload.</summary>
    public void Write(byte[] payload) => _inner.Write(payload);

    /// <summary>Blocks until at least <paramref name="count"/> subscriptions match or timeout.</summary>
    public void WaitForMatchedSubscription(int count, TimeSpan timeout) =>
        _inner.WaitForMatched(count, timeout.ToDuration());

    /// <summary>Blocks until all written samples are acknowledged or timeout.</summary>
    public void WaitForAcks(TimeSpan timeout) => _inner.WaitForAcks(timeout.ToDuration());

    public void Dispose() => _inner.Dispose();
}

/// <summary>
/// Convenience reader for raw <see cref="byte"/> payloads. Adds a
/// TimeSpan-based match wait, an event-driven <see cref="WaitForData"/>,
/// and a <see cref="Take"/> overload yielding raw <c>byte[]</c> payloads.
/// </summary>
public sealed class BytesReader : IDisposable
{
    private readonly DataReader<byte[]> _inner;

    internal BytesReader(DataReader<byte[]> inner) => _inner = inner;

    /// <summary>Underlying spec-faithful reader.</summary>
    public DataReader<byte[]> Reader => _inner;

    /// <summary>Blocks until at least <paramref name="count"/> publications match or timeout.</summary>
    public void WaitForMatchedPublication(int count, TimeSpan timeout) =>
        _inner.WaitForMatched(count, timeout.ToDuration());

    /// <summary>
    /// Blocks until unread data is available or <paramref name="timeout"/> elapses.
    /// Event-driven via a not-read <see cref="ReadCondition{T}"/> on a
    /// <see cref="WaitSet"/> — no busy poll. Returns true if data arrived,
    /// false on timeout.
    /// </summary>
    public bool WaitForData(TimeSpan timeout) => _inner.WaitForData(timeout);

    /// <summary>Takes all available samples and yields their raw payloads.</summary>
    public IEnumerable<byte[]> Take(int maxSamples = 0)
    {
        var samples = _inner.Take(maxSamples);
        var result = new List<byte[]>(samples.Count);
        foreach (var s in samples)
        {
            if (s.Info.ValidData && s.Data is not null)
                result.Add(s.Data);
        }
        return result;
    }

    public void Dispose() => _inner.Dispose();
}

/// <summary>TimeSpan ↔ Duration bridging for the fluent surface.</summary>
public static class TimeSpanBridge
{
    /// <summary>Converts a BCL <see cref="TimeSpan"/> to a DDS <see cref="Duration"/>.</summary>
    public static Duration ToDuration(this TimeSpan ts)
    {
        if (ts == System.Threading.Timeout.InfiniteTimeSpan || ts.TotalMilliseconds < 0)
            return Duration.Infinite;
        long ms = (long)ts.TotalMilliseconds;
        return Duration.FromMillis(ms);
    }
}
