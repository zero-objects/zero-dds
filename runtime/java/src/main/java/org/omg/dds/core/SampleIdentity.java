// SPDX-License-Identifier: Apache-2.0
package org.omg.dds.core;

import java.util.UUID;

/**
 * The identity of a sample — DDS-RPC 1.0 §7.5.1.1.1: "SampleIdentity =
 * struct {GUID_t writer_guid; SequenceNumber_t sequence_number;}" — as the
 * machine-readable {@code org/omg/dds/core/SampleIdentity.java} of the
 * specification declares it: an interface in {@code org.omg.dds.core}, with
 * the GUID as a {@link UUID} and the sequence number as a {@code long}.
 *
 * <p>{@code org.omg.dds.rpc.SampleIdentity} is the implementation this
 * binding uses; it also gives the sixteen octets of the GUID, which the
 * wire carries.
 */
public interface SampleIdentity {

    @Override
    boolean equals(Object identity);

    @Override
    int hashCode();

    /** The GUID of the writer, as the UUID of its sixteen octets (big-endian halves). */
    UUID guid();

    /** The sequence number of the sample (high word in the upper 32 bits). */
    long sequenceNumber();
}
