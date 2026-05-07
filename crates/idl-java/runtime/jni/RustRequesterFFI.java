// Copyright (c) zerodds contributors
// SPDX-License-Identifier: Apache-2.0
package org.zerodds.rpc;

import java.util.concurrent.CompletableFuture;
import java.util.concurrent.Executors;
import java.util.concurrent.ScheduledExecutorService;
import java.util.concurrent.TimeUnit;

import org.zerodds.dds.DomainParticipant;
import org.zerodds.dds.jni.RustNativeLibrary;

/**
 * JNI-FFI-Bruecke zum Rust-{@code Requester<RawBytes, RawBytes>}.
 *
 * <p>Spec: DDS-RPC 1.0 §7.9 (Requester-API).
 *
 * <p>Async-Pattern: {@link #sendRequestAsync(byte[])} liefert einen
 * {@link CompletableFuture}, der von einem internen ScheduledExecutor
 * gepollt wird (Default 2 ms).
 */
public final class RustRequesterFFI implements AutoCloseable {
    static {
        RustNativeLibrary.ensureLoaded();
    }

    private static final ScheduledExecutorService POLLER =
            Executors.newSingleThreadScheduledExecutor(r -> {
                Thread t = new Thread(r, "zerodds-rpc-poller");
                t.setDaemon(true);
                return t;
            });

    private long handle;

    public RustRequesterFFI(DomainParticipant participant, String serviceName) {
        this.handle = create0(participant.nativeHandle(), serviceName);
        if (this.handle == 0L) {
            throw new RuntimeException("Requester.create0 returned null for " + serviceName);
        }
    }

    public long nativeHandle() {
        return handle;
    }

    /** Synchroner Request mit timeout in Millisekunden (0 = QoS-Default). */
    public byte[] sendRequest(byte[] payload, long timeoutMs) {
        return send_request0(handle, payload, timeoutMs);
    }

    /**
     * Schickt einen Request asynchron. Der zurueckgelieferte
     * {@link CompletableFuture} wird vom Poller-Thread mit dem Reply
     * gefuellt; bei Disconnect / RemoteException wird er
     * exceptional completed.
     */
    public CompletableFuture<byte[]> sendRequestAsync(byte[] payload) {
        long futureHandle = send_request_async0(handle, payload);
        if (futureHandle == 0L) {
            CompletableFuture<byte[]> f = new CompletableFuture<>();
            f.completeExceptionally(new RuntimeException("send_request_async0 returned 0"));
            return f;
        }
        CompletableFuture<byte[]> result = new CompletableFuture<>();
        Runnable poll = new Runnable() {
            @Override
            public void run() {
                if (result.isDone()) {
                    destroy_future0(futureHandle);
                    return;
                }
                try {
                    byte[] reply = poll_future0(futureHandle);
                    if (reply != null) {
                        result.complete(reply);
                        destroy_future0(futureHandle);
                        return;
                    }
                    // tick und reschedule
                    tick0(handle);
                    POLLER.schedule(this, 2, TimeUnit.MILLISECONDS);
                } catch (RuntimeException ex) {
                    result.completeExceptionally(ex);
                    destroy_future0(futureHandle);
                }
            }
        };
        POLLER.schedule(poll, 2, TimeUnit.MILLISECONDS);
        return result;
    }

    @Override
    public void close() {
        if (handle != 0L) {
            destroy0(handle);
            handle = 0L;
        }
    }

    @Override
    @SuppressWarnings("deprecation")
    protected void finalize() {
        close();
    }

    public static native long create0(long participant, String service);
    public static native void destroy0(long handle);
    public static native byte[] send_request0(long handle, byte[] payload, long timeoutMs);
    public static native long send_request_async0(long handle, byte[] payload);
    public static native byte[] poll_future0(long futureHandle);
    public static native void destroy_future0(long futureHandle);
    public static native int tick0(long handle);
}
