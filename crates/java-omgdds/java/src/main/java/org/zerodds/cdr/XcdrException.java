// SPDX-License-Identifier: Apache-2.0
package org.zerodds.cdr;

/**
 * Runtime error in the XCDR2 encoder/decoder.
 *
 * <p>Thrown on bounds violations, invalid wire data,
 * decoder stream underflow and encoder range violations. Extends
 * {@link RuntimeException} because the DDS Java PSM does not use checked exceptions
 * an den Sample-API-Pfaden vorsieht.
 */
public final class XcdrException extends RuntimeException {

    private static final long serialVersionUID = 1L;

    /** Constructs with a detail message. */
    public XcdrException(String message) {
        super(message);
    }

    /** Constructs with a detail message and cause. */
    public XcdrException(String message, Throwable cause) {
        super(message, cause);
    }
}
