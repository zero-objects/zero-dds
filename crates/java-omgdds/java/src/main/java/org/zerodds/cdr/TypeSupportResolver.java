// SPDX-License-Identifier: Apache-2.0
package org.zerodds.cdr;

import org.omg.dds.topic.TopicTypeSupport;

import java.lang.reflect.Field;
import java.lang.reflect.Modifier;

/**
 * Resolves a {@link TopicTypeSupport} for a topic's data {@code Class} when the
 * application does not pass one explicitly — the machinery behind the
 * single-argument {@code createDataWriter(Topic)} / {@code createDataReader(Topic)}
 * convenience overloads.
 *
 * <p>Resolution order (most specific first):
 * <ol>
 *   <li>{@code byte[].class} → {@link ByteArrayTypeSupport#INSTANCE} (built-in
 *       raw-bytes type, zero IDL / zero boilerplate).</li>
 *   <li>A generated companion {@code <FQN>TypeSupport} carrying a {@code public
 *       static} {@code INSTANCE} field — the form {@code idl-java} emits for an
 *       IDL {@code struct} (e.g. {@code sensor_msgs.msg.TemperatureTypeSupport}).</li>
 *   <li>Otherwise {@link ReflectionTypeSupport#of(Class)} — reflective XCDR2 for
 *       a plain Java bean.</li>
 * </ol>
 */
public final class TypeSupportResolver {

    private TypeSupportResolver() {}

    /**
     * Resolves a {@link TopicTypeSupport} for {@code type}.
     *
     * @param type the topic data class (never {@code null}).
     * @param <T> the topic data type.
     * @return a usable {@link TopicTypeSupport} for {@code type}.
     */
    @SuppressWarnings("unchecked")
    public static <T> TopicTypeSupport<T> resolve(Class<T> type) {
        if (type == null) {
            throw new XcdrException("topic type must not be null");
        }
        if (type == byte[].class) {
            return (TopicTypeSupport<T>) ByteArrayTypeSupport.INSTANCE;
        }
        TopicTypeSupport<T> generated = lookupGenerated(type);
        if (generated != null) {
            return generated;
        }
        return ReflectionTypeSupport.of(type);
    }

    /**
     * Looks for the {@code idl-java}-generated {@code <Name>TypeSupport.INSTANCE}
     * companion next to {@code type}. Returns {@code null} if there is none (the
     * common case for hand-written beans), so the caller can fall back to
     * reflection.
     */
    @SuppressWarnings("unchecked")
    private static <T> TopicTypeSupport<T> lookupGenerated(Class<T> type) {
        String candidate = type.getName() + "TypeSupport";
        Class<?> tsClass;
        try {
            tsClass = Class.forName(candidate, true, classLoaderFor(type));
        } catch (ClassNotFoundException notGenerated) {
            return null;
        }
        if (!TopicTypeSupport.class.isAssignableFrom(tsClass)) {
            return null;
        }
        try {
            Field instance = tsClass.getField("INSTANCE");
            if (Modifier.isStatic(instance.getModifiers())) {
                Object value = instance.get(null);
                if (value instanceof TopicTypeSupport) {
                    return (TopicTypeSupport<T>) value;
                }
            }
        } catch (NoSuchFieldException | IllegalAccessException ignored) {
            // Fall through to no-arg construction below.
        }
        try {
            Object value = tsClass.getDeclaredConstructor().newInstance();
            if (value instanceof TopicTypeSupport) {
                return (TopicTypeSupport<T>) value;
            }
        } catch (ReflectiveOperationException ignored) {
            // Not instantiable as a TypeSupport → fall back to reflection.
        }
        return null;
    }

    private static ClassLoader classLoaderFor(Class<?> type) {
        ClassLoader cl = type.getClassLoader();
        return cl != null ? cl : Thread.currentThread().getContextClassLoader();
    }
}
