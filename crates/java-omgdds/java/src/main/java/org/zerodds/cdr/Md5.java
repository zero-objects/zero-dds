// SPDX-License-Identifier: Apache-2.0
package org.zerodds.cdr;

import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;

/**
 * MD5-Hash-Helper fuer XTypes §7.6.8 Key-Hash-Berechnung.
 *
 * <p>Wraps {@link java.security.MessageDigest}. Liefert immer 16 Bytes;
 * MD5 ist als XTypes-Spec-Anforderung gewaehlt — nicht als
 * Crypto-Funktion (Key-Hash dient ausschliesslich der Topic-Instance-
 * Identifikation).
 */
public final class Md5 {

    private Md5() {}

    /**
     * Berechnet MD5 ueber {@code data}. Liefert 16 Bytes.
     *
     * @throws XcdrException falls die JVM keinen MD5-Provider hat
     *         (sollte nie eintreten — MD5 ist seit Java 1.4
     *         garantiert; {@link MessageDigest#getInstance(String)}
     *         throwt {@link NoSuchAlgorithmException} nur theoretisch).
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
