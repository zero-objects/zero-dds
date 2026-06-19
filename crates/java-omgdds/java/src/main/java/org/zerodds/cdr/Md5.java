// SPDX-License-Identifier: Apache-2.0
package org.zerodds.cdr;

import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;

/**
 * MD5 hash helper for XTypes §7.6.8 key hash computation.
 *
 * <p>Wraps {@link java.security.MessageDigest}. Always returns 16 bytes;
 * MD5 is chosen as an XTypes spec requirement — not as a
 * crypto function (the key hash serves exclusively for topic-instance
 * identification).
 */
public final class Md5 {

    private Md5() {}

    /**
     * Computes MD5 over {@code data}. Returns 16 bytes.
     *
     * @throws XcdrException if the JVM has no MD5 provider
     *         (should never happen — MD5 has been guaranteed since
     *         Java 1.4; {@link MessageDigest#getInstance(String)}
     *         throws {@link NoSuchAlgorithmException} only theoretically).
     */
    public static byte[] hash(byte[] data) {
        try {
            MessageDigest md = MessageDigest.getInstance("MD5");
            return md.digest(data);
        } catch (NoSuchAlgorithmException e) {
            throw new XcdrException("MD5 not available", e);
        }
    }
}
