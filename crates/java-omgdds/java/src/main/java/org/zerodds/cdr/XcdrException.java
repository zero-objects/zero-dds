// SPDX-License-Identifier: Apache-2.0
package org.zerodds.cdr;

/**
 * Runtime-Fehler im XCDR2-Encoder/Decoder.
 *
 * <p>Wird geworfen bei Bounds-Verletzungen, ungueltigen Wire-Daten,
 * Decoder-Stream-Underflow und Encoder-Range-Verstoessen. Erweitert
 * {@link RuntimeException} weil DDS-Java-PSM keine checked Exceptions
 * an den Sample-API-Pfaden vorsieht.
 */
public final class XcdrException extends RuntimeException {

    private static final long serialVersionUID = 1L;

    /** Konstruiert mit Detail-Message. */
    public XcdrException(String message) {
        super(message);
    }

    /** Konstruiert mit Detail-Message und Ursache. */
    public XcdrException(String message, Throwable cause) {
        super(message, cause);
    }
}
