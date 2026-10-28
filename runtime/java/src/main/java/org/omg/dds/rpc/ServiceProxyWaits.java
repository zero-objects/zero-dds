// SPDX-License-Identifier: Apache-2.0
package org.omg.dds.rpc;

import java.util.concurrent.CompletableFuture;

/** The background wait behind the asynchronous waits of {@link ServiceProxy} (no private interface methods before Java 9). */
final class ServiceProxyWaits {
    private ServiceProxyWaits() {
    }

    static Future<Void> later(java.util.function.BooleanSupplier wait) {
        CompletableFuture<Void> done = new CompletableFuture<>();
        Thread waiter =
                new Thread(
                        () -> {
                            try {
                                wait.getAsBoolean();
                                done.complete(null);
                            } catch (RuntimeException failure) {
                                done.completeExceptionally(failure);
                            }
                        },
                        "rpc-wait-for-service");
        waiter.setDaemon(true);
        waiter.start();
        return Future.over(done);
    }
}
