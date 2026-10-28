// SPDX-License-Identifier: Apache-2.0
package org.omg.dds.rpc;

/**
 * {@code typedef octet UnknownOperation} — DDS-RPC 1.0 §7.5.1.1.1.
 *
 * <p>The type of the `unknownOp` member on the `default` case of a `Call` or `Return`
 * union (§7.5.1.1.6 rule 3).
 *
 * <p>A class over one {@code byte} and not a bare {@code byte} field:
 * the generated code names the type, and a name is what makes the octet
 * readable as the placeholder the clause declares rather than as a
 * number somebody forgot to remove. One octet and not an absent member —
 * XCDR gives an empty structure no octets, and a reader cannot tell a
 * sample that arrived from one that did not.
 */
public final class UnknownOperation {

    /** The octet on the wire. */
    public byte value;

    public UnknownOperation() {}

    public UnknownOperation(byte value) {
        this.value = value;
    }
}
