// SPDX-License-Identifier: Apache-2.0
package org.zerodds.testing;

/**
 * Waits on an event of another thread instead of sleeping (internal/release/rc8-test-suite.md,
 * "Rules": no sleep as synchronisation, wait on an event with a timeout).
 *
 * <p>A test whose second thread has to act while the first one is blocked in a call waits until
 * that thread is parked inside the call: its state is {@code WAITING} or {@code TIMED_WAITING}
 * and a frame of its stack is the method named.
 */
public final class Waits {

    /** The longest wait for an event; a test of the crate tier has 25 s. */
    public static final long TIMEOUT_NANOS = 10_000_000_000L;

    private Waits() {
    }

    /**
     * Returns once {@code thread} waits inside a method named {@code method}; throws an
     * {@link AssertionError} after {@link #TIMEOUT_NANOS}.
     */
    public static void untilWaitingIn(Thread thread, String method) {
        long end = System.nanoTime() + TIMEOUT_NANOS;
        while (System.nanoTime() < end) {
            Thread.State state = thread.getState();
            if (state == Thread.State.WAITING || state == Thread.State.TIMED_WAITING) {
                for (StackTraceElement frame : thread.getStackTrace()) {
                    if (frame.getMethodName().equals(method)) {
                        return;
                    }
                }
            }
            if (state == Thread.State.TERMINATED) {
                throw new AssertionError(thread.getName() + " ended before it waited in " + method);
            }
            java.util.concurrent.locks.LockSupport.parkNanos(100_000L);
        }
        throw new AssertionError(thread.getName() + " did not wait in " + method + " within 10 s");
    }

    /**
     * Returns once {@code condition} holds; throws an {@link AssertionError} naming
     * {@code what} after {@link #TIMEOUT_NANOS}.
     */
    public static void until(java.util.function.BooleanSupplier condition, String what) {
        long end = System.nanoTime() + TIMEOUT_NANOS;
        while (!condition.getAsBoolean()) {
            if (System.nanoTime() >= end) {
                throw new AssertionError("not within 10 s: " + what);
            }
            java.util.concurrent.locks.LockSupport.parkNanos(1_000_000L);
        }
    }
}
