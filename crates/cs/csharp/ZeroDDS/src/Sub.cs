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

    /// <summary>Constructs with explicit QoS (Spec §2.2.2.2.1.7).</summary>
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

    // Samples buffered by WaitForData (see WaitForData remarks). They are
    // logically still "unread" — Take() returns them ahead of the live reader
    // so that wait-then-take never drops a sample.
    private readonly object _pendingLock = new();
    private System.Collections.Generic.Queue<Sample<T>>? _pending;

    // Event-driven readiness wait state, created lazily on the first
    // WaitForData call and reused across calls (disposed with the reader):
    //   * a ReadCondition over NOT_READ samples (native trigger flips true when
    //     a sample arrives, woken via the reader's data waker — no busy poll),
    //   * a GuardCondition tripped by the CancellationToken for instant cancel,
    //   * a WaitSet holding both.
    private readonly object _waitLock = new();
    private IntPtr _readCond = IntPtr.Zero;
    private IntPtr _guardCond = IntPtr.Zero;
    private IntPtr _waitSet = IntPtr.Zero;

    // Sample-state mask for "unread" samples (matches the native read cache).
    private const uint SampleStateNotRead = 2;
    private const uint StateAny = 0;

    public DataReader(Subscriber sub, Topic<T> topic)
    {
        _subscriber = sub.Handle;
        _handle = Native.SubCreateDatareader(_subscriber, topic.Handle, IntPtr.Zero);
        if (_handle == IntPtr.Zero) throw new DdsError("DataReader::create failed");
        _traits = topic.Traits;
    }

    /// <summary>Constructs with explicit QoS (Spec §2.2.2.5.1.5).</summary>
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
        // Drain any samples buffered by a prior WaitForData first, then top up
        // from the live reader. This preserves at-most-once delivery across the
        // wait/take boundary.
        List<Sample<T>>? buffered = null;
        lock (_pendingLock)
        {
            if (_pending is { Count: > 0 })
            {
                buffered = new List<Sample<T>>(_pending.Count);
                while (_pending.Count > 0) buffered.Add(_pending.Dequeue());
            }
        }

        if (buffered is null)
            return TakeNative(maxSamples);

        if (maxSamples > 0 && buffered.Count >= maxSamples)
            return buffered.GetRange(0, maxSamples);

        int remaining = maxSamples > 0 ? maxSamples - buffered.Count : 0;
        var live = TakeNative(remaining);
        buffered.AddRange(live);
        return buffered;
    }

    private List<Sample<T>> TakeNative(int maxSamples)
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

    /// <summary>
    /// Blocks until unread data is available or <paramref name="timeout"/>
    /// elapses. Returns <c>true</c> when data became available, <c>false</c>
    /// on timeout. Any samples observed are buffered and returned by the next
    /// <see cref="Take(int)"/>, so a wait-then-take never drops a sample.
    /// </summary>
    public bool WaitForData(Duration timeout) =>
        WaitForData(timeout, System.Threading.CancellationToken.None);

    /// <summary>TimeSpan overload of <see cref="WaitForData(Duration)"/>.</summary>
    public bool WaitForData(TimeSpan timeout) =>
        WaitForData(ZeroDDS.TimeSpanBridge.ToDuration(timeout));

    /// <summary>
    /// Cancellable wait-for-data.
    ///
    /// Blocks on a native WaitSet over a NOT_READ <c>ReadCondition</c>: the
    /// reader's data-available signal wakes the wait the instant a sample
    /// arrives (event-driven — no busy poll). The observed samples are buffered
    /// into the reader so the subsequent <see cref="Take(int)"/> returns them,
    /// so a wait-then-take never drops a sample. Cancellation is event-driven
    /// too: <paramref name="ct"/> trips a GuardCondition also attached to the
    /// WaitSet, waking the wait immediately and surfacing as
    /// <see cref="OperationCanceledException"/>.
    /// </summary>
    /// <exception cref="OperationCanceledException">If <paramref name="ct"/> fires.</exception>
    public bool WaitForData(Duration timeout, System.Threading.CancellationToken ct)
    {
        ct.ThrowIfCancellationRequested();

        // Already buffered from an earlier wait?
        lock (_pendingLock)
        {
            if (_pending is { Count: > 0 }) return true;
        }

        EnsureWaitState();

        // Reset the cancellation guard and arm the CT → guard trip (event-driven
        // cancel; the registration is disposed at the end of the wait).
        StatusCheck.Check(Native.GuardConditionSetTrigger(_guardCond, false),
            "WaitForData::resetGuard");
        using var ctReg = ct.CanBeCanceled
            ? ct.Register(static s => Native.GuardConditionSetTrigger((IntPtr)s!, true), _guardCond)
            : default;

        bool infinite = timeout.IsInfinite;
        long budgetMs = infinite ? long.MaxValue : (long)timeout.TotalMilliseconds;
        var sw = System.Diagnostics.Stopwatch.StartNew();

        var buf = new IntPtr[8];
        while (true)
        {
            // A sample may already be takeable before we ever block (the writer
            // raced ahead, or a previous wake delivered it). Drain first.
            var fresh = TakeNative(0);
            if (fresh.Count > 0)
            {
                lock (_pendingLock)
                {
                    _pending ??= new System.Collections.Generic.Queue<Sample<T>>();
                    foreach (var s in fresh) _pending.Enqueue(s);
                }
                return true;
            }

            if (ct.IsCancellationRequested)
                throw new OperationCanceledException(ct);

            long elapsed = sw.ElapsedMilliseconds;
            if (!infinite && elapsed >= budgetMs)
                return false;

            // Remaining budget for this blocking wait.
            Duration block;
            if (infinite)
            {
                block = Duration.Infinite;
            }
            else
            {
                long left = budgetMs - elapsed;
                if (left <= 0) return false;
                block = Duration.FromMillis(left);
            }

            UIntPtr count;
            int rc;
            unsafe
            {
                fixed (IntPtr* p = buf)
                {
                    rc = Native.WaitSetWait(_waitSet, (IntPtr)p, (UIntPtr)buf.Length,
                        out count, block.Sec, block.Nanosec);
                }
            }

            if (rc == Native.Timeout)
            {
                // No condition tripped within the budget.
                if (ct.IsCancellationRequested)
                    throw new OperationCanceledException(ct);
                return false;
            }
            StatusCheck.Check(rc, "WaitForData::WaitSetWait");

            // Woken: either the read condition (data) or the guard (cancel). The
            // loop top re-checks both via TakeNative + the CT flag, so we do not
            // need to inspect the active-condition list here.
            if (ct.IsCancellationRequested)
                throw new OperationCanceledException(ct);
        }
    }

    /// Lazily builds the reusable ReadCondition + GuardCondition + WaitSet for
    /// the event-driven readiness wait. Idempotent; cheap after the first call.
    private void EnsureWaitState()
    {
        if (_waitSet != IntPtr.Zero) return;
        lock (_waitLock)
        {
            if (_waitSet != IntPtr.Zero) return;

            var rcCond = Native.DrCreateReadCondition(_handle, SampleStateNotRead, StateAny, StateAny);
            if (rcCond == IntPtr.Zero) throw new DdsError("WaitForData: ReadCondition create failed");

            var guard = Native.GuardConditionCreate();
            if (guard == IntPtr.Zero)
            {
                Native.DrDeleteReadCondition(_handle, rcCond);
                throw new DdsError("WaitForData: GuardCondition create failed");
            }

            var ws = Native.WaitSetCreate();
            if (ws == IntPtr.Zero)
            {
                Native.GuardConditionDestroy(guard);
                Native.DrDeleteReadCondition(_handle, rcCond);
                throw new DdsError("WaitForData: WaitSet create failed");
            }

            StatusCheck.Check(Native.WaitSetAttach(ws, rcCond), "WaitForData: attach ReadCondition");
            StatusCheck.Check(Native.WaitSetAttach(ws, guard), "WaitForData: attach GuardCondition");

            _readCond = rcCond;
            _guardCond = guard;
            _waitSet = ws;
        }
    }

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
        // Tear down the readiness-wait state before the reader handle: the
        // WaitSet first (releases its references to the conditions), then the
        // conditions themselves.
        lock (_waitLock)
        {
            if (_waitSet != IntPtr.Zero) { Native.WaitSetDestroy(_waitSet); _waitSet = IntPtr.Zero; }
            if (_guardCond != IntPtr.Zero) { Native.GuardConditionDestroy(_guardCond); _guardCond = IntPtr.Zero; }
            if (_readCond != IntPtr.Zero && _handle != IntPtr.Zero)
            {
                Native.DrDeleteReadCondition(_handle, _readCond);
                _readCond = IntPtr.Zero;
            }
        }
        if (_handle != IntPtr.Zero)
        {
            Native.SubDeleteDatareader(_subscriber, _handle);
            _handle = IntPtr.Zero;
        }
        GC.SuppressFinalize(this);
    }
    ~DataReader() { Dispose(); }
}
