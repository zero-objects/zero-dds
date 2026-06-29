// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Typed.cs — Fluent typed-topic surface for IDL-generated types.
//
// `idl-csharp` emits, per IDL struct, a `<Name>TypeSupport : IDdsTopicType<T>`
// (ZeroDDS.Cdr) with a `public static readonly <Name>TypeSupport Instance`.
// The DDS entity layer, however, consumes `ITopicTraits<T>` (ZeroDDS.Topic).
// This file bridges the two interfaces and lets callers create a typed topic
// / writer / reader with **no explicit traits argument** — the TypeSupport is
// auto-resolved either via an explicit registration or the
// `<Name>TypeSupport.Instance` reflection convention.

using System;
using System.Collections.Concurrent;
using System.Reflection;
using ZeroDDS.Cdr;
using ZeroDDS.Domain;
using ZeroDDS.Pub;
using ZeroDDS.Sub;
using ZeroDDS.Topic;

namespace ZeroDDS;

/// <summary>
/// Non-generic capability marker so the DDS entity layer (whose <c>T</c> is not
/// constrained to <c>notnull</c>) can obtain a keyed-instance key hash from a
/// generated TypeSupport without a generic-constraint mismatch.
/// </summary>
public interface IKeyHashProvider
{
    /// <summary>16-byte XTypes key hash (§7.6.8) of <paramref name="sample"/>.</summary>
    byte[] KeyHashOf(object sample);
}

/// <summary>
/// Adapter exposing an <see cref="IDdsTopicType{T}"/> (codegen output) as the
/// <see cref="ITopicTraits{T}"/> the DDS entity layer consumes.
/// </summary>
public sealed class DdsTopicTypeTraits<T> : ITopicTraits<T>, IKeyHashProvider where T : notnull
{
    private readonly IDdsTopicType<T> _ts;

    /// <summary>Wraps a generated TypeSupport.</summary>
    public DdsTopicTypeTraits(IDdsTopicType<T> typeSupport) =>
        _ts = typeSupport ?? throw new ArgumentNullException(nameof(typeSupport));

    /// <summary>The wrapped TypeSupport.</summary>
    public IDdsTopicType<T> TypeSupport => _ts;

    /// <inheritdoc/>
    public string TypeName => _ts.TypeName;

    /// <inheritdoc/>
    public byte[] Encode(T value) => _ts.Encode(value);

    /// <inheritdoc/>
    public T Decode(ReadOnlySpan<byte> bytes) => _ts.Decode(bytes);

    /// <inheritdoc/>
    public T Decode(ReadOnlySpan<byte> bytes, EndianMode endian) => _ts.Decode(bytes, endian);

    /// <inheritdoc/>
    public T Decode(ReadOnlySpan<byte> bytes, EndianMode endian, int representation) =>
        _ts.Decode(bytes, endian, representation);

    /// <summary>
    /// 16-byte XTypes key hash (§7.6.8) of the instance carried by
    /// <paramref name="value"/>. Used to drive keyed-instance lifecycle
    /// (register/dispose/unregister) over the C-FFI.
    /// </summary>
    public byte[] KeyHash(T value) => _ts.KeyHash(value);

    /// <inheritdoc/>
    byte[] IKeyHashProvider.KeyHashOf(object sample) => _ts.KeyHash((T)sample);
}

/// <summary>
/// Process-wide registry mapping a sample type <c>T</c> to its
/// <see cref="IDdsTopicType{T}"/>. Generated code may register explicitly via
/// <see cref="Register{T}"/>; otherwise <see cref="Resolve{T}"/> falls back to
/// the <c>&lt;Name&gt;TypeSupport.Instance</c> reflection convention.
/// </summary>
public static class TypeSupportRegistry
{
    private static readonly ConcurrentDictionary<Type, object> _map = new();

    /// <summary>Registers a TypeSupport instance for <typeparamref name="T"/>.</summary>
    public static void Register<T>(IDdsTopicType<T> typeSupport) where T : notnull =>
        _map[typeof(T)] = typeSupport ?? throw new ArgumentNullException(nameof(typeSupport));

    /// <summary>
    /// Resolves the TypeSupport for <typeparamref name="T"/>. Looks up an
    /// explicit registration first, then probes the convention
    /// <c>{T.FullName}TypeSupport.Instance</c> / any
    /// <see cref="IDdsTopicType{T}"/>-implementing type in T's assembly with a
    /// static <c>Instance</c> member.
    /// </summary>
    /// <exception cref="InvalidOperationException">If no TypeSupport can be found.</exception>
    public static IDdsTopicType<T> Resolve<T>() where T : notnull
    {
        if (_map.TryGetValue(typeof(T), out var existing))
            return (IDdsTopicType<T>)existing;

        var ts = Discover<T>();
        if (ts is null)
        {
            throw new InvalidOperationException(
                $"No IDdsTopicType<{typeof(T).Name}> found. Register one via " +
                $"TypeSupportRegistry.Register, or ensure a '{typeof(T).Name}TypeSupport' " +
                "class with a static 'Instance' member is loaded.");
        }
        _map[typeof(T)] = ts;
        return ts;
    }

    private static IDdsTopicType<T>? Discover<T>() where T : notnull
    {
        var t = typeof(T);

        // 1) Convention: a sibling type named "<T>TypeSupport".
        var byName = t.Assembly.GetType(t.FullName + "TypeSupport");
        var fromName = InstanceOf<T>(byName);
        if (fromName is not null) return fromName;

        // 2) Any type in T's assembly that implements IDdsTopicType<T>.
        foreach (var candidate in t.Assembly.GetTypes())
        {
            if (candidate == t) continue;
            if (typeof(IDdsTopicType<T>).IsAssignableFrom(candidate))
            {
                var inst = InstanceOf<T>(candidate);
                if (inst is not null) return inst;
            }
        }
        return null;
    }

    private static IDdsTopicType<T>? InstanceOf<T>(Type? candidate) where T : notnull
    {
        if (candidate is null) return null;
        if (!typeof(IDdsTopicType<T>).IsAssignableFrom(candidate)) return null;

        // Prefer a static `Instance` field/property (the codegen convention).
        var field = candidate.GetField("Instance",
            BindingFlags.Public | BindingFlags.Static | BindingFlags.FlattenHierarchy);
        if (field?.GetValue(null) is IDdsTopicType<T> fv) return fv;

        var prop = candidate.GetProperty("Instance",
            BindingFlags.Public | BindingFlags.Static | BindingFlags.FlattenHierarchy);
        if (prop?.GetValue(null) is IDdsTopicType<T> pv) return pv;

        // Fall back to a public parameterless ctor.
        if (candidate.GetConstructor(Type.EmptyTypes) is not null &&
            Activator.CreateInstance(candidate) is IDdsTopicType<T> created)
        {
            return created;
        }
        return null;
    }
}

/// <summary>Fluent typed-topic shortcuts auto-resolving the type's TypeSupport.</summary>
public static class TypedFluentExtensions
{
    // ---- DomainParticipant ----

    /// <summary>
    /// Creates a strongly-typed <see cref="Topic{T}"/>, auto-resolving
    /// <typeparamref name="T"/>'s TypeSupport (no traits argument needed).
    /// </summary>
    public static Topic<T> CreateTypedTopic<T>(this DomainParticipant dp, string name)
        where T : notnull
    {
        var traits = new DdsTopicTypeTraits<T>(TypeSupportRegistry.Resolve<T>());
        return new Topic<T>(dp, name, traits);
    }

    /// <summary>
    /// Creates a strongly-typed <see cref="Topic{T}"/> from an explicit
    /// <see cref="IDdsTopicType{T}"/> TypeSupport.
    /// </summary>
    public static Topic<T> CreateTypedTopic<T>(this DomainParticipant dp, string name,
        IDdsTopicType<T> typeSupport) where T : notnull =>
        new Topic<T>(dp, name, new DdsTopicTypeTraits<T>(typeSupport));

    // ---- Publisher ----

    /// <summary>Creates a strongly-typed <see cref="DataWriter{T}"/> on a typed topic.</summary>
    public static DataWriter<T> CreateTypedWriter<T>(this Publisher pub, Topic<T> topic)
        where T : notnull =>
        new DataWriter<T>(pub, topic);

    // ---- Subscriber ----

    /// <summary>Creates a strongly-typed <see cref="DataReader{T}"/> on a typed topic.</summary>
    public static DataReader<T> CreateTypedReader<T>(this Subscriber sub, Topic<T> topic)
        where T : notnull =>
        new DataReader<T>(sub, topic);
}
