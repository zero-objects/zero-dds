// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Topic.cs — DDS-PSM-Cxx 1.0 §7.5.13.

using System;
using System.Runtime.InteropServices;
using ZeroDDS.Core;
using ZeroDDS.Domain;

namespace ZeroDDS.Topic;

/// <summary>
/// Trait der pro Sample-Type T spezialisiert wird.
/// Generierte IDL-Bindings emittieren eine konkrete Klasse die dieses
/// Interface implementiert; Anwendungen ohne IDL benutzen
/// <see cref="ByteSeqTraits"/> fuer rohe Bytes.
/// </summary>
public interface ITopicTraits<T>
{
    string TypeName { get; }
    byte[] Encode(T value);
    T Decode(ReadOnlySpan<byte> bytes);
}

/// <summary>Default-Traits fuer raw bytes.</summary>
public sealed class ByteSeqTraits : ITopicTraits<byte[]>
{
    public string TypeName => "DDS::Bytes";
    public byte[] Encode(byte[] v) => v;
    public byte[] Decode(ReadOnlySpan<byte> bytes) => bytes.ToArray();
}

/// <summary>Default-Traits fuer UTF-8 strings.</summary>
public sealed class StringTraits : ITopicTraits<string>
{
    public string TypeName => "DDS::String";
    public byte[] Encode(string v) => System.Text.Encoding.UTF8.GetBytes(v);
    public string Decode(ReadOnlySpan<byte> bytes)
    {
        return System.Text.Encoding.UTF8.GetString(bytes);
    }
}

/// <summary>TopicDescription (Spec §7.5.13.1).</summary>
public class TopicDescription
{
    protected internal IntPtr Handle;

    internal TopicDescription(IntPtr handle) { Handle = handle; }

    /// <summary>Topic-Name (Spec §7.5.13.1).</summary>
    public string Name
    {
        get
        {
            if (Handle == IntPtr.Zero) return "";
            var raw = Native.TopicGetName(Handle);
            if (raw == IntPtr.Zero) return "";
            try
            {
                return Marshal.PtrToStringAnsi(raw) ?? "";
            }
            finally
            {
                Native.StringFree(raw);
            }
        }
    }

    /// <summary>Type-Name.</summary>
    public string TypeName
    {
        get
        {
            if (Handle == IntPtr.Zero) return "";
            var raw = Native.TopicGetTypeName(Handle);
            if (raw == IntPtr.Zero) return "";
            try
            {
                return Marshal.PtrToStringAnsi(raw) ?? "";
            }
            finally
            {
                Native.StringFree(raw);
            }
        }
    }
}

/// <summary>Topic&lt;T&gt; (Spec §7.5.13.5).</summary>
public sealed class Topic<T> : TopicDescription, IDisposable
{
    private readonly IntPtr _participant;
    private readonly ITopicTraits<T> _traits;
    private bool _disposed;

    /// <summary>Konstruiert via Participant + Name + Traits (Default-QoS).</summary>
    public Topic(DomainParticipant dp, string name, ITopicTraits<T> traits)
        : base(Native.DpCreateTopic(dp.Handle, name, traits.TypeName, IntPtr.Zero))
    {
        if (Handle == IntPtr.Zero)
            throw new DdsError("Topic::create failed");
        _participant = dp.Handle;
        _traits = traits;
    }

    /// <summary>Konstruiert mit expliziter QoS (Spec §2.2.2.2.1.5).</summary>
    public Topic(DomainParticipant dp, string name, ITopicTraits<T> traits,
        ZeroDDS.Qos.TopicQos qos)
        : base(IntPtr.Zero)
    {
        using var scope = new ZeroDDS.QosBridge.NativeQosScope();
        var native = ZeroDDS.QosBridge.QosBridge.ToNative(qos, scope);
        unsafe
        {
            Handle = Native.DpCreateTopic(dp.Handle, name, traits.TypeName, (IntPtr)(&native));
        }
        if (Handle == IntPtr.Zero)
            throw new DdsError("Topic::create with QoS failed");
        _participant = dp.Handle;
        _traits = traits;
    }

    /// <summary>Type-Support trait.</summary>
    public ITopicTraits<T> Traits => _traits;

    public void Dispose()
    {
        if (_disposed) return;
        _disposed = true;
        if (Handle != IntPtr.Zero && _participant != IntPtr.Zero)
        {
            Native.DpDeleteTopic(_participant, Handle);
            Handle = IntPtr.Zero;
        }
        GC.SuppressFinalize(this);
    }

    ~Topic() { Dispose(); }
}
