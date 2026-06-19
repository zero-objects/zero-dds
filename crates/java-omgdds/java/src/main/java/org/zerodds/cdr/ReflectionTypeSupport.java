// SPDX-License-Identifier: Apache-2.0
package org.zerodds.cdr;

import java.lang.annotation.Annotation;
import java.lang.reflect.AnnotatedElement;
import java.lang.reflect.Array;
import java.lang.reflect.Constructor;
import java.lang.reflect.Field;
import java.lang.reflect.Modifier;
import java.lang.reflect.ParameterizedType;
import java.lang.reflect.Type;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;

/**
 * Reflection-based {@link TopicTypeSupport} for plain Java beans —
 * DDS-Java-PSM 1.0 §8 (Java Type Representation) + §1.2: marshal/unmarshal a
 * Java type to/from XCDR2 <em>without</em> an IDL or generated {@code
 * TypeSupport}, by inspecting its fields reflectively.
 *
 * <p>The produced wire bytes are <strong>byte-identical</strong> to what the
 * typed {@code idl-java} codegen emits for an equivalent IDL {@code struct}:
 * this class drives the same {@link Xcdr2Writer}/{@link Xcdr2Reader} with the
 * same alignment, DHEADER and EMHEADER conventions (see
 * {@code Xcdr2WireVectorsTest} V-1..V-12).
 *
 * <h2>Type mapping (XTypes 1.3 §8.2 Tab.8.1)</h2>
 * <ul>
 *   <li>{@code boolean/Boolean}→BOOLEAN, {@code byte/Byte}→BYTE,
 *       {@code char/Character}→CHAR8, {@code short/Short}→INT16,
 *       {@code int/Integer}→INT32, {@code long/Long}→INT64,
 *       {@code float/Float}→FLOAT32, {@code double/Double}→FLOAT64,
 *       {@code String}→STRING&lt;CHAR8&gt;.</li>
 *   <li>{@code T[]} and {@code java.util.List<T>}→{@code sequence<T>}
 *       (primitive-element sequences inline per V-5; non-primitive-element
 *       sequences carry a DHEADER per V-6).</li>
 *   <li>{@code java.util.Map<K,V>}→{@code map<K,V>} (DHEADER + count +
 *       key/value entries; roundtrip-stable for a given instance).</li>
 *   <li>Any other class is treated as a nested aggregate and marshalled
 *       recursively with its own extensibility.</li>
 * </ul>
 *
 * <h2>Extensibility &amp; annotations</h2>
 * The extensibility defaults to {@link ExtensibilityKind#FINAL} (a plain bean
 * marshals exactly like a FINAL IDL struct). It is overridable via an
 * {@code @Extensibility(FINAL|APPENDABLE|MUTABLE)} annotation on the type;
 * fields honour {@code @Key} (topic key), {@code @Id} (explicit member id for
 * MUTABLE) and {@code @MustUnderstand}. Annotations are matched reflectively by
 * <em>simple name</em>, so the {@code org.zerodds.types.*} annotations emitted
 * by {@code idl-java} are honoured without a compile-time dependency on them.
 *
 * <p>Field order follows {@link Class#getDeclaredFields()} (HotSpot returns
 * declaration order); static, transient and synthetic fields are skipped.
 *
 * @param <T> the bean type.
 */
public final class ReflectionTypeSupport<T> implements TopicTypeSupport<T> {

    private static final Map<Class<?>, TypeInfo> CACHE = new ConcurrentHashMap<>();

    private final Class<T> type;
    private final TypeInfo info;

    /** Builds a reflection-based TypeSupport for {@code type}. */
    public ReflectionTypeSupport(Class<T> type) {
        if (type == null) {
            throw new XcdrException("type must not be null");
        }
        this.type = type;
        this.info = infoOf(type);
    }

    /** Convenience factory. */
    public static <T> ReflectionTypeSupport<T> of(Class<T> type) {
        return new ReflectionTypeSupport<>(type);
    }

    // ------------------------------------------------------------------
    // TopicTypeSupport
    // ------------------------------------------------------------------

    @Override
    public String getTypeName() {
        return info.typeName;
    }

    @Override
    public boolean isKeyed() {
        return info.hasKey;
    }

    @Override
    public ExtensibilityKind getExtensibility() {
        return info.extensibility;
    }

    @Override
    public byte[] encode(T sample) {
        return encode(sample, EndianMode.LITTLE_ENDIAN);
    }

    @Override
    public byte[] encode(T sample, EndianMode endian) {
        if (sample == null) {
            throw new XcdrException("sample must not be null");
        }
        Xcdr2Writer w = new Xcdr2Writer(endian);
        encodeStruct(w, info, sample);
        return w.toByteArray();
    }

    @Override
    public T decode(byte[] bytes) {
        return decode(bytes, 0, bytes.length);
    }

    @Override
    @SuppressWarnings("unchecked")
    public T decode(byte[] bytes, int offset, int length) {
        Xcdr2Reader r = new Xcdr2Reader(bytes, offset, length, EndianMode.LITTLE_ENDIAN);
        return (T) decodeStruct(r, info);
    }

    @Override
    public byte[] keyHash(T sample) {
        byte[] out = new byte[16];
        if (!info.hasKey || sample == null) {
            return out; // all-zero per interface contract.
        }
        // PlainCdr2BeKeyHolder: the @key fields in BE, in declaration order
        // (XTypes 1.3 §7.6.8).
        Xcdr2Writer w = new Xcdr2Writer(EndianMode.BIG_ENDIAN);
        for (MemberInfo m : info.members) {
            if (m.isKey) {
                encodeValue(w, m.type, m.elementType, get(m.field, sample));
            }
        }
        byte[] holder = w.toByteArray();
        // §7.6.8.4: holder ≤ 16 octets → zero-pad; otherwise MD5.
        if (holder.length <= 16) {
            System.arraycopy(holder, 0, out, 0, holder.length);
            return out;
        }
        return Md5.hash(holder);
    }

    // ------------------------------------------------------------------
    // Struct encode / decode
    // ------------------------------------------------------------------

    private void encodeStruct(Xcdr2Writer w, TypeInfo ti, Object sample) {
        switch (ti.extensibility) {
            case FINAL: {
                for (MemberInfo m : ti.members) {
                    encodeValue(w, m.type, m.elementType, get(m.field, sample));
                }
                break;
            }
            case APPENDABLE: {
                int dh = w.beginAppendable();
                for (MemberInfo m : ti.members) {
                    encodeValue(w, m.type, m.elementType, get(m.field, sample));
                }
                w.endDelimited(dh);
                break;
            }
            case MUTABLE: {
                int dh = w.beginMutable();
                for (MemberInfo m : ti.members) {
                    Object value = get(m.field, sample);
                    if (m.isOptional && value == null) {
                        continue; // absent optional → no EMHEADER (V-11 None).
                    }
                    // LC_NEXTINT is the universal length-code (always accepted).
                    // The member body is written INLINE (so its alignment is
                    // relative to the stream, not a temp buffer) and the NEXTINT
                    // is back-patched with the body size — identical mechanics to
                    // beginAppendable/endDelimited (a uint32 size prefix).
                    w.writeEmHeader(m.memberId, Xcdr2Writer.LC_NEXTINT, m.mustUnderstand);
                    int ni = w.beginAppendable();
                    encodeValue(w, m.type, m.elementType, value);
                    w.endDelimited(ni);
                }
                w.endDelimited(dh);
                break;
            }
            default:
                throw new XcdrException("unknown extensibility: " + ti.extensibility);
        }
    }

    private Object decodeStruct(Xcdr2Reader r, TypeInfo ti) {
        List<Object> values = new ArrayList<>(ti.members.size());
        switch (ti.extensibility) {
            case FINAL: {
                for (MemberInfo m : ti.members) {
                    values.add(decodeValue(r, m.type, m.elementType));
                }
                break;
            }
            case APPENDABLE: {
                int size = r.readDHeader();
                int end = r.position() + size;
                for (MemberInfo m : ti.members) {
                    if (r.position() >= end) {
                        values.add(defaultValue(m.type)); // trailing member absent.
                    } else {
                        values.add(decodeValue(r, m.type, m.elementType));
                    }
                }
                // Skip any extra appended members we do not know about.
                while (r.position() < end) {
                    r.skip(1);
                }
                break;
            }
            case MUTABLE: {
                int size = r.readDHeader();
                int end = r.position() + size;
                // Pre-fill with defaults; absent optionals stay null/default.
                Map<Integer, Object> byId = new LinkedHashMap<>();
                while (r.position() < end) {
                    Xcdr2Reader.EmHeader e = r.readEmHeader();
                    MemberInfo m = ti.byMemberId.get(e.memberId);
                    if (m == null) {
                        // Unknown member — skip its body using the length-code.
                        skipMutableMember(r, e);
                        continue;
                    }
                    if (e.lc >= Xcdr2Writer.LC_NEXTINT) {
                        r.readNextInt(); // size hint, not needed: dispatch by type.
                    }
                    byId.put(e.memberId, decodeValue(r, m.type, m.elementType));
                }
                for (MemberInfo m : ti.members) {
                    values.add(byId.containsKey(m.memberId)
                            ? byId.get(m.memberId)
                            : defaultValue(m.type));
                }
                break;
            }
            default:
                throw new XcdrException("unknown extensibility: " + ti.extensibility);
        }
        return instantiate(ti, values);
    }

    private void skipMutableMember(Xcdr2Reader r, Xcdr2Reader.EmHeader e) {
        switch (e.lc) {
            case Xcdr2Writer.LC_BYTE:
                r.skip(1);
                break;
            case Xcdr2Writer.LC_SHORT:
                r.align(2);
                r.skip(2);
                break;
            case Xcdr2Writer.LC_INT32:
                r.align(4);
                r.skip(4);
                break;
            case Xcdr2Writer.LC_INT64:
                r.align(8);
                r.skip(8);
                break;
            default: {
                int n = r.readNextInt();
                r.skip(n);
                break;
            }
        }
    }

    // ------------------------------------------------------------------
    // Value encode / decode (recursive)
    // ------------------------------------------------------------------

    private void encodeValue(Xcdr2Writer w, Class<?> cls, Type generic, Object value) {
        if (cls == boolean.class || cls == Boolean.class) {
            w.writeBoolean((Boolean) value);
        } else if (cls == byte.class || cls == Byte.class) {
            w.writeOctet((Byte) value);
        } else if (cls == char.class || cls == Character.class) {
            w.writeChar((Character) value);
        } else if (cls == short.class || cls == Short.class) {
            w.writeInt16((Short) value);
        } else if (cls == int.class || cls == Integer.class) {
            w.writeInt32((Integer) value);
        } else if (cls == long.class || cls == Long.class) {
            w.writeInt64((Long) value);
        } else if (cls == float.class || cls == Float.class) {
            w.writeFloat32((Float) value);
        } else if (cls == double.class || cls == Double.class) {
            w.writeFloat64((Double) value);
        } else if (cls == String.class) {
            w.writeString((String) value);
        } else if (cls.isArray()) {
            encodeSequence(w, cls.getComponentType(), null, arrayToList(value));
        } else if (List.class.isAssignableFrom(cls)) {
            Class<?> elem = elementClass(generic, 0);
            encodeSequence(w, elem, elementType(generic, 0), (List<?>) value);
        } else if (Map.class.isAssignableFrom(cls)) {
            encodeMap(w, generic, (Map<?, ?>) value);
        } else {
            encodeStruct(w, infoOf(cls), value); // nested aggregate.
        }
    }

    private Object decodeValue(Xcdr2Reader r, Class<?> cls, Type generic) {
        if (cls == boolean.class || cls == Boolean.class) {
            return r.readBoolean();
        } else if (cls == byte.class || cls == Byte.class) {
            return r.readOctet();
        } else if (cls == char.class || cls == Character.class) {
            return r.readChar();
        } else if (cls == short.class || cls == Short.class) {
            return r.readInt16();
        } else if (cls == int.class || cls == Integer.class) {
            return r.readInt32();
        } else if (cls == long.class || cls == Long.class) {
            return r.readInt64();
        } else if (cls == float.class || cls == Float.class) {
            return r.readFloat32();
        } else if (cls == double.class || cls == Double.class) {
            return r.readFloat64();
        } else if (cls == String.class) {
            return r.readString();
        } else if (cls.isArray()) {
            List<Object> list = decodeSequence(r, cls.getComponentType(), null);
            return listToArray(cls.getComponentType(), list);
        } else if (List.class.isAssignableFrom(cls)) {
            return decodeSequence(r, elementClass(generic, 0), elementType(generic, 0));
        } else if (Map.class.isAssignableFrom(cls)) {
            return decodeMap(r, generic);
        } else {
            return decodeStruct(r, infoOf(cls)); // nested aggregate.
        }
    }

    private void encodeSequence(Xcdr2Writer w, Class<?> elem, Type elemGeneric, List<?> values) {
        boolean primitive = isPrimitiveElement(elem);
        if (primitive) {
            w.writeSequenceCount(values.size());
            for (Object v : values) {
                encodeValue(w, elem, elemGeneric, v);
            }
        } else {
            // Non-primitive elements: DHEADER over [count + elements] (V-6).
            int dh = w.beginAppendable();
            w.writeSequenceCount(values.size());
            for (Object v : values) {
                encodeValue(w, elem, elemGeneric, v);
            }
            w.endDelimited(dh);
        }
    }

    private List<Object> decodeSequence(Xcdr2Reader r, Class<?> elem, Type elemGeneric) {
        boolean primitive = isPrimitiveElement(elem);
        List<Object> out = new ArrayList<>();
        if (primitive) {
            int n = r.readSequenceCount();
            for (int i = 0; i < n; i++) {
                out.add(decodeValue(r, elem, elemGeneric));
            }
        } else {
            r.readDHeader();
            int n = r.readSequenceCount();
            for (int i = 0; i < n; i++) {
                out.add(decodeValue(r, elem, elemGeneric));
            }
        }
        return out;
    }

    private void encodeMap(Xcdr2Writer w, Type generic, Map<?, ?> map) {
        Class<?> kCls = elementClass(generic, 0);
        Class<?> vCls = elementClass(generic, 1);
        Type kGen = elementType(generic, 0);
        Type vGen = elementType(generic, 1);
        // Map is a non-primitive aggregate → DHEADER over [count + entries].
        int dh = w.beginAppendable();
        w.writeSequenceCount(map.size());
        for (Map.Entry<?, ?> e : map.entrySet()) {
            encodeValue(w, kCls, kGen, e.getKey());
            encodeValue(w, vCls, vGen, e.getValue());
        }
        w.endDelimited(dh);
    }

    private Map<Object, Object> decodeMap(Xcdr2Reader r, Type generic) {
        Class<?> kCls = elementClass(generic, 0);
        Class<?> vCls = elementClass(generic, 1);
        Type kGen = elementType(generic, 0);
        Type vGen = elementType(generic, 1);
        r.readDHeader();
        int n = r.readSequenceCount();
        Map<Object, Object> out = new LinkedHashMap<>();
        for (int i = 0; i < n; i++) {
            Object k = decodeValue(r, kCls, kGen);
            Object v = decodeValue(r, vCls, vGen);
            out.put(k, v);
        }
        return out;
    }

    private static boolean isPrimitiveElement(Class<?> c) {
        return c == boolean.class || c == Boolean.class
                || c == byte.class || c == Byte.class
                || c == char.class || c == Character.class
                || c == short.class || c == Short.class
                || c == int.class || c == Integer.class
                || c == long.class || c == Long.class
                || c == float.class || c == Float.class
                || c == double.class || c == Double.class;
    }

    // ------------------------------------------------------------------
    // Introspection
    // ------------------------------------------------------------------

    static TypeInfo infoOf(Class<?> cls) {
        TypeInfo cached = CACHE.get(cls);
        if (cached != null) {
            return cached;
        }
        TypeInfo ti = introspect(cls);
        CACHE.put(cls, ti);
        return ti;
    }

    private static TypeInfo introspect(Class<?> cls) {
        ExtensibilityKind ext = ExtensibilityKind.FINAL;
        Object extAnn = annotation(cls, "Extensibility");
        if (extAnn != null) {
            Object v = annotationValue(extAnn, "value");
            if (v != null) {
                ext = mapExtensibility(v.toString());
            }
        }
        List<MemberInfo> members = new ArrayList<>();
        int nextId = 1;
        boolean anyKey = false;
        for (Field f : cls.getDeclaredFields()) {
            int mod = f.getModifiers();
            if (Modifier.isStatic(mod) || Modifier.isTransient(mod) || f.isSynthetic()) {
                continue;
            }
            f.setAccessible(true);
            MemberInfo m = new MemberInfo();
            m.field = f;
            m.name = f.getName();
            m.type = f.getType();
            m.elementType = f.getGenericType();
            m.isKey = annotation(f, "Key") != null;
            m.isOptional = annotation(f, "Optional") != null;
            m.mustUnderstand = annotation(f, "MustUnderstand") != null;
            Object idAnn = annotation(f, "Id");
            if (idAnn != null) {
                Object idv = annotationValue(idAnn, "value");
                m.memberId = ((Number) idv).intValue();
            } else {
                m.memberId = nextId;
            }
            nextId = m.memberId + 1;
            anyKey = anyKey || m.isKey;
            members.add(m);
        }
        TypeInfo ti = new TypeInfo();
        // Spec type-name convention Module::Sub::Struct: binary name with
        // package/nested separators rendered as "::".
        ti.typeName = cls.getName().replace('$', '.').replace(".", "::");
        ti.extensibility = ext;
        ti.members = members;
        ti.hasKey = anyKey;
        ti.declaringClass = cls;
        for (MemberInfo m : members) {
            ti.byMemberId.put(m.memberId, m);
        }
        return ti;
    }

    private static ExtensibilityKind mapExtensibility(String name) {
        String n = name.toUpperCase();
        if (n.contains("APPENDABLE")) {
            return ExtensibilityKind.APPENDABLE;
        }
        if (n.contains("MUTABLE")) {
            return ExtensibilityKind.MUTABLE;
        }
        return ExtensibilityKind.FINAL;
    }

    // ------------------------------------------------------------------
    // Instantiation
    // ------------------------------------------------------------------

    private static Object instantiate(TypeInfo ti, List<Object> values) {
        Class<?> cls = ti.declaringClass;
        // 1) No-arg constructor + field set (classic mutable bean).
        try {
            Constructor<?> ctor = cls.getDeclaredConstructor();
            ctor.setAccessible(true);
            Object obj = ctor.newInstance();
            for (int i = 0; i < ti.members.size(); i++) {
                MemberInfo m = ti.members.get(i);
                m.field.set(obj, coerce(m.type, values.get(i)));
            }
            return obj;
        } catch (NoSuchMethodException noNoArg) {
            // 2) Canonical constructor (records / immutable beans).
            return instantiateCanonical(cls, ti, values);
        } catch (ReflectiveOperationException e) {
            throw new XcdrException("cannot instantiate " + cls.getName() + ": " + e);
        }
    }

    private static Object instantiateCanonical(Class<?> cls, TypeInfo ti, List<Object> values) {
        Class<?>[] paramTypes = new Class<?>[ti.members.size()];
        Object[] args = new Object[ti.members.size()];
        for (int i = 0; i < ti.members.size(); i++) {
            MemberInfo m = ti.members.get(i);
            paramTypes[i] = m.type;
            args[i] = coerce(m.type, values.get(i));
        }
        try {
            Constructor<?> ctor = cls.getDeclaredConstructor(paramTypes);
            ctor.setAccessible(true);
            return ctor.newInstance(args);
        } catch (ReflectiveOperationException e) {
            throw new XcdrException(
                    "no usable constructor for " + cls.getName()
                            + " (need no-arg or canonical): " + e);
        }
    }

    /** Coerce decoded values to the exact field type (e.g. List → array done upstream). */
    private static Object coerce(Class<?> target, Object value) {
        return value;
    }

    private static Object defaultValue(Class<?> cls) {
        if (cls == boolean.class) {
            return Boolean.FALSE;
        }
        if (cls == byte.class) {
            return (byte) 0;
        }
        if (cls == char.class) {
            return (char) 0;
        }
        if (cls == short.class) {
            return (short) 0;
        }
        if (cls == int.class) {
            return 0;
        }
        if (cls == long.class) {
            return 0L;
        }
        if (cls == float.class) {
            return 0f;
        }
        if (cls == double.class) {
            return 0d;
        }
        return null;
    }

    // ------------------------------------------------------------------
    // Reflection helpers
    // ------------------------------------------------------------------

    private static Object get(Field f, Object obj) {
        try {
            return f.get(obj);
        } catch (IllegalAccessException e) {
            throw new XcdrException("cannot read field " + f.getName() + ": " + e);
        }
    }

    /** Returns the annotation on {@code el} whose simple name equals {@code simpleName}, or null. */
    private static Object annotation(AnnotatedElement el, String simpleName) {
        for (Annotation a : el.getDeclaredAnnotations()) {
            if (a.annotationType().getSimpleName().equals(simpleName)) {
                return a;
            }
        }
        return null;
    }

    private static Object annotationValue(Object annotation, String method) {
        try {
            return annotation.getClass().getMethod(method).invoke(annotation);
        } catch (ReflectiveOperationException e) {
            return null;
        }
    }

    private static Class<?> elementClass(Type generic, int index) {
        Type t = elementType(generic, index);
        if (t instanceof Class) {
            return (Class<?>) t;
        }
        if (t instanceof ParameterizedType) {
            return (Class<?>) ((ParameterizedType) t).getRawType();
        }
        // Erased generic with no element instance to infer from.
        throw new XcdrException(
                "cannot infer element type for collection/map (index " + index
                        + "); declare a concrete parameterized field type");
    }

    private static Type elementType(Type generic, int index) {
        if (generic instanceof ParameterizedType) {
            Type[] args = ((ParameterizedType) generic).getActualTypeArguments();
            if (index < args.length) {
                return args[index];
            }
        }
        return Object.class;
    }

    private static List<Object> arrayToList(Object array) {
        int n = Array.getLength(array);
        List<Object> out = new ArrayList<>(n);
        for (int i = 0; i < n; i++) {
            out.add(Array.get(array, i));
        }
        return out;
    }

    private static Object listToArray(Class<?> elem, List<Object> list) {
        Object array = Array.newInstance(elem, list.size());
        for (int i = 0; i < list.size(); i++) {
            Array.set(array, i, list.get(i));
        }
        return array;
    }

    // ------------------------------------------------------------------
    // Cached type model
    // ------------------------------------------------------------------

    static final class TypeInfo {
        String typeName;
        ExtensibilityKind extensibility;
        List<MemberInfo> members;
        boolean hasKey;
        Class<?> declaringClass;
        final Map<Integer, MemberInfo> byMemberId = new LinkedHashMap<>();
    }

    static final class MemberInfo {
        Field field;
        String name;
        Class<?> type;
        Type elementType;
        boolean isKey;
        boolean isOptional;
        boolean mustUnderstand;
        int memberId;
    }
}
