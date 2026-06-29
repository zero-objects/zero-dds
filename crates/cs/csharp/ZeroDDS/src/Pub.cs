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
    private readonly System.Collections.Generic.List<string> _partition = new();
    private bool _disposed;

    /// <summary>Constructs via a Participant.</summary>
    public Publisher(DomainParticipant dp)
    {
        _participant = dp.Handle;
        _handle = Native.DpCreatePublisher(_participant, IntPtr.Zero);
        if (_handle == IntPtr.Zero) throw new DdsError("Publisher::create failed");
    }

    /// <summary>Constructs with explicit QoS (Spec §2.2.2.2.1.6).</summary>
    public Publisher(DomainParticipant dp, ZeroDDS.Qos.PublisherQos qos)
    {
        _participant = dp.Handle;
        if (qos.Partition?.Names is { Count: > 0 }) _partition.AddRange(qos.Partition.Names);
        using var scope = new ZeroDDS.QosBridge.NativeQosScope();
        var native = ZeroDDS.QosBridge.QosBridge.ToNative(qos, scope);
        unsafe { _handle = Native.DpCreatePublisher(_participant, (IntPtr)(&native)); }
        if (_handle == IntPtr.Zero) throw new DdsError("Publisher::create with QoS failed");
    }

    /// <summary>Native handle.</summary>
    public IntPtr Handle => _handle;

    /// <summary>
    /// The PARTITION names this Publisher was created with (Spec §2.2.3.10).
    /// Writers created on it inherit these names unless they set their own.
    /// </summary>
    internal System.Collections.Generic.IReadOnlyList<string> PartitionNames => _partition;

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

    /// <summary>Constructs via Pub + Topic.</summary>
    public DataWriter(Publisher pub, Topic<T> topic)
    {
        _publisher = pub.Handle;
        // PARTITION (Spec §2.2.3.10) is a Publisher-level policy that writers
        // inherit. The C-FFI applies partition off the writer QoS, so when the
        // Publisher carries a partition we materialise a default writer QoS that
        // carries it forward (otherwise the writer lands in the default partition
        // and would wrongly match readers in other partitions).
        if (pub.PartitionNames.Count > 0)
        {
            var qos = new ZeroDDS.Qos.DataWriterQos();
            qos.Partition.Names.AddRange(pub.PartitionNames);
            using var scope = new ZeroDDS.QosBridge.NativeQosScope();
            var native = ZeroDDS.QosBridge.QosBridge.ToNative(qos, scope);
            unsafe { _handle = Native.PubCreateDatawriter(_publisher, topic.Handle, (IntPtr)(&native)); }
        }
        else
        {
            _handle = Native.PubCreateDatawriter(_publisher, topic.Handle, IntPtr.Zero);
        }
        if (_handle == IntPtr.Zero) throw new DdsError("DataWriter::create failed");
        _traits = topic.Traits;
    }

    /// <summary>Constructs with explicit QoS (Spec §2.2.2.4.1.5).</summary>
    public DataWriter(Publisher pub, Topic<T> topic, ZeroDDS.Qos.DataWriterQos qos)
    {
        _publisher = pub.Handle;
        // Inherit the Publisher's PARTITION when the writer QoS does not set one.
        if (qos.Partition.Names.Count == 0 && pub.PartitionNames.Count > 0)
            qos.Partition.Names.AddRange(pub.PartitionNames);
        using var scope = new ZeroDDS.QosBridge.NativeQosScope();
        var native = ZeroDDS.QosBridge.QosBridge.ToNative(qos, scope);
        unsafe { _handle = Native.PubCreateDatawriter(_publisher, topic.Handle, (IntPtr)(&native)); }
        if (_handle == IntPtr.Zero) throw new DdsError("DataWriter::create with QoS failed");
        _traits = topic.Traits;
    }

    /// <summary>Native handle.</summary>
    public IntPtr Handle => _handle;

    /// <summary>Writes a sample instance.</summary>
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

    // --- Keyed instance lifecycle (Spec §2.2.2.4.2.x) ---
    //
    // The C-FFI lifecycle path is key-hash-based: register/dispose/unregister
    // take the 16-byte XTypes key hash (§7.6.8) of the instance, which the
    // generated TypeSupport produces via KeyHash(sample). The typed traits
    // expose that here so the binding can drive the lifecycle without the caller
    // hand-rolling the key hash.

    private byte[] KeyHash(T sample)
    {
        if (_traits is ZeroDDS.IKeyHashProvider kp) return kp.KeyHashOf(sample!);
        throw new ZeroDDS.Core.UnsupportedException(
            "DataWriter::keyed-lifecycle requires a generated TypeSupport (IDdsTopicType<T>) " +
            "so the 16-byte key hash can be computed");
    }

    /// <summary>
    /// Informs DDS that the application will modify a particular instance,
    /// returning its <see cref="InstanceHandle"/> (Spec §2.2.2.4.2.5
    /// <c>register_instance</c>). The handle may be passed to subsequent
    /// dispose/unregister calls.
    /// </summary>
    public InstanceHandle RegisterInstance(T sample)
    {
        var key = KeyHash(sample);
        ulong h;
        unsafe
        {
            fixed (byte* p = key)
            {
                StatusCheck.Check(
                    Native.DwRegisterInstance(_handle, (IntPtr)p, (UIntPtr)key.Length, out h),
                    "DataWriter::RegisterInstance");
            }
        }
        return new InstanceHandle(h);
    }

    /// <summary>
    /// Looks up the <see cref="InstanceHandle"/> for an instance's key without
    /// registering it (Spec §2.2.2.4.2.10 <c>lookup_instance</c>).
    /// </summary>
    public InstanceHandle LookupInstance(T sample)
    {
        var key = KeyHash(sample);
        ulong h;
        unsafe
        {
            fixed (byte* p = key)
            {
                StatusCheck.Check(
                    Native.DwLookupInstance(_handle, (IntPtr)p, (UIntPtr)key.Length, out h),
                    "DataWriter::LookupInstance");
            }
        }
        return new InstanceHandle(h);
    }

    /// <summary>
    /// Reverses <see cref="RegisterInstance"/>: signals that the writer no longer
    /// has data for the instance (Spec §2.2.2.4.2.7 <c>unregister_instance</c>).
    /// Readers observe NOT_ALIVE_NO_WRITERS once no writer keeps it alive.
    /// </summary>
    public void UnregisterInstance(InstanceHandle handle) =>
        StatusCheck.Check(Native.DwUnregisterInstance(_handle, handle.Value),
            "DataWriter::UnregisterInstance");

    /// <summary>
    /// Unregisters by sample (resolves the key hash to a handle first).
    /// </summary>
    public void UnregisterInstance(T sample) =>
        UnregisterInstance(LookupInstance(sample));

    /// <summary>
    /// Unregisters with an explicit source timestamp
    /// (Spec §2.2.2.4.2.8 <c>unregister_instance_w_timestamp</c>).
    /// </summary>
    public void UnregisterInstance(InstanceHandle handle, Time sourceTimestamp) =>
        StatusCheck.Check(
            Native.DwUnregisterInstanceWTimestamp(_handle, handle.Value,
                sourceTimestamp.Sec, sourceTimestamp.Nanosec),
            "DataWriter::UnregisterInstance(w_timestamp)");

    /// <summary>
    /// Disposes the instance carried by <paramref name="sample"/>: readers
    /// observe NOT_ALIVE_DISPOSED (Spec §2.2.2.4.2.13 <c>dispose</c>). The key
    /// hash identifies the instance.
    /// </summary>
    public void DisposeInstance(T sample)
    {
        var key = KeyHash(sample);
        unsafe
        {
            fixed (byte* p = key)
            {
                StatusCheck.Check(Native.DwDispose(_handle, (IntPtr)p, 0),
                    "DataWriter::DisposeInstance");
            }
        }
    }

    /// <summary>
    /// Disposes the instance with an explicit source timestamp
    /// (Spec §2.2.2.4.2.14 <c>dispose_w_timestamp</c>).
    /// </summary>
    public void DisposeInstance(T sample, Time sourceTimestamp)
    {
        var key = KeyHash(sample);
        unsafe
        {
            fixed (byte* p = key)
            {
                StatusCheck.Check(
                    Native.DwDisposeWTimestamp(_handle, (IntPtr)p, 0,
                        sourceTimestamp.Sec, sourceTimestamp.Nanosec),
                    "DataWriter::DisposeInstance(w_timestamp)");
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

    /// <summary>Assert liveliness.</summary>
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
