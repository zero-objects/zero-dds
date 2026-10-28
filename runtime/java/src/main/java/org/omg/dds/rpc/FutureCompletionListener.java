// SPDX-License-Identifier: Apache-2.0
package org.omg.dds.rpc;

/**
 * DDS-RPC 1.0 §7.11.2.2.5: "The FutureCompletionListener allows callback
 * notification when the corresponding future becomes ready."
 *
 * @param <T> what the future resolves to
 */
public interface FutureCompletionListener<T> extends java.util.EventListener {

    /** Called when {@code future} is ready: DDS-RPC 1.0 §7.11.2.2.5, {@code onComplete}. */
    void onComplete(java.util.concurrent.Future<T> future);

    /** The earlier name of {@link #onComplete}. */
    default void onFutureComplete(Future<T> future) {
        onComplete(future);
    }
}
