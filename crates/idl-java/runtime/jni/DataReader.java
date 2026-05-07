// Copyright (c) zerodds contributors
// SPDX-License-Identifier: Apache-2.0
package org.zerodds.dds;

import org.zerodds.dds.jni.RustNativeLibrary;

/** Java-Wrapper fuer Rust-{@code DataReader<RawBytes>}. */
public final class DataReader implements AutoCloseable {
    static {
        RustNativeLibrary.ensureLoaded();
    }

    private long handle;

    DataReader(long handle) {
        this.handle = handle;
    }

    public long nativeHandle() {
        return handle;
    }

    /**
     * Liefert alle pending Samples als 2D-byte-Array.
     * Java-Codegen deserialisiert die einzelnen Samples per XCDR2.
     */
    public byte[][] take() {
        byte[][] arr = take0(handle);
        if (arr == null) {
            return new byte[0][];
        }
        return arr;
    }

    /** Test-Helper — pumpt Bytes direkt in die Reader-Inbox. */
    public void pushRaw(byte[] payload) {
        int rc = push_raw0(handle, payload);
        if (rc != 0) {
            throw new RuntimeException("DataReader.push_raw0 failed rc=" + rc);
        }
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

    static native long create0(long subscriber, long topic);
    private static native void destroy0(long handle);
    private static native byte[][] take0(long handle);
    private static native int push_raw0(long handle, byte[] payload);
}
