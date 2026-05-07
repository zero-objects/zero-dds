// Copyright (c) zerodds contributors
// SPDX-License-Identifier: Apache-2.0
package org.zerodds.dds;

import org.zerodds.dds.jni.RustNativeLibrary;

/**
 * Java-Wrapper fuer den Rust-Side {@code dds_dcps::DomainParticipant}.
 *
 * <p>Lifecycle:
 * <ul>
 *   <li>{@link #DomainParticipant(int)} → Rust {@code Box::into_raw}.</li>
 *   <li>{@link #close()} → Rust {@code Box::from_raw} + drop.</li>
 *   <li>{@link #finalize()} ist Best-Effort-Backstop.</li>
 * </ul>
 *
 * <p>Spec: DDS DCPS 1.4 §2.2.2.2, Java-PSM 1.0 §7.4.
 */
public final class DomainParticipant implements AutoCloseable {
    static {
        RustNativeLibrary.ensureLoaded();
    }

    private long handle;

    public DomainParticipant(int domainId) {
        this.handle = create0(domainId);
        if (this.handle == 0L) {
            throw new RuntimeException("DomainParticipant.create0 returned null handle");
        }
    }

    public long nativeHandle() {
        return handle;
    }

    /**
     * Erzeugt ein {@code Topic<TIn>} mit dem gegebenen Topic- und
     * Type-Namen. Java-Side haelt den Type-Namen, der Rust-Side
     * verwendet ihn als Identifier.
     */
    public Topic createTopic(String name, String typeName) {
        long h = create_topic0(handle, name, typeName);
        if (h == 0L) {
            throw new RuntimeException("create_topic0 returned null handle for " + name);
        }
        return new Topic(h, name, typeName);
    }

    public Publisher createPublisher() {
        long h = create_publisher0(handle);
        if (h == 0L) {
            throw new RuntimeException("create_publisher0 returned null handle");
        }
        return new Publisher(h);
    }

    public Subscriber createSubscriber() {
        long h = create_subscriber0(handle);
        if (h == 0L) {
            throw new RuntimeException("create_subscriber0 returned null handle");
        }
        return new Subscriber(h);
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
        // Best-Effort-Backstop falls close() vergessen wurde.
        close();
    }

    private static native long create0(int domainId);
    private static native void destroy0(long handle);
    private static native long create_topic0(long handle, String name, String typeName);
    private static native long create_publisher0(long handle);
    private static native long create_subscriber0(long handle);
}
