// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Domain.cs — DDS-PSM-Cxx 1.0 §7.5.11.

using System;
using ZeroDDS.Core;

namespace ZeroDDS.Domain;

/// <summary>DomainParticipantFactory (Spec §7.5.11.4). Singleton.</summary>
public static class DomainParticipantFactory
{
    /// <summary>Returns the singleton factory.</summary>
    public static IntPtr GetInstance() => Native.DpfGetInstance();

    /// <summary>Creates a participant with default QoS.</summary>
    public static DomainParticipant CreateParticipant(uint domainId)
    {
        var f = Native.DpfGetInstance();
        var h = Native.DpfCreateParticipant(f, domainId, IntPtr.Zero);
        if (h == IntPtr.Zero) throw new DdsError("DomainParticipantFactory::CreateParticipant");
        return new DomainParticipant(h, f);
    }

    /// <summary>Creates a participant with explicit QoS (Spec §2.2.2.2.2.2).</summary>
    public static DomainParticipant CreateParticipant(uint domainId,
        ZeroDDS.Qos.DomainParticipantQos qos)
    {
        var f = Native.DpfGetInstance();
        using var scope = new ZeroDDS.QosBridge.NativeQosScope();
        var native = ZeroDDS.QosBridge.QosBridge.ToNative(qos, scope);
        IntPtr h;
        unsafe { h = Native.DpfCreateParticipant(f, domainId, (IntPtr)(&native)); }
        if (h == IntPtr.Zero) throw new DdsError("DomainParticipantFactory::CreateParticipant with QoS");
        return new DomainParticipant(h, f);
    }
}

/// <summary>DomainParticipant (Spec §7.5.11.5).</summary>
public sealed class DomainParticipant : IDisposable
{
    private IntPtr _handle;
    private readonly IntPtr _factory;
    private bool _disposed;

    internal DomainParticipant(IntPtr handle, IntPtr factory)
    {
        _handle = handle;
        _factory = factory;
    }

    /// <summary>Native handle (internal, for Topic/Pub/Sub construction).</summary>
    public IntPtr Handle => _handle;

    /// <summary>Domain id (Spec §7.5.11.5).</summary>
    public uint DomainId
    {
        get
        {
            EnsureAlive();
            return Native.DpGetDomainId(_handle);
        }
    }

    /// <summary>assert_liveliness.</summary>
    public void AssertLiveliness()
    {
        EnsureAlive();
        StatusCheck.Check(Native.DpAssertLiveliness(_handle), "DomainParticipant::AssertLiveliness");
    }

    /// <summary>contains_entity.</summary>
    public bool ContainsEntity(InstanceHandle h)
    {
        EnsureAlive();
        return Native.DpContainsEntity(_handle, h.Value) != 0;
    }

    /// <summary>ignore_participant.</summary>
    public void IgnoreParticipant(InstanceHandle h)
    {
        EnsureAlive();
        StatusCheck.Check(Native.DpIgnoreParticipant(_handle, h.Value),
            "DomainParticipant::IgnoreParticipant");
    }

    /// <summary>delete_contained_entities.</summary>
    public void DeleteContainedEntities()
    {
        EnsureAlive();
        StatusCheck.Check(Native.DpDeleteContainedEntities(_handle),
            "DomainParticipant::DeleteContainedEntities");
    }

    /// <summary>Disposable path: clean up.</summary>
    public void Dispose()
    {
        if (_disposed) return;
        _disposed = true;
        if (_handle != IntPtr.Zero)
        {
            Native.DpDeleteContainedEntities(_handle);
            Native.DpfDeleteParticipant(_factory, _handle);
            _handle = IntPtr.Zero;
        }
        GC.SuppressFinalize(this);
    }

    ~DomainParticipant() { Dispose(); }

    internal void EnsureAlive()
    {
        if (_disposed || _handle == IntPtr.Zero)
            throw new AlreadyClosedException("DomainParticipant disposed");
    }
}
