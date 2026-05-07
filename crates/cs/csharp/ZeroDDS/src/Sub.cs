// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Sub.cs — DDS-PSM-Cxx 1.0 §7.5.15.

using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using ZeroDDS.Core;
using ZeroDDS.Domain;
using ZeroDDS.Topic;

namespace ZeroDDS.Sub;

/// <summary>SampleInfo (Spec §7.5.15.6).</summary>
public sealed class SampleInfo
{
    public uint SampleState { get; init; }
    public uint ViewState { get; init; }
    public uint InstanceState { get; init; }
    public bool ValidData { get; init; }
    public InstanceHandle InstanceHandle { get; init; }
    public InstanceHandle PublicationHandle { get; init; }
    public Time SourceTimestamp { get; init; }
}

/// <summary>Sample&lt;T&gt; (Spec §7.5.15.5).</summary>
public sealed class Sample<T>
{
    public T Data { get; init; } = default!;
    public SampleInfo Info { get; init; } = new();
}

/// <summary>Subscriber (Spec §7.5.15.1).</summary>
public sealed class Subscriber : IDisposable
{
    private IntPtr _handle;
    private readonly IntPtr _participant;
    private bool _disposed;

    public Subscriber(DomainParticipant dp)
    {
        _participant = dp.Handle;
        _handle = Native.DpCreateSubscriber(_participant, IntPtr.Zero);
        if (_handle == IntPtr.Zero) throw new DdsError("Subscriber::create failed");
    }

    /// <summary>Konstruiert mit expliziter QoS (Spec §2.2.2.2.1.7).</summary>
    public Subscriber(DomainParticipant dp, ZeroDDS.Qos.SubscriberQos qos)
    {
        _participant = dp.Handle;
        using var scope = new ZeroDDS.QosBridge.NativeQosScope();
        var native = ZeroDDS.QosBridge.QosBridge.ToNative(qos, scope);
        unsafe { _handle = Native.DpCreateSubscriber(_participant, (IntPtr)(&native)); }
        if (_handle == IntPtr.Zero) throw new DdsError("Subscriber::create with QoS failed");
    }

    public IntPtr Handle => _handle;

    public void Dispose()
    {
        if (_disposed) return;
        _disposed = true;
        if (_handle != IntPtr.Zero)
        {
            Native.DpDeleteSubscriber(_participant, _handle);
            _handle = IntPtr.Zero;
        }
        GC.SuppressFinalize(this);
    }
    ~Subscriber() { Dispose(); }
}

/// <summary>DataReader&lt;T&gt; (Spec §7.5.15.5).</summary>
public sealed class DataReader<T> : IDisposable
{
    private IntPtr _handle;
    private readonly IntPtr _subscriber;
    private readonly ITopicTraits<T> _traits;
    private bool _disposed;

    public DataReader(Subscriber sub, Topic<T> topic)
    {
        _subscriber = sub.Handle;
        _handle = Native.SubCreateDatareader(_subscriber, topic.Handle, IntPtr.Zero);
        if (_handle == IntPtr.Zero) throw new DdsError("DataReader::create failed");
        _traits = topic.Traits;
    }

    /// <summary>Konstruiert mit expliziter QoS (Spec §2.2.2.5.1.5).</summary>
    public DataReader(Subscriber sub, Topic<T> topic, ZeroDDS.Qos.DataReaderQos qos)
    {
        _subscriber = sub.Handle;
        using var scope = new ZeroDDS.QosBridge.NativeQosScope();
        var native = ZeroDDS.QosBridge.QosBridge.ToNative(qos, scope);
        unsafe { _handle = Native.SubCreateDatareader(_subscriber, topic.Handle, (IntPtr)(&native)); }
        if (_handle == IntPtr.Zero) throw new DdsError("DataReader::create with QoS failed");
        _traits = topic.Traits;
    }

    public IntPtr Handle => _handle;

    /// <summary>Take samples.</summary>
    public List<Sample<T>> Take(int maxSamples = 0)
    {
        var arr = default(Native.SampleArray);
        int rc = Native.DrTake(_handle, ref arr, (UIntPtr)maxSamples, 0, 0, 0);
        if (rc == Native.NoData)
        {
            return new List<Sample<T>>();
        }
        StatusCheck.Check(rc, "DataReader::Take");

        var result = new List<Sample<T>>((int)arr.Count);
        unsafe
        {
            byte** buffers = (byte**)arr.Buffers.ToPointer();
            UIntPtr* lengths = (UIntPtr*)arr.Lengths.ToPointer();
            Native.SampleInfoNative* infos = (Native.SampleInfoNative*)arr.Infos.ToPointer();
            int count = (int)(uint)arr.Count;
            for (int i = 0; i < count; ++i)
            {
                var info = infos[i];
                T data = default!;
                if (info.ValidData && (uint)lengths[i] > 0)
                {
                    var len = (int)(uint)lengths[i];
                    var span = new ReadOnlySpan<byte>(buffers[i], len);
                    data = _traits.Decode(span);
                }
                result.Add(new Sample<T>
                {
                    Data = data,
                    Info = new SampleInfo
                    {
                        SampleState = info.SampleState,
                        ViewState = info.ViewState,
                        InstanceState = info.InstanceState,
                        ValidData = info.ValidData,
                        InstanceHandle = new InstanceHandle(info.InstanceHandle),
                        PublicationHandle = new InstanceHandle(info.PublicationHandle),
                        SourceTimestamp = new Time(info.SourceTimestampSec, info.SourceTimestampNanosec),
                    },
                });
            }
        }
        StatusCheck.Check(Native.DrReturnLoan(_handle, ref arr), "DataReader::ReturnLoan");
        return result;
    }

    public void WaitForMatched(int min, Duration timeout) =>
        StatusCheck.Check(Native.DrWaitForMatched(_handle, min, timeout.TotalMilliseconds),
            "DataReader::WaitForMatched");

    public ZeroDDS.Status.SubscriptionMatchedStatus GetSubscriptionMatchedStatus()
    {
        StatusCheck.Check(Native.DrGetSubscriptionMatchedStatus(_handle, out var s),
            "DataReader::GetSubscriptionMatchedStatus");
        return new ZeroDDS.Status.SubscriptionMatchedStatus(
            s.TotalCount, s.TotalCountChange, s.CurrentCount, s.CurrentCountChange,
            new InstanceHandle(s.LastPublicationHandle));
    }

    public ZeroDDS.Status.SampleLostStatus GetSampleLostStatus()
    {
        StatusCheck.Check(Native.DrGetSampleLostStatus(_handle, out var s),
            "DataReader::GetSampleLostStatus");
        return new ZeroDDS.Status.SampleLostStatus(s.TotalCount, s.TotalCountChange);
    }

    /// <summary>Liveliness-changed status.</summary>
    public ZeroDDS.Status.LivelinessChangedStatus GetLivelinessChangedStatus()
    {
        StatusCheck.Check(Native.DrGetLivelinessChangedStatus(_handle, out var s),
            "DataReader::GetLivelinessChangedStatus");
        return new ZeroDDS.Status.LivelinessChangedStatus(
            s.AliveCount, s.NotAliveCount, s.AliveCountChange, s.NotAliveCountChange,
            new InstanceHandle(s.LastPublicationHandle));
    }

    /// <summary>Requested-deadline-missed status.</summary>
    public ZeroDDS.Status.RequestedDeadlineMissedStatus GetRequestedDeadlineMissedStatus()
    {
        StatusCheck.Check(Native.DrGetRequestedDeadlineMissedStatus(_handle, out var s),
            "DataReader::GetRequestedDeadlineMissedStatus");
        return new ZeroDDS.Status.RequestedDeadlineMissedStatus(
            s.TotalCount, s.TotalCountChange, new InstanceHandle(s.LastInstanceHandle));
    }

    /// <summary>Requested-incompatible-QoS status.</summary>
    public ZeroDDS.Status.RequestedIncompatibleQosStatus GetRequestedIncompatibleQosStatus()
    {
        StatusCheck.Check(Native.DrGetRequestedIncompatibleQosStatus(_handle, out var s),
            "DataReader::GetRequestedIncompatibleQosStatus");
        return new ZeroDDS.Status.RequestedIncompatibleQosStatus(
            s.TotalCount, s.TotalCountChange, s.LastPolicyId);
    }

    /// <summary>Sample-rejected status.</summary>
    public ZeroDDS.Status.SampleRejectedStatus GetSampleRejectedStatus()
    {
        StatusCheck.Check(Native.DrGetSampleRejectedStatus(_handle, out var s),
            "DataReader::GetSampleRejectedStatus");
        return new ZeroDDS.Status.SampleRejectedStatus(
            s.TotalCount, s.TotalCountChange, s.LastReason,
            new InstanceHandle(s.LastInstanceHandle));
    }

    /// <summary>Read samples (RC1: alias to Take, see Vendor-Spec §3 for read/take separation roadmap).</summary>
    public List<Sample<T>> Read(int maxSamples = 0) => Take(maxSamples);

    public void Dispose()
    {
        if (_disposed) return;
        _disposed = true;
        if (_handle != IntPtr.Zero)
        {
            Native.SubDeleteDatareader(_subscriber, _handle);
            _handle = IntPtr.Zero;
        }
        GC.SuppressFinalize(this);
    }
    ~DataReader() { Dispose(); }
}
