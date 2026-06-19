// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Core.cs — DDS-PSM-Cxx 1.0 §7.5 core types ported to C#.

using System;

namespace ZeroDDS.Core;

/// <summary>Time (Spec §7.5.2).</summary>
public readonly struct Time : IEquatable<Time>
{
    public int Sec { get; }
    public uint Nanosec { get; }
    public Time(int sec, uint nanosec) { Sec = sec; Nanosec = nanosec; }
    public static Time Invalid => new(-1, 0xFFFFFFFFu);
    public bool Equals(Time other) => Sec == other.Sec && Nanosec == other.Nanosec;
    public override bool Equals(object? obj) => obj is Time t && Equals(t);
    public override int GetHashCode() => HashCode.Combine(Sec, Nanosec);
    public static bool operator ==(Time a, Time b) => a.Equals(b);
    public static bool operator !=(Time a, Time b) => !a.Equals(b);
}

/// <summary>Duration (Spec §7.5.3).</summary>
public readonly struct Duration : IEquatable<Duration>
{
    public int Sec { get; }
    public uint Nanosec { get; }
    public Duration(int sec, uint nanosec) { Sec = sec; Nanosec = nanosec; }
    public static Duration Zero => new(0, 0);
    public static Duration Infinite => new(int.MaxValue, 0xFFFFFFFFu);
    public static Duration FromMillis(long ms)
    {
        var s = (int)(ms / 1000);
        var rem = (int)(ms % 1000);
        if (rem < 0) { rem += 1000; s -= 1; }
        return new Duration(s, (uint)rem * 1_000_000u);
    }
    public static Duration FromSeconds(int s) => new(s, 0);
    public bool IsInfinite => Sec == int.MaxValue && Nanosec == 0xFFFFFFFFu;
    public ulong TotalMilliseconds => (ulong)Sec * 1000UL + Nanosec / 1_000_000UL;
    public bool Equals(Duration other) => Sec == other.Sec && Nanosec == other.Nanosec;
    public override bool Equals(object? obj) => obj is Duration d && Equals(d);
    public override int GetHashCode() => HashCode.Combine(Sec, Nanosec);
    public static bool operator ==(Duration a, Duration b) => a.Equals(b);
    public static bool operator !=(Duration a, Duration b) => !a.Equals(b);
}

/// <summary>InstanceHandle (Spec §7.5.4).</summary>
public readonly struct InstanceHandle : IEquatable<InstanceHandle>
{
    public ulong Value { get; }
    public InstanceHandle(ulong value) { Value = value; }
    public static InstanceHandle Nil => new(0);
    public bool IsNil => Value == 0;
    public bool Equals(InstanceHandle other) => Value == other.Value;
    public override bool Equals(object? obj) => obj is InstanceHandle h && Equals(h);
    public override int GetHashCode() => Value.GetHashCode();
    public static bool operator ==(InstanceHandle a, InstanceHandle b) => a.Equals(b);
    public static bool operator !=(InstanceHandle a, InstanceHandle b) => !a.Equals(b);
}

/// <summary>Base exception (Spec §7.5.1).</summary>
public class DdsException : Exception
{
    public DdsException(string msg) : base(msg) { }
}

/// <summary>Reads but reader/writer is closed.</summary>
public class AlreadyClosedException : DdsException
{
    public AlreadyClosedException(string msg) : base(msg) { }
}

/// <summary>Generic error.</summary>
public class DdsError : DdsException
{
    public DdsError(string msg) : base(msg) { }
}

/// <summary>Argument check failed.</summary>
public class IllegalOperationException : DdsException
{
    public IllegalOperationException(string msg) : base(msg) { }
}

/// <summary>Resource limit reached.</summary>
public class OutOfResourcesException : DdsException
{
    public OutOfResourcesException(string msg) : base(msg) { }
}

/// <summary>Operation not supported.</summary>
public class UnsupportedException : DdsException
{
    public UnsupportedException(string msg) : base(msg) { }
}

/// <summary>Pre-condition violation.</summary>
public class PreconditionNotMetException : DdsException
{
    public PreconditionNotMetException(string msg) : base(msg) { }
}

/// <summary>Bad parameter.</summary>
public class InvalidArgumentException : DdsException
{
    public InvalidArgumentException(string msg) : base(msg) { }
}

/// <summary>Immutable QoS modified.</summary>
public class ImmutablePolicyException : DdsException
{
    public ImmutablePolicyException(string msg) : base(msg) { }
}

/// <summary>Inconsistent QoS.</summary>
public class InconsistentPolicyException : DdsException
{
    public InconsistentPolicyException(string msg) : base(msg) { }
}

/// <summary>Timeout.</summary>
public class TimeoutException : DdsException
{
    public TimeoutException(string msg) : base(msg) { }
}

/// <summary>Entity not enabled.</summary>
public class NotEnabledException : DdsException
{
    public NotEnabledException(string msg) : base(msg) { }
}

internal static class StatusCheck
{
    public static void Check(int code, string what)
    {
        switch (code)
        {
            case Native.Ok: return;
            case Native.NoData: return;
            case Native.Error: throw new DdsError(what);
            case Native.BadHandle: throw new AlreadyClosedException(what);
            case Native.InvalidUtf8: throw new InvalidArgumentException(what);
            case Native.Timeout: throw new ZeroDDS.Core.TimeoutException(what);
            case Native.PreconditionNotMet: throw new PreconditionNotMetException(what);
            case Native.BadParameter: throw new InvalidArgumentException(what);
            case Native.OutOfResources: throw new OutOfResourcesException(what);
            case Native.NotEnabled: throw new NotEnabledException(what);
            case Native.ImmutablePolicy: throw new ImmutablePolicyException(what);
            case Native.InconsistentPolicy: throw new InconsistentPolicyException(what);
            case Native.AlreadyDeleted: throw new AlreadyClosedException(what);
            case Native.Unsupported: throw new UnsupportedException(what);
            case Native.IllegalOperation: throw new IllegalOperationException(what);
            default: throw new DdsError($"{what} (code={code})");
        }
    }
}
