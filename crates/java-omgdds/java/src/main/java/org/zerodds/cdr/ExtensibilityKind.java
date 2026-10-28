// SPDX-License-Identifier: Apache-2.0
package org.zerodds.cdr;

/**
 * Extensibility category of an IDL struct per OMG XTypes 1.3
 * §7.2.2.4.4 and DDS-Java-PSM 1.0 §8.6.1.2.
 *
 * <p>Determines the wire layout:
 * <ul>
 *   <li>{@link #FINAL}: PLAIN_CDR2 without DHEADER. §8.6.1.2 puts FINAL on a
 *       final class that extends {@code java.lang.Object} directly.</li>
 *   <li>{@link #APPENDABLE}: DELIMITED_CDR2 with DHEADER (object-size).
 *       §8.6.1.2 puts EXTENSIBLE on every other class; XTypes 1.3 §7.2.2.4.4.4
 *       says that EXTENSIBLE is what XTypes 1.1 called APPENDABLE, so it is the
 *       same kind on the wire, and the binding reads it as APPENDABLE.</li>
 *   <li>{@link #MUTABLE}: PL_CDR2 with DHEADER + EMHEADER per member.</li>
 * </ul>
 */
public enum ExtensibilityKind {
    /** PLAIN_CDR2 — no DHEADER. */
    FINAL,
    /** DELIMITED_CDR2 — DHEADER before body. */
    APPENDABLE,
    /** PL_CDR2 — EMHEADER pro Member. */
    MUTABLE,
    /**
     * The XTypes 1.1 name of {@link #APPENDABLE}; the binding does not produce it
     * (it maps EXTENSIBLE to APPENDABLE) and treats it like APPENDABLE where it
     * meets it.
     */
    EXTENSIBLE
}
