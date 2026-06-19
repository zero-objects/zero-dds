// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Conditions.cs — DDS-PSM-Cxx 1.0 §7.5.10.

using System;
using System.Collections.Generic;
using ZeroDDS.Core;

namespace ZeroDDS.Cond;

/// <summary>Condition (Spec §7.5.10.1) — Basis-Klasse.</summary>
public abstract class Condition
{
    internal IntPtr Handle;
    internal Condition(IntPtr handle) { Handle = handle; }
    /// <summary>Trigger value.</summary>
    public bool TriggerValue => Handle != IntPtr.Zero && Native.ConditionGetTrigger(Handle);
}

/// <summary>GuardCondition (Spec §7.5.10.5).</summary>
public sealed class GuardCondition : Condition, IDisposable
{
    private bool _disposed;
    public GuardCondition() : base(Native.GuardConditionCreate())
    {
        if (Handle == IntPtr.Zero) throw new DdsError("GuardCondition::create failed");
    }

    /// <summary>Sets the trigger value.</summary>
    public void SetTriggerValue(bool v) =>
        StatusCheck.Check(Native.GuardConditionSetTrigger(Handle, v),
            "GuardCondition::SetTriggerValue");

    public void Dispose()
    {
        if (_disposed) return;
        _disposed = true;
        if (Handle != IntPtr.Zero) { Native.GuardConditionDestroy(Handle); Handle = IntPtr.Zero; }
        GC.SuppressFinalize(this);
    }
    ~GuardCondition() { Dispose(); }
}

/// <summary>WaitSet (Spec §7.5.10.2).</summary>
public sealed class WaitSet : IDisposable
{
    private IntPtr _ws;
    private readonly List<Condition> _conditions = new();
    private bool _disposed;

    public WaitSet()
    {
        _ws = Native.WaitSetCreate();
        if (_ws == IntPtr.Zero) throw new DdsError("WaitSet::create failed");
    }

    /// <summary>Attach Condition.</summary>
    public void AttachCondition(Condition c)
    {
        StatusCheck.Check(Native.WaitSetAttach(_ws, c.Handle), "WaitSet::AttachCondition");
        _conditions.Add(c);
    }

    /// <summary>Detach Condition.</summary>
    public void DetachCondition(Condition c)
    {
        StatusCheck.Check(Native.WaitSetDetach(_ws, c.Handle), "WaitSet::DetachCondition");
        _conditions.Remove(c);
    }

    /// <summary>Wait, returns aktive Conditions.</summary>
    public List<Condition> Wait(Duration timeout)
    {
        var buf = new IntPtr[64];
        UIntPtr count;
        unsafe
        {
            fixed (IntPtr* p = buf)
            {
                int rc = Native.WaitSetWait(_ws, (IntPtr)p, (UIntPtr)buf.Length,
                    out count, timeout.Sec, timeout.Nanosec);
                if (rc == Native.Timeout)
                    throw new ZeroDDS.Core.TimeoutException("WaitSet::Wait");
                StatusCheck.Check(rc, "WaitSet::Wait");
            }
        }
        var result = new List<Condition>();
        var n = (int)(uint)count;
        for (int i = 0; i < n; ++i)
        {
            foreach (var c in _conditions)
            {
                if (c.Handle == buf[i]) { result.Add(c); break; }
            }
        }
        return result;
    }

    public void Dispose()
    {
        if (_disposed) return;
        _disposed = true;
        if (_ws != IntPtr.Zero) { Native.WaitSetDestroy(_ws); _ws = IntPtr.Zero; }
        GC.SuppressFinalize(this);
    }
    ~WaitSet() { Dispose(); }
}
