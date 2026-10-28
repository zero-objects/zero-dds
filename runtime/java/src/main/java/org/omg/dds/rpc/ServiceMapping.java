// SPDX-License-Identifier: Apache-2.0
package org.omg.dds.rpc;

/**
 * Which of the two conformance profiles of DDS-RPC 1.0 §2 an endpoint
 * speaks.
 *
 * <p>The Basic Service Mapping (§7.5.1.1) puts a header in every sample
 * and correlates by it; the Enhanced Service Mapping (§7.5.1.2) has no
 * header and correlates by the sample identity of the request and
 * {@code PID_RELATED_SAMPLE_IDENTITY} of the reply (§7.8.2). The two do not
 * interoperate on one topic pair, so an endpoint names the one it speaks.
 */
public enum ServiceMapping {
    /** §7.5.1.1: {@code RequestHeader}/{@code ReplyHeader} in the sample. */
    BASIC,
    /** §7.5.1.2: no header; identity and related identity carry the correlation. */
    ENHANCED
}
