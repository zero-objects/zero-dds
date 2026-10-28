// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
package org.zerodds.security;

/**
 * SecurityException (DDS-Security 1.2 Table 28): thrown by a plugin operation
 * that fails. {@code code} is the category of the failure.
 */
public final class SecurityException extends RuntimeException {
    private static final long serialVersionUID = 1L;

    /** Identity handshake failed. */
    public static final int AUTHENTICATION_FAILED = 1;
    /** No permission for the operation. */
    public static final int ACCESS_DENIED = 2;
    /** A cryptographic operation failed. */
    public static final int CRYPTO_FAILED = 3;
    /** Missing or implausible configuration. */
    public static final int INVALID_CONFIGURATION = 4;
    /** Invalid argument. */
    public static final int BAD_ARGUMENT = 5;
    /** Operation not implemented. */
    public static final int NOT_IMPLEMENTED = 6;
    /** Internal error of the plugin. */
    public static final int INTERNAL = 7;

    private final int code;
    private final int minorCode;

    /** A failure of category {@code code}. */
    public SecurityException(int code, String message) {
        this(code, message, 0);
    }

    /** A failure of category {@code code} with the plugin's own number. */
    public SecurityException(int code, String message, int minorCode) {
        super(message);
        this.code = code;
        this.minorCode = minorCode;
    }

    /** The {@code code} of the exception. */
    public int code() {
        return code;
    }

    /** The {@code minor_code} of the exception. */
    public int minorCode() {
        return minorCode;
    }
}
