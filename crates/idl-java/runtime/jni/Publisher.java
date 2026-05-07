// Copyright (c) zerodds contributors
// SPDX-License-Identifier: Apache-2.0
package org.zerodds.dds;

import org.zerodds.dds.jni.RustNativeLibrary;

/** Java-Wrapper fuer einen Rust-{@code Publisher}-Handle. */
public final class Publisher implements AutoCloseable {
    static {
        RustNativeLibrary.ensureLoaded();
    }

    private long handle;

    Publisher(long handle) {
        this.handle = handle;
    }

    public long nativeHandle() {
        return handle;
    }

    public DataWriter createDataWriter(Topic topic) {
        long h = DataWriter.create0(handle, topic.nativeHandle());
        if (h == 0L) {
            throw new RuntimeException("DataWriter.create0 returned null handle");
        }
        return new DataWriter(h);
    }

    @Override
    public void close() {
        // Publisher hat in der aktuellen JNI-Schicht keinen
        // dedizierten destroy0-Pfad — die Lifetime ist an den
        // Participant gekoppelt. Wir setzen den Handle nur auf 0,
        // damit createDataWriter nicht weiter aufgerufen wird.
        handle = 0L;
    }
}
