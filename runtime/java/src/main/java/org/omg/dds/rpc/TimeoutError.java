// SPDX-License-Identifier: Apache-2.0
package org.omg.dds.rpc;

/**
 * A synchronous call got no reply in time. A local error, the counterpart
 * of the DDS exceptions §7.11.1.3 lets the binding throw besides the
 * remote ones.
 */
public final class TimeoutError extends RuntimeException {

    private static final long serialVersionUID = 1L;

    public TimeoutError(String message) {
        super(message);
    }
}
