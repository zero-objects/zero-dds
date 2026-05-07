// Runtime annotation for IDL `@nested` (struct is non-topic-type).
package org.zerodds.types;

import java.lang.annotation.ElementType;
import java.lang.annotation.Retention;
import java.lang.annotation.RetentionPolicy;
import java.lang.annotation.Target;

/** Marks a constructed type as IDL `@nested` — not eligible as a DDS topic. */
@Retention(RetentionPolicy.RUNTIME)
@Target(ElementType.TYPE)
public @interface Nested {
}
