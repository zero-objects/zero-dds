// SPDX-License-Identifier: Apache-2.0
package org.zerodds.cdr;

/**
 * Extensibility-Kategorie eines IDL-Struct gemaess OMG XTypes 1.3
 * §7.2.2.4.4.
 *
 * <p>Bestimmt das Wire-Layout:
 * <ul>
 *   <li>{@link #FINAL}: PLAIN_CDR2 ohne DHEADER.</li>
 *   <li>{@link #APPENDABLE}: DELIMITED_CDR2 mit DHEADER (object-size).</li>
 *   <li>{@link #MUTABLE}: PL_CDR2 mit DHEADER + EMHEADER pro Member.</li>
 * </ul>
 */
public enum ExtensibilityKind {
    /** PLAIN_CDR2 — kein DHEADER. */
    FINAL,
    /** DELIMITED_CDR2 — DHEADER vor Body. */
    APPENDABLE,
    /** PL_CDR2 — EMHEADER pro Member. */
    MUTABLE
}
