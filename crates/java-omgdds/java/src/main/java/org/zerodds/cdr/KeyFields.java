// SPDX-License-Identifier: Apache-2.0
package org.zerodds.cdr;

import java.lang.annotation.Annotation;
import java.lang.reflect.Field;
import java.lang.reflect.Modifier;
import java.util.ArrayList;
import java.util.List;

/**
 * Copies the key fields of one sample of a topic type into another sample of
 * it, for {@code getKeyValue(keyHolder, handle)} (DDS-Java-PSM 1.0 §7.2.5,
 * DDS 1.4 §2.2.2.5.3.29): the key holder is overwritten in the fields that
 * form the key and in no other.
 *
 * <p>A key field is a field of the class or of a superclass annotated with
 * {@code @Key} (any annotation of that simple name, as
 * {@link ReflectionTypeSupport} reads it). A class with no such field has no
 * key information in its fields; all its instance fields are copied then.
 */
final class KeyFields {

    private KeyFields() {}

    /**
     * Whether the key fields of {@code holder} can be written. A class whose key fields are
     * final (a record) cannot; {@code getKeyValue} then returns a copy of the sample instead.
     */
    static boolean overwritable(Object holder) {
        if (holder == null) {
            return false;
        }
        boolean anyKey = false;
        boolean finalKey = false;
        boolean finalAny = false;
        for (Class<?> c = holder.getClass(); c != null && c != Object.class; c = c.getSuperclass()) {
            for (Field f : c.getDeclaredFields()) {
                int mod = f.getModifiers();
                if (Modifier.isStatic(mod) || Modifier.isTransient(mod) || f.isSynthetic()) {
                    continue;
                }
                finalAny |= Modifier.isFinal(mod);
                if (isKey(f)) {
                    anyKey = true;
                    finalKey |= Modifier.isFinal(mod);
                }
            }
        }
        return anyKey ? !finalKey : !finalAny;
    }

    static <T> void copy(T from, T into) {
        if (from == null || into == null) {
            throw new IllegalArgumentException("a sample and a key holder are needed");
        }
        List<Field> all = new ArrayList<>();
        List<Field> keys = new ArrayList<>();
        for (Class<?> c = from.getClass(); c != null && c != Object.class; c = c.getSuperclass()) {
            for (Field f : c.getDeclaredFields()) {
                int mod = f.getModifiers();
                if (Modifier.isStatic(mod) || Modifier.isTransient(mod) || f.isSynthetic()) {
                    continue;
                }
                all.add(f);
                if (isKey(f)) {
                    keys.add(f);
                }
            }
        }
        List<Field> chosen = keys.isEmpty() ? all : keys;
        for (Field f : chosen) {
            if (Modifier.isFinal(f.getModifiers())) {
                continue;
            }
            try {
                f.setAccessible(true);
                f.set(into, f.get(from));
            } catch (IllegalAccessException | RuntimeException e) {
                throw new XcdrException("cannot copy the field " + f.getName() + " into the key holder", e);
            }
        }
    }

    private static boolean isKey(Field f) {
        for (Annotation a : f.getAnnotations()) {
            if (a.annotationType().getSimpleName().equals("Key")) {
                return true;
            }
        }
        return false;
    }
}
