// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// SubConditions.cs — ReadCondition + QueryCondition (Spec §7.5.10.6/7).

using System;
using System.Runtime.InteropServices;
using ZeroDDS.Cond;
using ZeroDDS.Core;
using ZeroDDS.Sub;

namespace ZeroDDS.SubCond;

/// <summary>ReadCondition&lt;T&gt; (Spec §7.5.10.6).</summary>
public sealed class ReadCondition<T> : Condition, IDisposable
{
    private IntPtr _reader;
    private bool _disposed;

    public ReadCondition(DataReader<T> dr, uint sampleStates, uint viewStates, uint instanceStates)
        : base(IntPtr.Zero)
    {
        Handle = Native.DrCreateReadCondition(dr.Handle, sampleStates, viewStates, instanceStates);
        if (Handle == IntPtr.Zero) throw new DdsError("ReadCondition::create failed");
        _reader = dr.Handle;
    }

    public void Dispose()
    {
        if (_disposed) return;
        _disposed = true;
        if (Handle != IntPtr.Zero)
        {
            Native.DrDeleteReadCondition(_reader, Handle);
            Handle = IntPtr.Zero;
        }
        GC.SuppressFinalize(this);
    }
    ~ReadCondition() { Dispose(); }
}

/// <summary>QueryCondition&lt;T&gt; (Spec §7.5.10.7).</summary>
public sealed class QueryCondition<T> : Condition, IDisposable
{
    private IntPtr _reader;
    private bool _disposed;

    public QueryCondition(DataReader<T> dr, uint sampleStates, uint viewStates,
        uint instanceStates, string expression, string[] parameters)
        : base(IntPtr.Zero)
    {
        IntPtr paramPtr = IntPtr.Zero;
        IntPtr[]? paramHandles = null;
        try
        {
            if (parameters != null && parameters.Length > 0)
            {
                paramHandles = new IntPtr[parameters.Length];
                for (int i = 0; i < parameters.Length; ++i)
                {
                    paramHandles[i] = Marshal.StringToHGlobalAnsi(parameters[i]);
                }
                paramPtr = Marshal.AllocHGlobal(IntPtr.Size * parameters.Length);
                Marshal.Copy(paramHandles, 0, paramPtr, parameters.Length);
            }
            Handle = Native.DrCreateQueryCondition(dr.Handle, sampleStates, viewStates,
                instanceStates, expression, paramPtr,
                (UIntPtr)(parameters?.Length ?? 0));
            if (Handle == IntPtr.Zero) throw new DdsError("QueryCondition::create failed");
            _reader = dr.Handle;
        }
        finally
        {
            if (paramPtr != IntPtr.Zero) Marshal.FreeHGlobal(paramPtr);
            if (paramHandles != null)
            {
                foreach (var h in paramHandles)
                {
                    if (h != IntPtr.Zero) Marshal.FreeHGlobal(h);
                }
            }
        }
    }

    public void Dispose()
    {
        if (_disposed) return;
        _disposed = true;
        if (Handle != IntPtr.Zero)
        {
            Native.DrDeleteReadCondition(_reader, Handle);
            Handle = IntPtr.Zero;
        }
        GC.SuppressFinalize(this);
    }
    ~QueryCondition() { Dispose(); }
}
