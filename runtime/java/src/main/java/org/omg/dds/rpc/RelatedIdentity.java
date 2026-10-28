// SPDX-License-Identifier: Apache-2.0
package org.omg.dds.rpc;

/**
 * Whether a reply must carry {@code PID_RELATED_SAMPLE_IDENTITY}.
 *
 * <p>DDS-RPC 1.0 §7.8.2: "The PID_RELATED_SAMPLE_IDENTITY shall be used
 * for data samples sent as reply to a request." The Basic Service Mapping
 * additionally carries the identity in the header (§7.8.1), so a DDS
 * binding that cannot put a parameter into the inline QoS of a DATA can
 * still correlate Basic replies.
 */
public enum RelatedIdentity {
    /**
     * The clause as written: every reply carries the parameter. A binding
     * that cannot send it makes the reply fail with
     * {@link RemoteUnsupportedError}.
     */
    REQUIRED,
    /**
     * Send the parameter where the binding can, and rely on the header of
     * the Basic mapping where it cannot. Not available in the Enhanced
     * mapping, which has no header.
     */
    IF_SUPPORTED
}
