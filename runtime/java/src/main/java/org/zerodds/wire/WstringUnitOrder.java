// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
package org.zerodds.wire;

/**
 * In what byte order a {@code wstring}'s code units go out.
 *
 * <p>A third axis, because CORBA makes it one: omniORB and JacORB decode a
 * {@code wstring}'s units as UTF-16 big-endian whatever the stream says, and
 * omniORB writes a byte-order mark so that its own reader knows.
 *
 * <p>Mirrors {@code zerodds_ast_layout::WstringUnitOrder} and
 * {@code zdw_wstring_unit_order}.
 */
public enum WstringUnitOrder {
    /** The stream's own order. XTypes 1.3 §7.4.1.1.2 through ESWAP. */
    STREAM("stream"),
    /**
     * UTF-16 big-endian whatever the stream says, and no mark. What omniORB
     * and JacORB decode in the absence of one.
     */
    BIG_ENDIAN_ALWAYS("big-endian-always"),
    /**
     * A byte-order mark, then the units, both little-endian. omniORB. The
     * mark's two octets are counted in the length and it is not a character of
     * the string. On read the mark decides: {@code FEFF} means the units
     * follow in the order it was read in, {@code FFFE} means they are the
     * other way round, and no mark at all means big-endian.
     */
    BYTE_ORDER_MARK("byte-order-mark"),
    /**
     * The units always little-endian, whatever the stream says, and no mark.
     *
     * <p>This reproduces a peer misreading its own output, and it is on this
     * axis because it is a byte order. Cyclone swaps a {@code wstring}'s
     * length word for a big-endian stream and does not swap its content, so
     * its big-endian {@code "ab"} is {@code 00000004 61006200} where RTI and
     * Fast DDS write {@code 00000004 00610062}. Never a default and no dialect
     * selects it; without it nothing can exchange a {@code wstring} with
     * Cyclone over big endian. In little endian it is {@link #STREAM} byte for
     * byte.
     */
    HOST_UNSWAPPED("host-unswapped");

    private final String name;

    WstringUnitOrder(String name) {
        this.name = name;
    }

    /** The name {@code WstringUnitOrder::as_str} gives. */
    public String asString() {
        return name;
    }

    /**
     * Whether a code unit goes out little-endian whatever the stream's order.
     *
     * <p>True for {@link #BYTE_ORDER_MARK} and {@link #HOST_UNSWAPPED}, which
     * is the one thing the unit writers ask of this axis besides
     * {@link #BIG_ENDIAN_ALWAYS}.
     */
    public boolean isFixedLittle() {
        return this == BYTE_ORDER_MARK || this == HOST_UNSWAPPED;
    }

    /** The named order, or {@code null} for anything else. */
    public static WstringUnitOrder parse(String text) {
        for (WstringUnitOrder order : values()) {
            if (order.name.equals(text)) {
                return order;
            }
        }
        return null;
    }
}
