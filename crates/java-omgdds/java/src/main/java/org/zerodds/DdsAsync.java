// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
package org.zerodds;

import org.omg.dds.core.WaitSet;
import org.omg.dds.pub.DataWriter;
import org.omg.dds.sub.DataReader;
import org.omg.dds.sub.ReadCondition;
import org.omg.dds.sub.Sample;
import org.omg.dds.sub.Subscriber;

import java.time.Duration;
import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.Executor;
import java.util.concurrent.ForkJoinPool;
import java.util.concurrent.CompletionException;
import java.util.concurrent.TimeoutException;

/**
 * Idiomatic modern-Java async surface over the synchronous DDS API, returning
 * {@link CompletableFuture}s. Mirrors {@code zerodds.aio} (Python) and
 * {@code Async.cs} (C#): the blocking wait/write calls are lifted onto an
 * {@link Executor} (the common {@link ForkJoinPool} by default) so they compose
 * with {@code CompletableFuture} pipelines without blocking the caller.
 *
 * <p>The wait helpers block on a {@link WaitSet} with a
 * {@link ReadCondition} of the reader (DDS-Java-PSM 1.0 §7.3.7.3,
 * §7.3.7.4), not on a poll loop: a {@code DATA_AVAILABLE} change or a sample
 * arrival releases the wait.
 */
public final class DdsAsync {

    private DdsAsync() {}

    // ---- write ------------------------------------------------------------

    /** Publish a sample; a failed write completes exceptionally (§7.3.2). */
    public static <T> CompletableFuture<Void> writeAsync(DataWriter<T> writer, T sample) {
        return writeAsync(writer, sample, ForkJoinPool.commonPool());
    }

    public static <T> CompletableFuture<Void> writeAsync(DataWriter<T> writer, T sample,
            Executor ex) {
        return CompletableFuture.runAsync(() -> {
            try {
                writer.write(sample);
            } catch (TimeoutException blocked) {
                throw new CompletionException(blocked);
            } catch (org.omg.dds.core.DDSException failed) {
                throw new CompletionException(failed);
            }
        }, ex);
    }

    // ---- wait for data ----------------------------------------------------

    /** Completes with {@code true} as soon as the reader has samples, else {@code false} at timeout. */
    public static <T> CompletableFuture<Boolean> waitForSamplesAsync(DataReader<T> reader,
            Duration timeout) {
        return waitForSamplesAsync(reader, timeout, ForkJoinPool.commonPool());
    }

    public static <T> CompletableFuture<Boolean> waitForSamplesAsync(DataReader<T> reader,
            Duration timeout, Executor ex) {
        return CompletableFuture.supplyAsync(
                () -> hasSamples(reader, timeout), ex);
    }

    // ---- take -------------------------------------------------------------

    /** Waits (up to {@code timeout}) for samples and {@code take()}s them; empty list on timeout. */
    public static <T> CompletableFuture<List<Sample<T>>> takeAsync(DataReader<T> reader,
            Duration timeout) {
        return takeAsync(reader, timeout, ForkJoinPool.commonPool());
    }

    public static <T> CompletableFuture<List<Sample<T>>> takeAsync(DataReader<T> reader,
            Duration timeout, Executor ex) {
        return CompletableFuture.supplyAsync(() -> {
            awaitSamples(reader, timeout);
            return takeAll(reader);
        }, ex);
    }

    /** Waits (up to {@code timeout}) for at least {@code count} samples and takes them. */
    public static <T> CompletableFuture<List<Sample<T>>> takeAsync(DataReader<T> reader,
            int count, Duration timeout) {
        return CompletableFuture.supplyAsync(() -> {
            long deadline = System.nanoTime() + timeout.toNanos();
            while (count(reader) < count && System.nanoTime() < deadline) {
                awaitSamples(reader, timeout);
            }
            return takeAll(reader);
        }, ForkJoinPool.commonPool());
    }

    // ---- the blocking helpers ---------------------------------------------

    private static <T> boolean hasSamples(DataReader<T> reader, Duration timeout) {
        if (count(reader) > 0) {
            return true;
        }
        return awaitSamples(reader, timeout) && count(reader) > 0;
    }

    private static <T> long count(DataReader<T> reader) {
        int total = 0;
        java.util.Iterator<Sample<T>> walk = reader.read();
        while (walk.hasNext()) {
            walk.next();
            total++;
        }
        return total;
    }

    /** §7.3.7.4 — block until the reader's read condition triggers or time is up. */
    private static <T> boolean awaitSamples(DataReader<T> reader, Duration timeout) {
        Subscriber.DataState any = anyState(reader);
        ReadCondition<T> condition = reader.createReadCondition(any);
        WaitSet waitSet = WaitSet.newWaitSet(reader.getEnvironment());
        waitSet.attachCondition(condition);
        try {
            waitSet.waitForConditions(
                    reader.getEnvironment().getSPI().newDuration(
                            timeout.toMillis(), java.util.concurrent.TimeUnit.MILLISECONDS));
            return true;
        } catch (TimeoutException timedOut) {
            return false;
        } finally {
            condition.close();
        }
    }

    private static <T> Subscriber.DataState anyState(DataReader<T> reader) {
        // Every sample state, every view state, every instance state (§7.7.3).
        return DataStates.all(reader.getEnvironment());
    }

    private static <T> List<Sample<T>> takeAll(DataReader<T> reader) {
        List<Sample<T>> out = new ArrayList<>();
        java.util.Iterator<Sample<T>> walk = reader.take();
        while (walk.hasNext()) {
            out.add(walk.next());
        }
        return out;
    }
}