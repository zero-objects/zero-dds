// SPDX-License-Identifier: Apache-2.0
package org.omg.dds.rpc;

/**
 * The three placeholder types DDS-RPC 1.0 §7.5.1.1.1 declares, and the
 * one label §7.5.1.1.5 fixes.
 *
 * <p>§7.5.1.1.1 writes them as {@code typedef octet UnusedMember},
 * {@code typedef octet UnknownOperation} and {@code typedef octet
 * UnknownException}. Java has no typedef, so each is a {@code byte} and
 * this class carries the width and the reason rather than leaving a
 * bare {@code byte} in a generated field with nothing to say why.
 *
 * <p>They are one octet and not an absent member: XCDR gives an empty
 * structure no octets at all, and a reader cannot tell a sample that
 * arrived from one that did not.
 */
public final class PlaceholderTypes {

    private PlaceholderTypes() {}

    /**
     * The octets a {@code UnusedMember}, {@code UnknownOperation} or
     * {@code UnknownException} member occupies on the wire.
     */
    public static final int PLACEHOLDER_OCTETS = 1;

    /**
     * The label §7.5.1.1.5 rule 3 gives the {@code Result} union's
     * success case. DDS-DCPS gives {@code RETCODE_OK} the value 0.
     */
    public static final int RETCODE_OK = 0;
}
