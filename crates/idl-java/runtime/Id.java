// Runtime annotation for IDL `@id(N)` (XTypes member-id).
package org.zerodds.types;

import java.lang.annotation.ElementType;
import java.lang.annotation.Retention;
import java.lang.annotation.RetentionPolicy;
import java.lang.annotation.Target;

/** Carries the explicit XTypes member-id assigned via IDL `@id(N)`. */
@Retention(RetentionPolicy.RUNTIME)
@Target({ElementType.FIELD, ElementType.METHOD })
public @interface Id {
    /** The explicit member-id (u32). */
    int value();
}
