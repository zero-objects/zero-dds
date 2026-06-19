// Runtime annotation for IDL `@shared`.
//
// Spec source: idl4-cpp 1.0 §8.1.5 + dds-psm-cxx 1.0 §8.1.5.
// `@shared` marks a member as a pointer / shared reference.
// In Java, class fields are reference types anyway; this marker
// annotation exists primarily for codegen consumers and cross-
// language round-trip tooling.
package org.zerodds.types;

import java.lang.annotation.ElementType;
import java.lang.annotation.Retention;
import java.lang.annotation.RetentionPolicy;
import java.lang.annotation.Target;

/** Marks a member as IDL `@shared` (pointer-shared semantics). */
@Retention(RetentionPolicy.RUNTIME)
@Target({ElementType.FIELD, ElementType.METHOD})
public @interface Shared {
}
