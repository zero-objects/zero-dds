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
}
