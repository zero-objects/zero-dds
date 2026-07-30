// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
package org.zerodds;

import org.omg.dds.core.ReturnCode;
import org.omg.dds.pub.DataWriter;
import org.omg.dds.sub.DataReader;
import org.omg.dds.sub.Sample;

import java.time.Duration;
import java.util.List;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.Executor;
import java.util.concurrent.ForkJoinPool;

/**
 * Idiomatic modern-Java async surface over the synchronous DDS API, returning
 * {@link CompletableFuture}s. Mirrors {@code zerodds.aio} (Python) and
 * {@code Async.cs} (C#): the blocking wait/write calls are lifted onto an
 * {@link Executor} (the common {@link ForkJoinPool} by default) so they compose
 * with {@code CompletableFuture} pipelines without blocking the caller.
 */
public final class DdsAsync {

    private DdsAsync() {}

    /** Polling granularity for the wait helpers. */
    private static final Duration POLL = Duration.ofMillis(1);

    // ---- write ------------------------------------------------------------

    /** Publish a sample; completes with the {@link ReturnCode}. */
    public static <T> CompletableFuture<ReturnCode> writeAsync(DataWriter<T> writer, T sample) {
        return writeAsync(writer, sample, ForkJoinPool.commonPool());
    }

    public static <T> CompletableFuture<ReturnCode> writeAsync(DataWriter<T> writer, T sample, Executor ex) {
        return CompletableFuture.supplyAsync(() -> writer.write(sample), ex);
    }

    // ---- wait for data ----------------------------------------------------

    /** Completes with {@code true} as soon as the reader has samples, else {@code false} at timeout. */
    public static <T> CompletableFuture<Boolean> waitForSamplesAsync(DataReader<T> reader, Duration timeout) {
        return waitForSamplesAsync(reader, timeout, ForkJoinPool.commonPool());
    }

    public static <T> CompletableFuture<Boolean> waitForSamplesAsync(DataReader<T> reader, Duration timeout, Executor ex) {
        final long deadline = System.nanoTime() + timeout.toNanos();
        return CompletableFuture.supplyAsync(() -> {
            while (System.nanoTime() < deadline) {
                if (!reader.read().isEmpty()) return true;
                sleep(POLL);
            }
            return !reader.read().isEmpty();
        }, ex);
    }

    // ---- take -------------------------------------------------------------

    /** Waits (up to {@code timeout}) for samples and {@code take()}s them; empty list on timeout. */
    public static <T> CompletableFuture<List<Sample<T>>> takeAsync(DataReader<T> reader, Duration timeout) {
        return takeAsync(reader, timeout, ForkJoinPool.commonPool());
    }

    public static <T> CompletableFuture<List<Sample<T>>> takeAsync(DataReader<T> reader, Duration timeout, Executor ex) {
        final long deadline = System.nanoTime() + timeout.toNanos();
        return CompletableFuture.supplyAsync(() -> {
            while (System.nanoTime() < deadline) {
                List<Sample<T>> s = reader.take();
                if (!s.isEmpty()) return s;
                sleep(POLL);
            }
            return reader.take();
        }, ex);
    }

    private static void sleep(Duration d) {
        try {
            Thread.sleep(d.toMillis(), (int) (d.toNanos() % 1_000_000));
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
            throw new RuntimeException(e);
        }
    }
}
