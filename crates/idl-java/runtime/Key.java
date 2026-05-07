// Runtime annotation for IDL `@key` (DDS topic-key marker).
//
// Shipped alongside generated sources from zerodds idl-java (Cluster
// C5.4-b). Discovery code reads this annotation via reflection to
// extract topic-key fields per OMG IDL4-Java mapping v1.0 §8.
package org.zerodds.types;

import java.lang.annotation.ElementType;
import java.lang.annotation.Retention;
import java.lang.annotation.RetentionPolicy;
import java.lang.annotation.Target;

/** Marks a struct member as part of the DDS topic key. */
@Retention(RetentionPolicy.RUNTIME)
@Target({ElementType.FIELD, ElementType.METHOD })
public @interface Key {
}
