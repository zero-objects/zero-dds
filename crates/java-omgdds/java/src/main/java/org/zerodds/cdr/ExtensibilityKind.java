// SPDX-License-Identifier: Apache-2.0
package org.zerodds.cdr;

/**
 * Extensibility category of an IDL struct per OMG XTypes 1.3
 * §7.2.2.4.4.
 *
 * <p>Bestimmt das Wire-Layout:
 * <ul>
 *   <li>{@link #FINAL}: PLAIN_CDR2 without DHEADER.</li>
 *   <li>{@link #APPENDABLE}: DELIMITED_CDR2 with DHEADER (object-size).</li>
 *   <li>{@link #MUTABLE}: PL_CDR2 with DHEADER + EMHEADER per member.</li>
 * </ul>
 */
public enum ExtensibilityKind {
    /** PLAIN_CDR2 — no DHEADER. */
    FINAL,
    /** DELIMITED_CDR2 — DHEADER before body. */
    APPENDABLE,
    /** PL_CDR2 — EMHEADER pro Member. */
    MUTABLE
}
