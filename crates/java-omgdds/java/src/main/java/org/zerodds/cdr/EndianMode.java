// SPDX-License-Identifier: Apache-2.0
package org.zerodds.cdr;

/**
 * Endianness for XCDR2 encoding.
 *
 * <p>Spec zerodds-xcdr2-bindings-conformance-1.0 §3: Default ist
 * {@code LITTLE_ENDIAN} (PLAIN_CDR2 LE Encapsulation
 * {@code 0x00 0x01 0x00 0x00}). {@code BIG_ENDIAN} for key-hash
 * PlainCdr2BeKeyHolder (XTypes §7.6.8).
 */
public enum EndianMode {
    /** Little-endian (default for wire encoding). */
    LITTLE_ENDIAN,
    /** Big-endian (for key hash computation). */
    BIG_ENDIAN
}
