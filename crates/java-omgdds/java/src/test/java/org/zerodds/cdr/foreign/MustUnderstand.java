// SPDX-License-Identifier: Apache-2.0
package org.zerodds.cdr.foreign;

import java.lang.annotation.ElementType;
import java.lang.annotation.Retention;
import java.lang.annotation.RetentionPolicy;
import java.lang.annotation.Target;

/**
 * An annotation named like the type-system annotation {@code org.omg.dds.type.MustUnderstand} but
 * declared in another package. DDS-Java-PSM 1.0 §8.9: a Java annotation that is not one of
 * the type system's is not represented, whatever its simple name.
 */
@Retention(RetentionPolicy.RUNTIME)
@Target({ElementType.FIELD, ElementType.METHOD})
public @interface MustUnderstand {
}
