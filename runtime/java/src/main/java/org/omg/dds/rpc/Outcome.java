// SPDX-License-Identifier: Apache-2.0
package org.omg.dds.rpc;

import java.util.function.Supplier;

/**
 * What a service implementation's operation did: returned a value, or
 * raised one of the exceptions its {@code raises} clause declares.
 *
 * <p>The generated dispatcher needs the difference to fill the right case
 * of the {@code Result} union (§7.5.1.1.5), and the generator has no
 * {@code try} statement to write it with, so the call goes through here.
 * Any other failure of the implementation is not caught: the service
 * answers it with {@code REMOTE_EX_UNKNOWN_EXCEPTION} (§7.5.2).
 *
 * @param <T> what the operation returns
 */
public final class Outcome<T> {

    private final T value;
    private final UserExceptionRaised raised;

    private Outcome(T value, UserExceptionRaised raised) {
        this.value = value;
        this.raised = raised;
    }

    /** Calls an operation that returns a value. */
    public static <T> Outcome<T> of(Supplier<T> operation) {
        try {
            return new Outcome<>(operation.get(), null);
        } catch (UserExceptionRaised raised) {
            return new Outcome<>(null, raised);
        }
    }

    /** Calls an operation that returns nothing. */
    public static Outcome<Void> ofVoid(Runnable operation) {
        try {
            operation.run();
            return new Outcome<>(null, null);
        } catch (UserExceptionRaised raised) {
            return new Outcome<>(null, raised);
        }
    }

    /** Whether the operation raised a declared exception. */
    public boolean isRaised() {
        return raised != null;
    }

    /** What the operation returned. */
    public T value() {
        return value;
    }

    /** The exception data holder the operation raised. */
    public Object raised() {
        return raised.value();
    }

    /**
     * The exception to throw on if the holder is not one the operation
     * declares.
     */
    public RuntimeException failure() {
        return raised;
    }
}
