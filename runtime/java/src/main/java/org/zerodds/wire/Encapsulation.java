// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
package org.zerodds.wire;

/**
 * Encodings, and the four bytes that announce one.
 *
 * <p>DDSI-RTPS 2.5 §10.5 and XTypes 1.3 §7.4.1.1. Kept out of {@link Writer}
 * because it frames a sample rather than being part of one: a KeyHash holder
 * is written by the same writer and carries no header at all.
 */
public final class Encapsulation {

    private Encapsulation() {
    }

    /**
     * The four-byte encapsulation header.
     *
     * <p>The identifier itself is always big-endian on the wire, whatever the
     * payload behind it is — it is what tells a reader which order to switch
     * to.
     *
     * <p>The two option bytes carry the payload's trailing padding count in
     * the low two bits of the second one. XTypes 1.3 §7.6.3.1.2: those bits
     * "shall be interpreted by the receiver to determine where the serialized
     * data exactly ended". Measured on the loopback wire on 2026-08-09:
     * Cyclone, RTI and Fast DDS all pad the payload to four and all three
     * state the count — {@code @final struct One { octet a; }} goes out as
     * {@code 0001 0003 ab000000} from every one of them.
     *
     * @param encoding which encoding the payload is in
     * @param endianness which order the payload is in
     * @param padding how many bytes of padding follow the payload, 0 to 3
     * @return the four bytes
     */
    public static byte[] header(Encoding encoding, Endianness endianness, int padding) {
        int identifier = identifier(encoding, endianness);
        return new byte[] {
            (byte) (identifier >> 8),
            (byte) (identifier & 0xFF),
            0,
            (byte) (padding & 0x3),
        };
    }

    /**
     * How many zero bytes bring {@code length} up to the next multiple of four.
     *
     * <p>XTypes 1.3 §7.4.1.1 pads the serialized data to a four-byte boundary;
     * §7.6.3.1.2 states the count in the encapsulation options.
     */
    public static int padding(int length) {
        return (4 - (length & 3)) & 3;
    }

    /** The two-byte encapsulation identifier, as an unsigned number. */
    public static int identifier(Encoding encoding, Endianness endianness) {
        boolean little = endianness.isLittle();
        switch (encoding) {
            case PLAIN_CDR1:
                return little ? 0x0001 : 0x0000;
            case PL_CDR1:
                return little ? 0x0003 : 0x0002;
            case PLAIN_CDR2:
                return little ? 0x0007 : 0x0006;
            case DELIMITED_CDR:
                return little ? 0x0009 : 0x0008;
            case PL_CDR2:
                return little ? 0x000B : 0x000A;
            default:
                throw new IllegalArgumentException("an encoding, not " + encoding);
        }
    }
}
