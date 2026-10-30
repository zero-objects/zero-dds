// SPDX-License-Identifier: Apache-2.0
package org.omg.dds.rpc;

import java.util.concurrent.CompletableFuture;
import java.util.concurrent.ExecutionException;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.TimeoutException;

/**
 * DDS-RPC 1.0 §7.11.2.2.4: "The {@code rpc.Future<T>} interface extends the
 * {@code java.util.concurrent.Future<T>} interface and adds a single operation
 * to specify a FutureCompletionListener."
 *
 * @param <T> what the future resolves to
 */
public interface Future<T> extends java.util.concurrent.Future<T> {

    /** The listener §7.11.2.2.5 notifies when this becomes ready. */
    void withCompletionListener(FutureCompletionListener<T> listener);

    /** The earlier name of {@link #withCompletionListener}. */
    default void setListener(FutureCompletionListener<T> listener) {
        withCompletionListener(listener);
    }

    /**
     * The future over {@code completable} whose completion listener is called on
     * {@code executor}.
     */
    static <T> Future<T> over(CompletableFuture<T> completable, java.util.concurrent.Executor executor) {
        return new Future<T>() {
            @Override
            public void withCompletionListener(FutureCompletionListener<T> listener) {
                completable.whenCompleteAsync((value, failure) -> listener.onComplete(this), executor);
            }

            @Override
            public boolean cancel(boolean mayInterruptIfRunning) {
                return completable.cancel(mayInterruptIfRunning);
            }

            @Override
            public boolean isCancelled() {
                return completable.isCancelled();
            }

            @Override
            public boolean isDone() {
                return completable.isDone();
            }

            @Override
            public T get() throws InterruptedException, ExecutionException {
                return completable.get();
            }

            @Override
            public T get(long timeout, TimeUnit unit)
                    throws InterruptedException, ExecutionException, TimeoutException {
                return completable.get(timeout, unit);
            }
        };
    }

    /** A future that is already ready with {@code value}. */
    static <T> Future<T> completed(T value) {
        return over(CompletableFuture.completedFuture(value));
    }

    /** A future that is already ready with a failure. */
    static <T> Future<T> failed(Throwable cause) {
        CompletableFuture<T> completable = new CompletableFuture<>();
        completable.completeExceptionally(cause);
        return over(completable);
    }

    /** An {@code rpc.Future} over a {@link CompletableFuture}. */
    static <T> Future<T> over(CompletableFuture<T> completable) {
        return new Future<T>() {
            @Override
            public void withCompletionListener(FutureCompletionListener<T> listener) {
                completable.whenComplete((value, failure) -> listener.onComplete(this));
            }

            @Override
            public boolean cancel(boolean mayInterruptIfRunning) {
                return completable.cancel(mayInterruptIfRunning);
            }

            @Override
            public boolean isCancelled() {
                return completable.isCancelled();
            }

            @Override
            public boolean isDone() {
                return completable.isDone();
            }

            @Override
            public T get() throws InterruptedException, ExecutionException {
                return completable.get();
            }

            @Override
            public T get(long timeout, TimeUnit unit)
                    throws InterruptedException, ExecutionException, TimeoutException {
                return completable.get(timeout, unit);
            }
        };
    }
}
