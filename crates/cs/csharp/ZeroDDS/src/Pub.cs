// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Pub.cs — DDS-PSM-Cxx 1.0 §7.5.14.

using System;
using System.Runtime.InteropServices;
using ZeroDDS.Core;
using ZeroDDS.Domain;
using ZeroDDS.Topic;

namespace ZeroDDS.Pub;

/// <summary>Publisher (Spec §7.5.14.1).</summary>
public sealed class Publisher : IDisposable
{
    private IntPtr _handle;
    private readonly IntPtr _participant;
    private bool _disposed;

    /// <summary>Konstruiert via Participant.</summary>
    public Publisher(DomainParticipant dp)
    {
        _participant = dp.Handle;
        _handle = Native.DpCreatePublisher(_participant, IntPtr.Zero);
        if (_handle == IntPtr.Zero) throw new DdsError("Publisher::create failed");
    }

    /// <summary>Konstruiert mit expliziter QoS (Spec §2.2.2.2.1.6).</summary>
    public Publisher(DomainParticipant dp, ZeroDDS.Qos.PublisherQos qos)
    {
        _participant = dp.Handle;
        using var scope = new ZeroDDS.QosBridge.NativeQosScope();
        var native = ZeroDDS.QosBridge.QosBridge.ToNative(qos, scope);
        unsafe { _handle = Native.DpCreatePublisher(_participant, (IntPtr)(&native)); }
        if (_handle == IntPtr.Zero) throw new DdsError("Publisher::create with QoS failed");
    }

    /// <summary>Native handle.</summary>
    public IntPtr Handle => _handle;

    /// <summary>Suspend.</summary>
    public void Suspend() =>
        StatusCheck.Check(Native.PubSuspendPublications(_handle), "Publisher::Suspend");
    /// <summary>Resume.</summary>
    public void Resume() =>
        StatusCheck.Check(Native.PubResumePublications(_handle), "Publisher::Resume");
    /// <summary>Wait for acknowledgments.</summary>
    public void WaitForAcks(Duration timeout) =>
        StatusCheck.Check(Native.PubWaitForAcks(_handle, timeout.TotalMilliseconds),
            "Publisher::WaitForAcks");

    public void Dispose()
    {
        if (_disposed) return;
        _disposed = true;
        if (_handle != IntPtr.Zero)
        {
            Native.DpDeletePublisher(_participant, _handle);
            _handle = IntPtr.Zero;
        }
        GC.SuppressFinalize(this);
    }
    ~Publisher() { Dispose(); }
}

/// <summary>DataWriter&lt;T&gt; (Spec §7.5.14.5).</summary>
public sealed class DataWriter<T> : IDisposable
{
    private IntPtr _handle;
    private readonly IntPtr _publisher;
    private readonly ITopicTraits<T> _traits;
    private bool _disposed;

    /// <summary>Konstruiert via Pub + Topic.</summary>
    public DataWriter(Publisher pub, Topic<T> topic)
    {
        _publisher = pub.Handle;
        _handle = Native.PubCreateDatawriter(_publisher, topic.Handle, IntPtr.Zero);
        if (_handle == IntPtr.Zero) throw new DdsError("DataWriter::create failed");
        _traits = topic.Traits;
    }

    /// <summary>Konstruiert mit expliziter QoS (Spec §2.2.2.4.1.5).</summary>
    public DataWriter(Publisher pub, Topic<T> topic, ZeroDDS.Qos.DataWriterQos qos)
    {
        _publisher = pub.Handle;
        using var scope = new ZeroDDS.QosBridge.NativeQosScope();
        var native = ZeroDDS.QosBridge.QosBridge.ToNative(qos, scope);
        unsafe { _handle = Native.PubCreateDatawriter(_publisher, topic.Handle, (IntPtr)(&native)); }
        if (_handle == IntPtr.Zero) throw new DdsError("DataWriter::create with QoS failed");
        _traits = topic.Traits;
    }

    /// <summary>Native handle.</summary>
    public IntPtr Handle => _handle;

    /// <summary>Schreibt eine Sample-Instanz.</summary>
    public void Write(T sample)
    {
        var bytes = _traits.Encode(sample);
        unsafe
        {
            fixed (byte* p = bytes)
            {
                int rc = Native.DwWrite(_handle, (IntPtr)p, (UIntPtr)bytes.Length, 0);
                StatusCheck.Check(rc, "DataWriter::Write");
            }
        }
    }

    /// <summary>Wait for acknowledgments.</summary>
    public void WaitForAcks(Duration timeout) =>
        StatusCheck.Check(Native.DwWaitForAcks(_handle, timeout.TotalMilliseconds),
            "DataWriter::WaitForAcks");

    /// <summary>Wait for matched.</summary>
    public void WaitForMatched(int min, Duration timeout) =>
        StatusCheck.Check(Native.DwWaitForMatched(_handle, min, timeout.TotalMilliseconds),
            "DataWriter::WaitForMatched");

    /// <summary>Liveliness asserten.</summary>
    public void AssertLiveliness() =>
        StatusCheck.Check(Native.DwAssertLiveliness(_handle), "DataWriter::AssertLiveliness");

    /// <summary>Publication-matched-status.</summary>
    public ZeroDDS.Status.PublicationMatchedStatus GetPublicationMatchedStatus()
    {
        StatusCheck.Check(Native.DwGetPublicationMatchedStatus(_handle, out var s),
            "DataWriter::GetPublicationMatchedStatus");
        return new ZeroDDS.Status.PublicationMatchedStatus(
            s.TotalCount, s.TotalCountChange, s.CurrentCount, s.CurrentCountChange,
            new InstanceHandle(s.LastSubscriptionHandle));
    }

    /// <summary>Liveliness-lost status.</summary>
    public ZeroDDS.Status.LivelinessLostStatus GetLivelinessLostStatus()
    {
        StatusCheck.Check(Native.DwGetLivelinessLostStatus(_handle, out var s),
            "DataWriter::GetLivelinessLostStatus");
        return new ZeroDDS.Status.LivelinessLostStatus(s.TotalCount, s.TotalCountChange);
    }

    /// <summary>Offered-deadline-missed status.</summary>
    public ZeroDDS.Status.OfferedDeadlineMissedStatus GetOfferedDeadlineMissedStatus()
    {
        StatusCheck.Check(Native.DwGetOfferedDeadlineMissedStatus(_handle, out var s),
            "DataWriter::GetOfferedDeadlineMissedStatus");
        return new ZeroDDS.Status.OfferedDeadlineMissedStatus(
            s.TotalCount, s.TotalCountChange, new InstanceHandle(s.LastInstanceHandle));
    }

    /// <summary>Offered-incompatible-QoS status.</summary>
    public ZeroDDS.Status.OfferedIncompatibleQosStatus GetOfferedIncompatibleQosStatus()
    {
        StatusCheck.Check(Native.DwGetOfferedIncompatibleQosStatus(_handle, out var s),
            "DataWriter::GetOfferedIncompatibleQosStatus");
        return new ZeroDDS.Status.OfferedIncompatibleQosStatus(
            s.TotalCount, s.TotalCountChange, s.LastPolicyId);
    }

    public void Dispose()
    {
        if (_disposed) return;
        _disposed = true;
        if (_handle != IntPtr.Zero)
        {
            Native.PubDeleteDatawriter(_publisher, _handle);
            _handle = IntPtr.Zero;
        }
        GC.SuppressFinalize(this);
    }
    ~DataWriter() { Dispose(); }
}
