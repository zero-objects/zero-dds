// SPDX-License-Identifier: Apache-2.0
package org.omg.dds.rpc;

/**
 * {@code typedef octet UnusedMember} — DDS-RPC 1.0 §7.5.1.1.1.
 *
 * <p>The type of the `dummy` member an `In` or an `Out` structure carries when the
 * operation gives it nothing else (§7.5.1.1.4 rule 3c, §7.5.1.1.5 rule 3c).
 *
 * <p>A class over one {@code byte} and not a bare {@code byte} field:
 * the generated code names the type, and a name is what makes the octet
 * readable as the placeholder the clause declares rather than as a
 * number somebody forgot to remove. One octet and not an absent member —
 * XCDR gives an empty structure no octets, and a reader cannot tell a
 * sample that arrived from one that did not.
 */
public final class UnusedMember {

    /** The octet on the wire. */
    public byte value;

    public UnusedMember() {}

    public UnusedMember(byte value) {
        this.value = value;
    }
}
