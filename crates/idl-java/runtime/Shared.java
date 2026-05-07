// Runtime annotation for IDL `@shared`.
//
// Spec-Quelle: idl4-cpp 1.0 §8.1.5 + dds-psm-cxx 1.0 §8.1.5.
// `@shared` markiert einen Member als Pointer / shared-Reference.
// In Java sind Class-Felder ohnehin Reference-Types; das Marker-
// Annotation existiert primaer fuer Codegen-Konsumenten + Cross-
// Sprach-Roundtrip-Tooling.
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
