// Runtime annotation for IDL `@service` (DDS-RPC service marker).
//
// Shipped alongside generated sources from zerodds idl-java (Cluster
// C6.1.D-java). Mirrors OMG DDS-RPC 1.0 §7.3 service annotation —
// generated `<Service>.java` interfaces carry this marker so DI
// frameworks / reflection-based discovery can locate service contracts.
package org.zerodds.rpc;

import java.lang.annotation.ElementType;
import java.lang.annotation.Retention;
import java.lang.annotation.RetentionPolicy;
import java.lang.annotation.Target;

/** Marks a Java interface as the synchronous facade of a DDS-RPC service. */
@Retention(RetentionPolicy.RUNTIME)
@Target(ElementType.TYPE)
public @interface Service {
    /** Optional service name. Empty string falls back to the interface simple name. */
    String value() default "";
}
