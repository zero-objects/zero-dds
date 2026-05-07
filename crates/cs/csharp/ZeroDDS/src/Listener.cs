// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Listener.cs — DDS-PSM-Cxx 1.0 §7.5.9 + Vendor-Spec
//                `zerodds-listener-callbacks-1.0`.

using System;
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

/// <summary>
/// Bridge DataWriter+IDataWriterListener → C-FFI vtable.
/// vtable wird gesetzt; Active-Wireup via `ListenerPoll.PollAll()`
/// — siehe Vendor-Spec `zerodds-listener-callbacks-1.0` §6.2.
/// </summary>
public static class DataWriterListenerBridge<T>
{
    /// <summary>Bindet einen Listener an einen DataWriter.</summary>
    public static IDisposable Attach(DataWriter<T> dw, IDataWriterListener<T> listener,
        StatusKind statusMask = (StatusKind)0xFFFFFFFF)
    {
        if (listener == null)
        {
            Native.DwSetListenerNull(dw.Handle, IntPtr.Zero, 0);
            return new EmptyDisposable();
        }
        var anchor = new ListenerAnchor<IDataWriterListener<T>>(listener);
        var vt = new Native.DataWriterListenerVTable
        {
            UserData = anchor.Pointer,
            OnLivelinessLost = IntPtr.Zero,
            OnOfferedDeadlineMissed = IntPtr.Zero,
            OnOfferedIncompatibleQos = IntPtr.Zero,
            OnPublicationMatched = IntPtr.Zero,
        };
        StatusCheck.Check(Native.DwSetListener(dw.Handle, ref vt, (uint)statusMask),
            "DataWriter::SetListener");
        return anchor;
    }
}

/// <summary>
/// Bridge DataReader+IDataReaderListener → C-FFI vtable.
/// </summary>
public static class DataReaderListenerBridge<T>
{
    /// <summary>Bindet einen Listener an einen DataReader.</summary>
    public static IDisposable Attach(DataReader<T> dr, IDataReaderListener<T> listener,
        StatusKind statusMask = (StatusKind)0xFFFFFFFF)
    {
        if (listener == null)
        {
            Native.DrSetListenerNull(dr.Handle, IntPtr.Zero, 0);
            return new EmptyDisposable();
        }
        var anchor = new ListenerAnchor<IDataReaderListener<T>>(listener);
        var vt = new Native.DataReaderListenerVTable
        {
            UserData = anchor.Pointer,
            OnDataAvailable = IntPtr.Zero,
            OnSampleRejected = IntPtr.Zero,
            OnLivelinessChanged = IntPtr.Zero,
            OnRequestedDeadlineMissed = IntPtr.Zero,
            OnRequestedIncompatibleQos = IntPtr.Zero,
            OnSubscriptionMatched = IntPtr.Zero,
            OnSampleLost = IntPtr.Zero,
        };
        StatusCheck.Check(Native.DrSetListener(dr.Handle, ref vt, (uint)statusMask),
            "DataReader::SetListener");
        return anchor;
    }
}

internal sealed class ListenerAnchor<T> : IDisposable
{
    private GCHandle _handle;
    public IntPtr Pointer => GCHandle.ToIntPtr(_handle);
    public ListenerAnchor(T listener) { _handle = GCHandle.Alloc(listener); }
    public void Dispose() { if (_handle.IsAllocated) _handle.Free(); }
}

internal sealed class EmptyDisposable : IDisposable
{
    public void Dispose() { }
}

/// <summary>Caller-driven Listener-Poll-API (Spec-Vendor: Spec-konformer
/// Active-Wireup ueber expliziten Poll-Call). Caller ruft das periodisch
/// im Main-Loop. Liefert Anzahl gefeuerter Callbacks zurueck.</summary>
public static class ListenerPoll
{
    /// <summary>Pollt alle registrierten Listener auf Status-Counter-Delta
    /// und feuert die Callbacks. Threading: Callbacks feuern auf dem
    /// Caller-Thread.</summary>
    public static int PollAll()
    {
        return (int)(uint)Native.PollListeners();
    }
}
