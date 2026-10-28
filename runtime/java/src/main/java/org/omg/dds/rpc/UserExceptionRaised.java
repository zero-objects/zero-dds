// SPDX-License-Identifier: Apache-2.0
package org.omg.dds.rpc;

/**
 * An exception declared in the {@code raises} clause of an operation,
 * as a Java exception.
 *
 * <p>The generated exception types are data holders and not
 * {@code Throwable}s (a {@code Throwable} is {@code Serializable}, and
 * nothing about the wire form decides its {@code serialVersionUID}).
 * DDS-RPC 1.0 §7.11.1.1.2 rule 6.c: the operation has no exception
 * specification, "the implementation may still throw the exceptions
 * specified by the IDL operation". A service implementation throws this to
 * report one, and a client call throws it when the service did.
 */
public final class UserExceptionRaised extends RuntimeException {

    private static final long serialVersionUID = 1L;

    private final transient Object value;

    /**
     * @param value the generated exception holder
     */
    public UserExceptionRaised(Object value) {
        super("the operation raised " + value.getClass().getSimpleName());
        this.value = value;
    }

    /** The generated exception holder, of the class the {@code raises} clause names. */
    public Object value() {
        return value;
    }
}
