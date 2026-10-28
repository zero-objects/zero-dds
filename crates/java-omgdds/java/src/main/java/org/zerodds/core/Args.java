// SPDX-License-Identifier: Apache-2.0
package org.zerodds.core;

/**
 * The checks of the arguments of the DDS operations. DDS-Java-PSM 1.0 7.3.2, Table 7.1:
 * RETCODE_BAD_PARAMETER is {@code java.lang.IllegalArgumentException}, so a {@code null} where an
 * object is needed is that, and no {@code NullPointerException}.
 */
public final class Args {

    private Args() {
    }

    /** {@code value}, which must not be {@code null}. */
    public static <T> T require(T value, String name) {
        if (value == null) {
            throw new IllegalArgumentException("the argument " + name + " must not be null");
        }
        return value;
    }
}
