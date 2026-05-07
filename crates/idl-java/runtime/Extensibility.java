// Runtime annotation for IDL `@extensibility(FINAL|APPENDABLE|MUTABLE)`.
package org.zerodds.types;

import java.lang.annotation.ElementType;
import java.lang.annotation.Retention;
import java.lang.annotation.RetentionPolicy;
import java.lang.annotation.Target;

/** Carries the IDL extensibility kind for the annotated type. */
@Retention(RetentionPolicy.RUNTIME)
@Target(ElementType.TYPE)
public @interface Extensibility {
    /** Extensibility kind. */
    Kind value();

    /** XTypes extensibility kinds (XTypes §7.2.2.4.4). */
    enum Kind {
        /** Strict equality on type and order. */
        FINAL,
        /** Trailing members may be added. */
        APPENDABLE,
        /** Member-id keyed match; arbitrary insertion. */
        MUTABLE
    }
}
