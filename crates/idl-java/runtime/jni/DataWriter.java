// Copyright (c) zerodds contributors
// SPDX-License-Identifier: Apache-2.0
package org.zerodds.dds;

import org.zerodds.dds.jni.RustNativeLibrary;

/** Java-Wrapper fuer Rust-{@code DataWriter<RawBytes>}. */
public final class DataWriter implements AutoCloseable {
    static {
        RustNativeLibrary.ensureLoaded();
    }

    private long handle;

    DataWriter(long handle) {
        this.handle = handle;
    }

    public long nativeHandle() {
        return handle;
    }

    /** Schreibt rohe Bytes (XCDR2-encodiert vom Java-Codegen). */
    public void write(byte[] payload) {
        int rc = write0(handle, payload);
        if (rc != 0) {
            throw new RuntimeException("DataWriter.write0 failed rc=" + rc);
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

    static native long create0(long publisher, long topic);
    private static native void destroy0(long handle);
    private static native int write0(long handle, byte[] payload);
}
