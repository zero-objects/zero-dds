// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Listener.cs — DDS-PSM-Cxx 1.0 §7.5.9 + vendor spec
//               `zerodds-listener-callbacks-1.1` §7.2.
//
// The C-FFI listener layer (crates/zerodds-c-api/src/listener_ffi.rs) takes a
// struct of function pointers plus one `void* user_data` and stores only the
// POINTER to it (spec §4.3, §9). Two consequences shape this file:
//
//   * the vtable lives in unmanaged memory owned by the registration, not on
//     the stack of Attach — a stack copy would be a dangling pointer the
//     moment Attach returned;
//   * the registration clears the entity's slot before it frees anything,
//     because the registry holds a weak raw pointer (spec §9).
//
// The callbacks themselves are `[UnmanagedCallersOnly]` statics: no delegate
// marshalling, no reflection, no GC allocation on the callback path, which is
// what NativeAOT needs (spec §7.2). `[UnmanagedCallersOnly]` cannot be generic,
// so the trampolines dispatch through the non-generic sink interfaces below and
// the closed generic type is reached without a single reflective lookup.

using System;
using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;
using ZeroDDS.Core;
using ZeroDDS.Pub;
using ZeroDDS.Status;
using ZeroDDS.Sub;

namespace ZeroDDS.Listener;

/// <summary>DataWriterListener (Spec §7.5.9.5).</summary>
public interface IDataWriterListener<T>
{
    void OnLivelinessLost(DataWriter<T> dw, LivelinessLostStatus s);
    void OnOfferedDeadlineMissed(DataWriter<T> dw, OfferedDeadlineMissedStatus s);
    void OnOfferedIncompatibleQos(DataWriter<T> dw, OfferedIncompatibleQosStatus s);
    void OnPublicationMatched(DataWriter<T> dw, PublicationMatchedStatus s);
}

/// <summary>DataReaderListener (Spec §7.5.9.6).</summary>
public interface IDataReaderListener<T>
{
    void OnDataAvailable(DataReader<T> dr);
    void OnSampleRejected(DataReader<T> dr, SampleRejectedStatus s);
    void OnLivelinessChanged(DataReader<T> dr, LivelinessChangedStatus s);
    void OnRequestedDeadlineMissed(DataReader<T> dr, RequestedDeadlineMissedStatus s);
    void OnRequestedIncompatibleQos(DataReader<T> dr, RequestedIncompatibleQosStatus s);
    void OnSubscriptionMatched(DataReader<T> dr, SubscriptionMatchedStatus s);
    void OnSampleLost(DataReader<T> dr, SampleLostStatus s);
}

// ---------------------------------------------------------------------------
// Non-generic dispatch seam.
//
// The C callback carries `(void* user_data, entity*)` and nothing else, so the
// status value the DDS-PSM-Cxx interface wants is read back from the entity
// through the status getters the binding already has. That read happens on the
// caller's poll thread, which spec §4.2 explicitly permits.
// ---------------------------------------------------------------------------

internal interface IWriterCallbackSink
{
    void LivelinessLost();
    void OfferedDeadlineMissed();
    void OfferedIncompatibleQos();
    void PublicationMatched();
}

internal interface IReaderCallbackSink
{
    void DataAvailable();
    void SampleRejected();
    void LivelinessChanged();
    void RequestedDeadlineMissed();
    void RequestedIncompatibleQos();
    void SubscriptionMatched();
    void SampleLost();
}

internal sealed class WriterCallbackSink<T> : IWriterCallbackSink
{
    private readonly DataWriter<T> _dw;
    private readonly IDataWriterListener<T> _listener;

    internal WriterCallbackSink(DataWriter<T> dw, IDataWriterListener<T> listener)
    {
        _dw = dw;
        _listener = listener;
    }

    public void LivelinessLost() => _listener.OnLivelinessLost(_dw, _dw.GetLivelinessLostStatus());

    public void OfferedDeadlineMissed() =>
        _listener.OnOfferedDeadlineMissed(_dw, _dw.GetOfferedDeadlineMissedStatus());

    public void OfferedIncompatibleQos() =>
        _listener.OnOfferedIncompatibleQos(_dw, _dw.GetOfferedIncompatibleQosStatus());

    public void PublicationMatched() =>
        _listener.OnPublicationMatched(_dw, _dw.GetPublicationMatchedStatus());
}

internal sealed class ReaderCallbackSink<T> : IReaderCallbackSink
{
    private readonly DataReader<T> _dr;
    private readonly IDataReaderListener<T> _listener;

    internal ReaderCallbackSink(DataReader<T> dr, IDataReaderListener<T> listener)
    {
        _dr = dr;
        _listener = listener;
    }

    public void DataAvailable() => _listener.OnDataAvailable(_dr);

    public void SampleRejected() => _listener.OnSampleRejected(_dr, _dr.GetSampleRejectedStatus());

    public void LivelinessChanged() =>
        _listener.OnLivelinessChanged(_dr, _dr.GetLivelinessChangedStatus());

    public void RequestedDeadlineMissed() =>
        _listener.OnRequestedDeadlineMissed(_dr, _dr.GetRequestedDeadlineMissedStatus());

    public void RequestedIncompatibleQos() =>
        _listener.OnRequestedIncompatibleQos(_dr, _dr.GetRequestedIncompatibleQosStatus());

    public void SubscriptionMatched() =>
        _listener.OnSubscriptionMatched(_dr, _dr.GetSubscriptionMatchedStatus());

    public void SampleLost() => _listener.OnSampleLost(_dr, _dr.GetSampleLostStatus());
}

// ---------------------------------------------------------------------------
// The C entry points.
// ---------------------------------------------------------------------------

internal static unsafe class ListenerTrampolines
{
    /// <summary>
    /// Resolves the `void* user_data` back to the sink. Returns null for a
    /// pointer that is no longer a live handle, which is what a callback fired
    /// between a detach and the caller dropping its reference would see.
    /// </summary>
    [MethodImpl(MethodImplOptions.AggressiveInlining)]
    private static TSink? Sink<TSink>(IntPtr userData)
        where TSink : class
    {
        if (userData == IntPtr.Zero)
        {
            return null;
        }
        return GCHandle.FromIntPtr(userData).Target as TSink;
    }

    // An exception must not unwind through the native frame — the C caller has
    // no handler and the process would go down inside `zerodds_poll_listeners`.
    // It is reported on stderr and swallowed at the boundary instead.
    private static void Guard<TSink>(IntPtr userData, Action<TSink> invoke)
        where TSink : class
    {
        try
        {
            var sink = Sink<TSink>(userData);
            if (sink != null)
            {
                invoke(sink);
            }
        }
        catch (Exception e)
        {
            Console.Error.WriteLine($"ZeroDDS listener callback threw: {e}");
        }
    }

    [UnmanagedCallersOnly(CallConvs = new[] { typeof(CallConvCdecl) })]
    internal static void WriterLivelinessLost(IntPtr userData, IntPtr dw) =>
        Guard<IWriterCallbackSink>(userData, s => s.LivelinessLost());

    [UnmanagedCallersOnly(CallConvs = new[] { typeof(CallConvCdecl) })]
    internal static void WriterOfferedDeadlineMissed(IntPtr userData, IntPtr dw) =>
        Guard<IWriterCallbackSink>(userData, s => s.OfferedDeadlineMissed());

    [UnmanagedCallersOnly(CallConvs = new[] { typeof(CallConvCdecl) })]
    internal static void WriterOfferedIncompatibleQos(IntPtr userData, IntPtr dw) =>
        Guard<IWriterCallbackSink>(userData, s => s.OfferedIncompatibleQos());

    [UnmanagedCallersOnly(CallConvs = new[] { typeof(CallConvCdecl) })]
    internal static void WriterPublicationMatched(IntPtr userData, IntPtr dw) =>
        Guard<IWriterCallbackSink>(userData, s => s.PublicationMatched());

    [UnmanagedCallersOnly(CallConvs = new[] { typeof(CallConvCdecl) })]
    internal static void ReaderDataAvailable(IntPtr userData, IntPtr dr) =>
        Guard<IReaderCallbackSink>(userData, s => s.DataAvailable());

    [UnmanagedCallersOnly(CallConvs = new[] { typeof(CallConvCdecl) })]
    internal static void ReaderSampleRejected(IntPtr userData, IntPtr dr) =>
        Guard<IReaderCallbackSink>(userData, s => s.SampleRejected());

    [UnmanagedCallersOnly(CallConvs = new[] { typeof(CallConvCdecl) })]
    internal static void ReaderLivelinessChanged(IntPtr userData, IntPtr dr) =>
        Guard<IReaderCallbackSink>(userData, s => s.LivelinessChanged());

    [UnmanagedCallersOnly(CallConvs = new[] { typeof(CallConvCdecl) })]
    internal static void ReaderRequestedDeadlineMissed(IntPtr userData, IntPtr dr) =>
        Guard<IReaderCallbackSink>(userData, s => s.RequestedDeadlineMissed());

    [UnmanagedCallersOnly(CallConvs = new[] { typeof(CallConvCdecl) })]
    internal static void ReaderRequestedIncompatibleQos(IntPtr userData, IntPtr dr) =>
        Guard<IReaderCallbackSink>(userData, s => s.RequestedIncompatibleQos());

    [UnmanagedCallersOnly(CallConvs = new[] { typeof(CallConvCdecl) })]
    internal static void ReaderSubscriptionMatched(IntPtr userData, IntPtr dr) =>
        Guard<IReaderCallbackSink>(userData, s => s.SubscriptionMatched());

    [UnmanagedCallersOnly(CallConvs = new[] { typeof(CallConvCdecl) })]
    internal static void ReaderSampleLost(IntPtr userData, IntPtr dr) =>
        Guard<IReaderCallbackSink>(userData, s => s.SampleLost());
}

// ---------------------------------------------------------------------------
// Registrations: own the unmanaged vtable and the GCHandle, clear the slot on
// Dispose. Spec §9 memory-ownership table, row by row.
// ---------------------------------------------------------------------------

/// <summary>
/// A bound listener. Disposing it clears the entity's listener slot first and
/// frees the vtable and the handle afterwards, in that order — the registry
/// holds a weak raw pointer, so freeing first would leave it dangling.
/// </summary>
public interface IListenerRegistration : IDisposable
{
    /// <summary>The `void* user_data` the callbacks receive (Spec §1.2).</summary>
    IntPtr UserData { get; }
}

internal abstract unsafe class ListenerRegistration : IListenerRegistration
{
    private GCHandle _sink;
    private IntPtr _vtable;
    private IntPtr _entity;

    private protected ListenerRegistration(object sink, IntPtr entity, int vtableBytes)
    {
        _sink = GCHandle.Alloc(sink);
        _entity = entity;
        _vtable = Marshal.AllocHGlobal(vtableBytes);
    }

    public IntPtr UserData => _sink.IsAllocated ? GCHandle.ToIntPtr(_sink) : IntPtr.Zero;

    private protected IntPtr Vtable => _vtable;

    private protected IntPtr Entity => _entity;

    /// <summary>Calls the entity's `*_set_listener` with a NULL vtable.</summary>
    private protected abstract void ClearSlot();

    public void Dispose()
    {
        if (_vtable != IntPtr.Zero)
        {
            ClearSlot();
            Marshal.FreeHGlobal(_vtable);
            _vtable = IntPtr.Zero;
        }
        if (_sink.IsAllocated)
        {
            _sink.Free();
        }
        _entity = IntPtr.Zero;
        GC.SuppressFinalize(this);
    }
}

internal sealed unsafe class WriterListenerRegistration : ListenerRegistration
{
    internal WriterListenerRegistration(IWriterCallbackSink sink, IntPtr dw, StatusKind statusMask)
        : base(sink, dw, sizeof(Native.DataWriterListenerVTable))
    {
        var vt = new Native.DataWriterListenerVTable
        {
            UserData = UserData,
            OnLivelinessLost =
                (IntPtr)(delegate* unmanaged[Cdecl]<IntPtr, IntPtr, void>)
                    (&ListenerTrampolines.WriterLivelinessLost),
            OnOfferedDeadlineMissed =
                (IntPtr)(delegate* unmanaged[Cdecl]<IntPtr, IntPtr, void>)
                    (&ListenerTrampolines.WriterOfferedDeadlineMissed),
            OnOfferedIncompatibleQos =
                (IntPtr)(delegate* unmanaged[Cdecl]<IntPtr, IntPtr, void>)
                    (&ListenerTrampolines.WriterOfferedIncompatibleQos),
            OnPublicationMatched =
                (IntPtr)(delegate* unmanaged[Cdecl]<IntPtr, IntPtr, void>)
                    (&ListenerTrampolines.WriterPublicationMatched),
        };
        *(Native.DataWriterListenerVTable*)Vtable = vt;
        StatusCheck.Check(Native.DwSetListenerRaw(dw, Vtable, (uint)statusMask),
            "DataWriter::SetListener");
    }

    private protected override void ClearSlot() =>
        Native.DwSetListenerRaw(Entity, IntPtr.Zero, 0);
}

internal sealed unsafe class ReaderListenerRegistration : ListenerRegistration
{
    internal ReaderListenerRegistration(IReaderCallbackSink sink, IntPtr dr, StatusKind statusMask)
        : base(sink, dr, sizeof(Native.DataReaderListenerVTable))
    {
        var vt = new Native.DataReaderListenerVTable
        {
            UserData = UserData,
            OnDataAvailable =
                (IntPtr)(delegate* unmanaged[Cdecl]<IntPtr, IntPtr, void>)
                    (&ListenerTrampolines.ReaderDataAvailable),
            OnSampleRejected =
                (IntPtr)(delegate* unmanaged[Cdecl]<IntPtr, IntPtr, void>)
                    (&ListenerTrampolines.ReaderSampleRejected),
            OnLivelinessChanged =
                (IntPtr)(delegate* unmanaged[Cdecl]<IntPtr, IntPtr, void>)
                    (&ListenerTrampolines.ReaderLivelinessChanged),
            OnRequestedDeadlineMissed =
                (IntPtr)(delegate* unmanaged[Cdecl]<IntPtr, IntPtr, void>)
                    (&ListenerTrampolines.ReaderRequestedDeadlineMissed),
            OnRequestedIncompatibleQos =
                (IntPtr)(delegate* unmanaged[Cdecl]<IntPtr, IntPtr, void>)
                    (&ListenerTrampolines.ReaderRequestedIncompatibleQos),
            OnSubscriptionMatched =
                (IntPtr)(delegate* unmanaged[Cdecl]<IntPtr, IntPtr, void>)
                    (&ListenerTrampolines.ReaderSubscriptionMatched),
            OnSampleLost =
                (IntPtr)(delegate* unmanaged[Cdecl]<IntPtr, IntPtr, void>)
                    (&ListenerTrampolines.ReaderSampleLost),
        };
        *(Native.DataReaderListenerVTable*)Vtable = vt;
        StatusCheck.Check(Native.DrSetListenerRaw(dr, Vtable, (uint)statusMask),
            "DataReader::SetListener");
    }

    private protected override void ClearSlot() =>
        Native.DrSetListenerRaw(Entity, IntPtr.Zero, 0);
}

/// <summary>Binds an <see cref="IDataWriterListener{T}"/> to the C-FFI vtable.</summary>
public static class DataWriterListenerBridge<T>
{
    /// <summary>Binds a listener to a DataWriter. Dispose to unbind.</summary>
    public static IListenerRegistration Attach(DataWriter<T> dw, IDataWriterListener<T> listener,
        StatusKind statusMask = (StatusKind)0xFFFFFFFF)
    {
        if (dw == null)
        {
            throw new ArgumentNullException(nameof(dw));
        }
        if (listener == null)
        {
            throw new ArgumentNullException(nameof(listener));
        }
        return new WriterListenerRegistration(
            new WriterCallbackSink<T>(dw, listener), dw.Handle, statusMask);
    }

    /// <summary>Clears the DataWriter's listener slot (Spec §1.3).</summary>
    public static void Detach(DataWriter<T> dw)
    {
        if (dw == null)
        {
            throw new ArgumentNullException(nameof(dw));
        }
        Native.DwSetListenerRaw(dw.Handle, IntPtr.Zero, 0);
    }
}

/// <summary>Binds an <see cref="IDataReaderListener{T}"/> to the C-FFI vtable.</summary>
public static class DataReaderListenerBridge<T>
{
    /// <summary>Binds a listener to a DataReader. Dispose to unbind.</summary>
    public static IListenerRegistration Attach(DataReader<T> dr, IDataReaderListener<T> listener,
        StatusKind statusMask = (StatusKind)0xFFFFFFFF)
    {
        if (dr == null)
        {
            throw new ArgumentNullException(nameof(dr));
        }
        if (listener == null)
        {
            throw new ArgumentNullException(nameof(listener));
        }
        return new ReaderListenerRegistration(
            new ReaderCallbackSink<T>(dr, listener), dr.Handle, statusMask);
    }

    /// <summary>Clears the DataReader's listener slot (Spec §1.3).</summary>
    public static void Detach(DataReader<T> dr)
    {
        if (dr == null)
        {
            throw new ArgumentNullException(nameof(dr));
        }
        Native.DrSetListenerRaw(dr.Handle, IntPtr.Zero, 0);
    }
}

/// <summary>
/// Caller-driven listener poll (vendor spec §4.1): the callbacks are delivered
/// from here and never synchronously out of a `set_listener`/`write`/`take`
/// call. The caller drives it from its own loop — a .NET timer, or the
/// readiness descriptor `zerodds_data_notify_fd()` returns.
/// </summary>
public static class ListenerPoll
{
    /// <summary>
    /// Polls every registered listener for status-counter deltas and fires the
    /// callbacks whose mask bit is set. Returns the number fired. Threading:
    /// the callbacks run on the calling thread.
    /// </summary>
    public static int PollAll()
    {
        return (int)(uint)Native.PollListeners();
    }
}
