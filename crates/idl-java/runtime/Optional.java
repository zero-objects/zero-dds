// Runtime annotation for IDL `@optional`.
//
// Note: this annotation is a SEMANTIC marker — the generated field type
// is `java.util.Optional<T>` already; the annotation lets reflection
// distinguish IDL-`@optional` from a Java-`Optional<T>` chosen for
// other reasons.
package org.zerodds.types;

import java.lang.annotation.ElementType;
import java.lang.annotation.Retention;
import java.lang.annotation.RetentionPolicy;
import java.lang.annotation.Target;

/** Marks a struct member as IDL `@optional`. */
@Retention(RetentionPolicy.RUNTIME)
@Target({ElementType.FIELD, ElementType.METHOD })
public @interface Optional {
}
