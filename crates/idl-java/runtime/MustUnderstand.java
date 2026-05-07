// Runtime annotation for IDL `@must_understand` (XTypes mutability).
package org.zerodds.types;

import java.lang.annotation.ElementType;
import java.lang.annotation.Retention;
import java.lang.annotation.RetentionPolicy;
import java.lang.annotation.Target;

/** Marks a member as `must_understand`; readers must reject samples lacking it. */
@Retention(RetentionPolicy.RUNTIME)
@Target({ElementType.FIELD, ElementType.METHOD })
public @interface MustUnderstand {
}
