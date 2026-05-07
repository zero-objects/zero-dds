// Runtime annotation for IDL `oneway` (DDS-RPC fire-and-forget marker).
//
// Shipped alongside generated sources from zerodds idl-java (Cluster
// C6.1.D-java). Mirrors OMG DDS-RPC 1.0 §7.3 — methods carrying this
// annotation must have a `void` return type and only `in`-parameters,
// and the runtime must not block the caller waiting for a reply.
package org.zerodds.rpc;

import java.lang.annotation.ElementType;
import java.lang.annotation.Retention;
import java.lang.annotation.RetentionPolicy;
import java.lang.annotation.Target;

/** Marks a service method as oneway (fire-and-forget). */
@Retention(RetentionPolicy.RUNTIME)
@Target(ElementType.METHOD)
public @interface Oneway {
}
