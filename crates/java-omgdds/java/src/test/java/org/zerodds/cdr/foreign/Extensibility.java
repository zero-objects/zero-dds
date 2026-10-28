// SPDX-License-Identifier: Apache-2.0
package org.zerodds.cdr.foreign;

import java.lang.annotation.ElementType;
import java.lang.annotation.Retention;
import java.lang.annotation.RetentionPolicy;
import java.lang.annotation.Target;

/**
 * An annotation named like the type-system annotation {@code org.omg.dds.type.Extensibility}
 * but declared in another package (DDS-Java-PSM 1.0 §8.9). Its value is the text a
 * name-only match would have taken for the extensibility kind.
 */
@Retention(RetentionPolicy.RUNTIME)
@Target(ElementType.TYPE)
public @interface Extensibility {
    String value();
}
