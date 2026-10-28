// SPDX-License-Identifier: Apache-2.0
package org.omg.dds.rpc;

/**
 * The CDR encoding of the Basic Service Mapping's samples.
 *
 * <p>DDS-RPC 1.0 §7.5.1.1: the Basic mapping "maps interfaces to data-types
 * that can be used by DDS version 1.3 compliant implementations, which may
 * lack support for the [DDS-XTypes] specification", so its default is the
 * classic CDR of DDS 1.3 (XCDR1). The headers encode identically in both;
 * members of eight octets align to 8 (XCDR1) or 4 (XCDR2), counted from the
 * start of the sample.
 */
public enum WireEncoding {
    /** Classic CDR; MAXALIGN 8. The default. */
    XCDR1(8),
    /** XCDR2 (XTypes 1.3 §7.4); MAXALIGN 4. The named alternative. */
    XCDR2(4);

    private final int maxAlign;

    WireEncoding(int maxAlign) {
        this.maxAlign = maxAlign;
    }

    /** The largest alignment this encoding applies. */
    public int maxAlign() {
        return maxAlign;
    }
}
