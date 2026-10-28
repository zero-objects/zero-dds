// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
package org.zerodds.wire;

import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;

/**
 * The MD5 a KeyHash is made of.
 *
 * <p>DDSI-RTPS 2.5 §9.6.3.8 names MD5 for the KeyHash of a key holder longer
 * than sixteen bytes. It is <b>not</b> a security claim: the KeyHash
 * identifies an instance and is not a signature, and nothing in this project
 * relies on MD5 being hard to invert.
 *
 * <p>Unlike the Rust and C columns, which carry RFC 1321 by hand because a
 * freestanding target has no library to call, this one calls
 * {@link MessageDigest}. A fourth hand-written MD5 would be a fourth place for
 * the same arithmetic to be wrong, and the platform's is the same function —
 * {@code WireSelfTest} checks it against RFC 1321's own test vectors so that
 * the assumption is stated in a test and not in a comment.
 */
public final class Md5 {

    /** A digest is sixteen bytes. */
    public static final int LENGTH = 16;

    private Md5() {
    }

    /** The MD5 digest of {@code input}. */
    public static byte[] digest(byte[] input) {
        return digest(input, 0, input.length);
    }

    /** The MD5 digest of {@code length} bytes of {@code input} from {@code from}. */
    public static byte[] digest(byte[] input, int from, int length) {
        try {
            MessageDigest digest = MessageDigest.getInstance("MD5");
            digest.update(input, from, length);
            return digest.digest();
        } catch (NoSuchAlgorithmException error) {
            // Every conforming Java runtime has MD5; a runtime that does not
            // cannot produce a KeyHash at all, and saying so beats returning
            // sixteen bytes of something else.
            throw new IllegalStateException("this Java runtime has no MD5", error);
        }
    }
}
