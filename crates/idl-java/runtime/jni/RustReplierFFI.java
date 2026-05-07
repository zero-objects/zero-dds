// Copyright (c) zerodds contributors
// SPDX-License-Identifier: Apache-2.0
package org.zerodds.rpc;

import org.zerodds.dds.DomainParticipant;
import org.zerodds.dds.jni.RustNativeLibrary;

/**
 * JNI-FFI-Bruecke zum Rust-{@code Replier<RawBytes, RawBytes>}.
 *
 * <p>Stand C6.1.D-stretch: Skeleton — der Rust-Side-Replier nutzt aktuell
 * einen statischen Echo-Handler. Eine Lambda-Bridge mit Java-seitigem
 * {@code (byte[]) -&gt; byte[]}-Handler kommt in der Folge-Iteration
 * (siehe {@code docs/architecture/05_java_jni_bridge.md} §6).
 */
public final class RustReplierFFI implements AutoCloseable {
    static {
        RustNativeLibrary.ensureLoaded();
    }

    private long handle;

    public RustReplierFFI(DomainParticipant participant, String serviceName) {
        this.handle = create0(participant.nativeHandle(), serviceName);
        if (this.handle == 0L) {
            throw new RuntimeException("Replier.create0 returned null for " + serviceName);
        }
    }

    public long nativeHandle() {
        return handle;
    }

    /** Bearbeitet alle pending Requests einmalig. Liefert Anzahl. */
    public int tick() {
        int n = tick0(handle);
        if (n < 0) {
            throw new RuntimeException("Replier.tick0 returned " + n);
        }
        return n;
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
    public static native int tick0(long handle);
}
