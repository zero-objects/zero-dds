// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
package org.omg.type;

/**
 * The box of an {@code out} or {@code inout} parameter: IDL4 to Java
 * Language Mapping 1.0 §7.1.3. A primitive is held boxed
 * ({@code Holder<Integer>} for an {@code int}).
 *
 * @param <E> the type held
 */
public class Holder<E> {
    /** The value. */
    public E value;

    /** A holder of nothing. */
    public Holder() {
    }

    /** A holder of {@code value}. */
    public Holder(E value) {
        this.value = value;
    }
}
