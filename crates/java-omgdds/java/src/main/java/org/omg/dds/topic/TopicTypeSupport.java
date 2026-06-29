// SPDX-License-Identifier: Apache-2.0
package org.omg.dds.topic;

import java.nio.ByteBuffer;

/**
 * Type-Support Trait for Topic-Data-Types — analogous to the C++/Java
 * IDL-generated {@code TypeSupport} class.
 *
 * <p>Caller implements this for each domain type. Generated code (from
 * {@code crates/idl-java}) produces these implementations automatically;
 * hand-written types implement them directly.
 */
public interface TopicTypeSupport<T> {
    /** Spec — fully-qualified type name (used for type-discovery). */
    String getTypeName();

    /** Serialize {@code value} to XCDR2 bytes, appending into {@code buf}. */
    void serialize(T value, ByteBuffer buf);

    /** Deserialize from XCDR2 bytes. */
    T deserialize(ByteBuffer buf);

    /**
     * Whether this type carries one or more {@code @key} members — DDS-DCPS
     * 1.4 §2.2.1.2.2 (keyed Topics define instances). The default is
     * {@code false} (an un-keyed type forms a single instance), so existing
     * hand-written supports need not change.
     */
    default boolean isKeyed() {
        return false;
    }

    /**
     * The instance key bytes for {@code value} — DDS-DCPS 1.4 §2.2.1.2.2.
     * Two samples with equal key bytes belong to the same instance; the
     * runtime uses this for per-instance HISTORY depth, EXCLUSIVE OWNERSHIP
     * arbitration, and keyed lifecycle (dispose/unregister) tracking.
     *
     * <p>Default (un-keyed types) returns an empty array, i.e. one global
     * instance. Keyed types override to serialize only their key members.
     */
    default byte[] keyHash(T value) {
        return new byte[0];
    }
}
