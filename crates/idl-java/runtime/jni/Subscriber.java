// Copyright (c) zerodds contributors
// SPDX-License-Identifier: Apache-2.0
package org.zerodds.dds;

import org.zerodds.dds.jni.RustNativeLibrary;

/** Java-Wrapper fuer einen Rust-{@code Subscriber}-Handle. */
public final class Subscriber implements AutoCloseable {
    static {
        RustNativeLibrary.ensureLoaded();
    }

    private long handle;

    Subscriber(long handle) {
        this.handle = handle;
    }

    public long nativeHandle() {
        return handle;
    }

    public DataReader createDataReader(Topic topic) {
        long h = DataReader.create0(handle, topic.nativeHandle());
        if (h == 0L) {
            throw new RuntimeException("DataReader.create0 returned null handle");
        }
        return new DataReader(h);
    }

    @Override
    public void close() {
        handle = 0L;
    }
}
