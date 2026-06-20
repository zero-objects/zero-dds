// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Async.cs — Idiomatic modern-C# async surface over the synchronous DDS API.
//
// Provides:
//   * Task DataWriter<T>.WriteAsync(T, CancellationToken)
//   * Task<bool> DataReader<T>.WaitForDataAsync(TimeSpan, CancellationToken)
//   * IAsyncEnumerable<Sample<T>> DataReader<T>.TakeAsync(CancellationToken)
//
// The blocking native readiness wait (WaitSet over a not-read ReadCondition,
// see DataReader<T>.WaitForData) is bridged to a Task via Task.Run so callers
// get an awaitable without busy-polling; cancellation is honoured both before
// the wait and between wait cycles, surfacing as OperationCanceledException.

using System;
using System.Collections.Generic;
using System.Runtime.CompilerServices;
using System.Threading;
using System.Threading.Tasks;
using ZeroDDS.Core;
using ZeroDDS.Pub;
using ZeroDDS.Sub;

namespace ZeroDDS.Sub;

/// <summary>Async extensions for <see cref="DataReader{T}"/>.</summary>
public static class DataReaderAsyncExtensions
{
    /// <summary>
    /// Awaitable wait-for-data. Completes <c>true</c> when unread data is
    /// available, <c>false</c> on timeout. Throws
    /// <see cref="OperationCanceledException"/> when <paramref name="ct"/> fires.
    /// </summary>
    public static Task<bool> WaitForDataAsync<T>(this DataReader<T> reader,
        TimeSpan timeout, CancellationToken ct = default)
    {
        ct.ThrowIfCancellationRequested();
        var duration = ZeroDDS.TimeSpanBridge.ToDuration(timeout);
        // Offload the blocking native WaitSet wait; cancellation is honoured
        // mid-wait via a GuardCondition tripped by `ct` (no busy poll), so a
        // long/infinite readiness wait wakes immediately on cancel.
        return Task.Run(() => reader.WaitForData(duration, ct), ct);
    }

    /// <summary>
    /// Awaits readiness then yields each taken <see cref="Sample{T}"/> as an
    /// async stream. Cancellation aborts the wait and the enumeration.
    /// </summary>
    public static async IAsyncEnumerable<Sample<T>> TakeAsync<T>(this DataReader<T> reader,
        TimeSpan timeout, [EnumeratorCancellation] CancellationToken ct = default)
    {
        bool ready = await reader.WaitForDataAsync(timeout, ct).ConfigureAwait(false);
        if (!ready) yield break;

        var samples = reader.Take();
        foreach (var s in samples)
        {
            ct.ThrowIfCancellationRequested();
            yield return s;
        }
    }

    /// <summary>
    /// Convenience <see cref="TakeAsync{T}(DataReader{T}, TimeSpan, CancellationToken)"/>
    /// using an infinite readiness wait (cancellable via <paramref name="ct"/>).
    /// </summary>
    public static IAsyncEnumerable<Sample<T>> TakeAsync<T>(this DataReader<T> reader,
        CancellationToken ct = default) =>
        reader.TakeAsync(System.Threading.Timeout.InfiniteTimeSpan, ct);
}

/// <summary>Async extensions for <see cref="DataWriter{T}"/>.</summary>
public static class DataWriterAsyncExtensions
{
    /// <summary>
    /// Awaitable write. The wire write is synchronous and fast; this wrapper
    /// honours <paramref name="ct"/> and yields a <see cref="Task"/> so it
    /// composes with <c>await</c>.
    /// </summary>
    public static Task WriteAsync<T>(this DataWriter<T> writer, T sample,
        CancellationToken ct = default)
    {
        ct.ThrowIfCancellationRequested();
        try
        {
            writer.Write(sample);
            return Task.CompletedTask;
        }
        catch (OperationCanceledException)
        {
            throw;
        }
        catch (Exception ex)
        {
            return Task.FromException(ex);
        }
    }
}
