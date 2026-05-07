// DDS-RPC 1.0 §7.11.2.2.4 — Future<T> for asynchronous results.
//
// The spec defines `org.omg.dds.rpc.Future<T>` as a sub-interface of
// `java.util.concurrent.Future<T>` plus a FutureCompletionListener-Setter.
// Modern Java provides `CompletableFuture<T>` which already covers both
// the listener pattern (via `whenComplete`) and the standard Future API,
// so we expose Future<T> as a thin alias.
package org.zerodds.rpc;

import java.util.concurrent.CompletableFuture;

/**
 * Type alias for {@link CompletableFuture}. Generated requesters return
 * this type so callers can chain {@code thenApply} / {@code whenComplete}
 * pipelines while remaining compatible with plain {@code java.util.
 * concurrent.Future}.
 */
public final class Future<T> extends CompletableFuture<T> {
    public Future() {
        super();
    }
}
