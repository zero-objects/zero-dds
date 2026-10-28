// SPDX-License-Identifier: Apache-2.0
package org.omg.dds.rpc;

import java.util.concurrent.CompletableFuture;
import java.util.concurrent.ExecutionException;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.TimeoutException;
import java.util.function.Function;

/** Helpers over {@link Future} for the generated function-call clients. */
public final class Futures {

    private Futures() {}

    /**
     * The future of what {@code unpack} makes of the reply that
     * {@code future} resolves to — §7.11.1.1.2 rule 7: an asynchronous
     * operation returns a future of its result.
     *
     * @param future the future of the reply sample
     * @param unpack the generated unpacking of one reply
     * @return a future that is ready when {@code future} is; if the reply
     *     carried a remote exception or {@code unpack} throws, it fails
     */
    public static <A, B> Future<B> map(
            Future<Sample<A>> future, Function<A, B> unpack) {
        CompletableFuture<B> mapped = new CompletableFuture<>();
        future.withCompletionListener(
                done -> {
                    try {
                        mapped.complete(unpack.apply(done.get().getData()));
                    } catch (ExecutionException failure) {
                        mapped.completeExceptionally(failure.getCause());
                    } catch (InterruptedException interrupted) {
                        Thread.currentThread().interrupt();
                        mapped.completeExceptionally(interrupted);
                    } catch (RuntimeException failure) {
                        mapped.completeExceptionally(failure);
                    }
                });
        return new Future<B>() {
            @Override
            public void withCompletionListener(FutureCompletionListener<B> listener) {
                mapped.whenComplete((value, failure) -> listener.onComplete(this));
            }

            @Override
            public boolean cancel(boolean mayInterruptIfRunning) {
                future.cancel(mayInterruptIfRunning);
                return mapped.cancel(mayInterruptIfRunning);
            }

            @Override
            public boolean isCancelled() {
                return mapped.isCancelled();
            }

            @Override
            public boolean isDone() {
                return mapped.isDone();
            }

            @Override
            public B get() throws InterruptedException, ExecutionException {
                return mapped.get();
            }

            @Override
            public B get(long timeout, TimeUnit unit)
                    throws InterruptedException, ExecutionException, TimeoutException {
                return mapped.get(timeout, unit);
            }
        };
    }
}
