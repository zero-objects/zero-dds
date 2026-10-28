// SPDX-License-Identifier: Apache-2.0
package org.zerodds.cdr;

import org.zerodds.cdr.TopicTypeSupport;

import java.lang.reflect.Field;
import java.lang.reflect.Modifier;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;

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
 *   <li>{@code String.class}, {@code KeyedString} and {@code KeyedBytes} → the
 *       built-in types {@code DDS::String}, {@code DDS::KeyedString} and
 *       {@code DDS::KeyedBytes} of XTypes 1.3 Annex E
 *       ({@link BuiltinTypeSupports}; DDS-Java-PSM 1.0 §7.8.2).</li>
 *   <li>The {@code zerodds-idlc --lang java}-generated companion
 *       {@code ZdTypeSupports.Support_<tag>.INSTANCE} sitting next to
 *       {@code type}'s own {@code ZdTypes} container — see
 *       {@code tools/idlc/src/java_support.rs} — where {@code type} is a
 *       class nested (directly or through IDL modules) inside a top-level
 *       {@code ZdTypes} class the same generation emitted.</li>
 *   <li>The request and reply types of an interface the generator wrote
 *       ({@code ZdServices}, DDS-RPC 1.0 §7.5.1.1.6 and §7.5.1.1.7) →
 *       {@code org.zerodds.ddsrpc.RpcTopicTypes}, the framing the RPC runtime
 *       writes them in.</li>
 *   <li>Otherwise {@link ReflectionTypeSupport#of(Class)} — reflective XCDR2
 *       for a plain Java bean, over the same wire runtime.</li>
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
        if (type == String.class) {
            return (TopicTypeSupport<T>) BuiltinTypeSupports.StringTypeSupport.INSTANCE;
        }
        if (org.omg.dds.type.builtin.KeyedString.class.isAssignableFrom(type)) {
            return (TopicTypeSupport<T>) BuiltinTypeSupports.KeyedStringTypeSupport.INSTANCE;
        }
        if (org.omg.dds.type.builtin.KeyedBytes.class.isAssignableFrom(type)) {
            return (TopicTypeSupport<T>) BuiltinTypeSupports.KeyedBytesTypeSupport.INSTANCE;
        }
        if (JdkTopicTypeSupport.supports(type)) {
            return new JdkTopicTypeSupport<>(type); // §8.1: a Serializable class of the JDK
        }
        TopicTypeSupport<T> generated = lookupGenerated(type);
        if (generated != null) {
            return generated;
        }
        TopicTypeSupport<T> rpc = lookupRpcTopicType(type);
        if (rpc != null) {
            return rpc;
        }
        return ReflectionTypeSupport.of(type);
    }

    /**
     * The type support of a DDS-RPC topic type ({@code <I>_Request}, {@code <I>_Reply} of a
     * generated interface, DDS-RPC 1.0 §7.5.1.1.6 and §7.5.1.1.7): its samples are framed as
     * the requesters and repliers of the interface write them, which
     * {@code org.zerodds.ddsrpc.RpcTopicTypes} knows. {@code null} when the RPC runtime is not
     * on the class path or {@code type} is no such type.
     */
    @SuppressWarnings("unchecked")
    private static <T> TopicTypeSupport<T> lookupRpcTopicType(Class<T> type) {
        java.lang.reflect.Method resolve;
        try {
            resolve = Class.forName("org.zerodds.ddsrpc.RpcTopicTypes", true, classLoaderFor(type))
                    .getMethod("resolve", Class.class);
        } catch (ClassNotFoundException | NoSuchMethodException absent) {
            return null;
        }
        try {
            return (TopicTypeSupport<T>) resolve.invoke(null, type);
        } catch (IllegalAccessException impossible) {
            throw new XcdrException("org.zerodds.ddsrpc.RpcTopicTypes.resolve is not public",
                    impossible);
        } catch (java.lang.reflect.InvocationTargetException failed) {
            throw new XcdrException("the DDS-RPC type support of " + type.getName()
                    + " could not be made: " + failed.getCause(), failed.getCause());
        }
    }

    /**
     * Looks for the {@code zerodds-idlc --lang java}-generated
     * {@code ZdTypeSupports.Support_<tag>.INSTANCE} companion for
     * {@code type}. Returns {@code null} if there is none — the common case
     * for a hand-written bean, or for a type this generation did not
     * produce — so the caller falls back to reflection.
     *
     * <p>{@code tools/idlc/src/generate.rs} always wraps a generated file's
     * topic types in one top-level {@code public final class ZdTypes { … }}
     * (IDL modules become nested classes inside it), and
     * {@code java_support.rs} emits the sibling {@code ZdTypeSupports}
     * in the same (unnamed or {@code --package}-named) package, with one
     * {@code public static final class Support_<tag>} per topic type —
     * {@code tag} is the type's IDL module path and name joined by
     * {@code _} (see {@code adapter::java::type_tag}). Reconstructing
     * {@code <tag>} is therefore a walk of {@code type}'s own enclosing
     * classes up to (but excluding) {@code ZdTypes}, not a naming
     * convention on {@code type} itself — the two files never generate a
     * {@code <FQN>TypeSupport} sibling class next to the type.
     */
    @SuppressWarnings("unchecked")
    private static <T> TopicTypeSupport<T> lookupGenerated(Class<T> type) {
        List<String> segments = new ArrayList<>();
        segments.add(type.getSimpleName());
        Class<?> topLevel = type;
        Class<?> outer = type.getEnclosingClass();
        while (outer != null) {
            segments.add(outer.getSimpleName());
            topLevel = outer;
            outer = outer.getEnclosingClass();
        }
        if (!"ZdTypes".equals(topLevel.getSimpleName())) {
            return null; // not a class this generation produced.
        }
        segments.remove(segments.size() - 1); // drop the "ZdTypes" sentinel itself.
        Collections.reverse(segments);
        String tag = String.join("_", segments);
        // `getPackage()` returns a non-null Package object with an empty
        // name for the unnamed package (the common case: idlc's own tests
        // generate into it), not null — testing for null alone left a
        // leading `.` in the class name and turned every lookup into a
        // guaranteed `ClassNotFoundException`.
        Package pkg = topLevel.getPackage();
        String packagePrefix = (pkg == null || pkg.getName().isEmpty()) ? "" : pkg.getName() + ".";
        String supportsClassName = packagePrefix + "ZdTypeSupports$Support_" + tag;
        Class<?> tsClass;
        try {
            tsClass = Class.forName(supportsClassName, true, classLoaderFor(type));
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
