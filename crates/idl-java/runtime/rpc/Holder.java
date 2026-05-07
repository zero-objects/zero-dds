// Generic out-/inout-parameter Holder for the IDL→Java mapping
// (DDS-RPC 1.0 §7.11.2.3 / IDL-to-Java 1.3 §1.5).
//
// Java has no `out` keyword. The Holder<T> wrapper is the canonical
// vehicle the IDL-to-Java mapping uses to convey out-/inout-parameters
// across a method call: the caller pre-allocates a Holder, the callee
// (or the runtime, after decoding the reply) writes to `value`.
package org.zerodds.rpc;

/**
 * Mutable single-field holder used to carry IDL `out` / `inout`
 * parameters across method invocations.
 */
public final class Holder<T> {
    /** The held value. */
    public T value;

    public Holder() {}

    public Holder(T value) {
        this.value = value;
    }
}
