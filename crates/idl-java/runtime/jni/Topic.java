// Copyright (c) zerodds contributors
// SPDX-License-Identifier: Apache-2.0
package org.zerodds.dds;

import org.zerodds.dds.jni.RustNativeLibrary;

/**
 * Java-Wrapper fuer einen Rust-{@code Topic<RawBytes>}-Handle.
 *
 * <p>Spec: DDS DCPS 1.4 §2.2.2.3, Java-PSM 1.0 §7.5.
 */
public final class Topic implements AutoCloseable {
    static {
        RustNativeLibrary.ensureLoaded();
    }

    private long handle;
    private final String name;
    private final String typeName;

    /** Package-private — wird von DomainParticipant.createTopic gerufen. */
    Topic(long handle, String name, String typeName) {
        this.handle = handle;
        this.name = name;
        this.typeName = typeName;
    }

    public long nativeHandle() {
        return handle;
    }

    public String getName() {
        return name;
    }

    public String getTypeName() {
        return typeName;
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

    private static native void destroy0(long handle);
    /** Topic-Name aus Rust — fuer cross-validation Tests. */
    private static native String name0(long handle);
    /** Type-Name als UTF-8-Bytes (siehe Doc in Rust-Side). */
    private static native byte[] type_name0(long handle);
}
