// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Cft.cs — ContentFilteredTopic (Spec §2.2.2.3.3).
//
// A ContentFilteredTopic is a TopicDescription that restricts the data a
// DataReader receives to the samples whose content satisfies a filter
// expression. The untyped C-FFI evaluates the filter against the on-wire CDR
// payload, so it needs a positional field SCHEMA (name + CDR kind, in wire
// declaration order) — set via Native.CftSetSchema — to resolve the field
// references in the expression (Spec §2.2.2.3.3 + the DDS-SQL filter grammar).

using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using ZeroDDS.Core;
using ZeroDDS.Domain;
using ZeroDDS.Topic;

namespace ZeroDDS.Sub;

/// <summary>CDR field kind for a ContentFilteredTopic schema column
/// (mirrors the C-FFI <c>zerodds_cft_set_schema</c> kind codes).</summary>
public enum CftFieldKind : uint
{
    Bool = 0,
    Int32 = 1,
    Int64 = 2,
    Float32 = 3,
    Float64 = 4,
    String = 5,
}

/// <summary>One positional field of a ContentFilteredTopic schema:
/// the field name referenced by the filter expression and its CDR kind.
/// Fields MUST be listed in on-wire declaration order.</summary>
public readonly record struct CftField(string Name, CftFieldKind Kind);

/// <summary>
/// ContentFilteredTopic&lt;T&gt; (Spec §2.2.2.3.3). Created from a related
/// <see cref="Topic{T}"/> plus a DDS-SQL filter expression. Pass it to the
/// <see cref="DataReader{T}(Subscriber, ContentFilteredTopic{T})"/> ctor to get
/// a reader that only receives matching samples.
/// </summary>
public sealed class ContentFilteredTopic<T> : IDisposable
{
    private IntPtr _handle;
    private readonly IntPtr _participant;
    private readonly ITopicTraits<T> _traits;
    private readonly string _name;
    private readonly string _filterExpression;
    private bool _disposed;

    /// <summary>Native handle.</summary>
    public IntPtr Handle => _handle;

    /// <summary>The filter expression (Spec §2.2.2.3.3).</summary>
    public string FilterExpression => _filterExpression;

    /// <summary>The CFT name.</summary>
    public string Name => _name;

    internal ITopicTraits<T> Traits => _traits;

    /// <summary>
    /// Creates a ContentFilteredTopic over <paramref name="related"/> with the
    /// DDS-SQL <paramref name="filterExpression"/> (e.g. <c>"Seq &gt; %0"</c>),
    /// positional <paramref name="parameters"/> (substituted for <c>%0..%n</c>),
    /// and a positional <paramref name="schema"/> that lets the untyped filter
    /// resolve field references against the CDR payload.
    /// </summary>
    public ContentFilteredTopic(DomainParticipant dp, string name, Topic<T> related,
        string filterExpression, IReadOnlyList<string>? parameters,
        IReadOnlyList<CftField> schema)
    {
        _participant = dp.Handle;
        _traits = related.Traits;
        _name = name;
        _filterExpression = filterExpression;

        IntPtr paramArr = IntPtr.Zero;
        var allocated = new List<IntPtr>();
        try
        {
            UIntPtr paramCount = UIntPtr.Zero;
            if (parameters is { Count: > 0 })
            {
                paramArr = Marshal.AllocHGlobal(IntPtr.Size * parameters.Count);
                allocated.Add(paramArr);
                for (int i = 0; i < parameters.Count; i++)
                {
                    IntPtr s = Marshal.StringToHGlobalAnsi(parameters[i] ?? string.Empty);
                    allocated.Add(s);
                    Marshal.WriteIntPtr(paramArr, i * IntPtr.Size, s);
                }
                paramCount = (UIntPtr)parameters.Count;
            }

            _handle = Native.DpCreateContentFilteredTopic(_participant, name,
                related.Handle, filterExpression, paramArr, paramCount);
            if (_handle == IntPtr.Zero)
                throw new DdsError("ContentFilteredTopic::create failed");
        }
        finally
        {
            foreach (var p in allocated) Marshal.FreeHGlobal(p);
        }

        // Hand the untyped filter the positional CDR schema so it can decode the
        // referenced fields out of the payload (Spec §2.2.2.3.3).
        if (schema is { Count: > 0 })
            SetSchema(schema);
    }

    private void SetSchema(IReadOnlyList<CftField> schema)
    {
        IntPtr namesArr = Marshal.AllocHGlobal(IntPtr.Size * schema.Count);
        var allocated = new List<IntPtr> { namesArr };
        uint[] kinds = new uint[schema.Count];
        try
        {
            for (int i = 0; i < schema.Count; i++)
            {
                IntPtr s = Marshal.StringToHGlobalAnsi(schema[i].Name);
                allocated.Add(s);
                Marshal.WriteIntPtr(namesArr, i * IntPtr.Size, s);
                kinds[i] = (uint)schema[i].Kind;
            }
            unsafe
            {
                fixed (uint* k = kinds)
                {
                    StatusCheck.Check(
                        Native.CftSetSchema(_handle, namesArr, (IntPtr)k, (UIntPtr)schema.Count),
                        "ContentFilteredTopic::SetSchema");
                }
            }
        }
        finally
        {
            foreach (var p in allocated) Marshal.FreeHGlobal(p);
        }
    }

    public void Dispose()
    {
        if (_disposed) return;
        _disposed = true;
        if (_handle != IntPtr.Zero && _participant != IntPtr.Zero)
        {
            Native.DpDeleteContentFilteredTopic(_participant, _handle);
            _handle = IntPtr.Zero;
        }
        GC.SuppressFinalize(this);
    }

    ~ContentFilteredTopic() { Dispose(); }
}
