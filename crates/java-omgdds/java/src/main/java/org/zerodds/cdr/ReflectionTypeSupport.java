// SPDX-License-Identifier: Apache-2.0
package org.zerodds.cdr;

import org.zerodds.wire.Endianness;
import org.zerodds.wire.Float128;
import org.zerodds.wire.PlCdr1Framing;
import org.zerodds.wire.Reader;
import org.zerodds.wire.Writer;

import java.lang.annotation.Annotation;
import java.lang.reflect.AnnotatedElement;
import java.lang.reflect.Array;
import java.lang.reflect.Constructor;
import java.lang.reflect.Field;
import java.lang.reflect.Modifier;
import java.lang.reflect.ParameterizedType;
import java.lang.reflect.Type;
import java.math.BigDecimal;
import java.math.BigInteger;
import java.util.ArrayDeque;
import java.util.ArrayList;
import java.util.Collection;
import java.util.EnumMap;
import java.util.EnumSet;
import java.util.LinkedHashMap;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.TreeMap;
import java.util.TreeSet;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.LinkedBlockingDeque;

/**
 * Reflection-based {@link TopicTypeSupport} for plain Java beans —
 * DDS-Java-PSM 1.0 §8 (Java Type Representation) + §1.2: marshal/unmarshal a
 * Java type to/from XCDR2 <em>without</em> an IDL or generated {@code
 * TypeSupport}, by inspecting its fields reflectively.
 *
 * <p>The produced wire bytes are <strong>byte-identical</strong> to what the
 * typed {@code zerodds-idlc --lang java} codegen emits for an equivalent IDL
 * {@code struct}: this class drives the same {@code org.zerodds.wire.Writer}/
 * {@code Reader} (the one XCDR2 codec this binding has — see
 * {@code Xcdr2WireVectorsTest} V-1..V-12) with the same alignment, DHEADER
 * and EMHEADER conventions.
 *
 * <p>One difference from the generated companion is intentional: a
 * {@code @Mutable} member is always written under length code 4 (a plain
 * NEXTINT), never under the self-delimiting codes 5-7 a generator would pick
 * for a {@code string} or nested aggregate member (XTypes 1.3 §7.4.3.4.2).
 * XTypes 1.3 §7.4.3.4.2 permits both framings; this one costs four bytes
 * per such member and gives other bytes than the generated companion for the
 * same type (the "universal NEXTINT" framing of
 * {@code docs/specs/zerodds-ast-1.0.md} §7.1). This reflective fallback has
 * no declared layout plan to pick a length code from.
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
 * The extensibility is {@link ExtensibilityKind#FINAL} for a final class that
 * extends {@code Object} directly and {@link ExtensibilityKind#APPENDABLE} for every
 * other class (DDS-Java-PSM 1.0 §8.6.1.2: EXTENSIBLE, which XTypes 1.3 calls
 * APPENDABLE, so the class is written with a DHEADER). It is overridable via an
 * {@code @Extensibility(FINAL|EXTENSIBLE|MUTABLE)} annotation on the type;
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

    /** The Java class this support encodes. */
    public Class<T> javaType() {
        return type;
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

    /** The superclasses below {@code Object}, nearest first, named as the type is (§8.8). */
    @Override
    public java.util.List<String> baseTypeNames() {
        java.util.List<String> names = new ArrayList<>();
        for (Class<?> c = info.declaringClass.getSuperclass();
                c != null && c != Object.class; c = c.getSuperclass()) {
            names.add(c.getName().replace('$', '.').replace(".", "::"));
        }
        return names;
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
        Writer w = new Writer(order(endian), 4, PlCdr1Framing.defaults(), false);
        encodeStruct(w, info, sample);
        return w.finish();
    }

    @Override
    public T decode(byte[] bytes) {
        return decode(bytes, 0, bytes.length);
    }

    @Override
    @SuppressWarnings("unchecked")
    public T decode(byte[] bytes, int offset, int length) {
        Reader r = new Reader(
                java.util.Arrays.copyOfRange(bytes, offset, offset + length),
                Endianness.LITTLE, 4, PlCdr1Framing.defaults());
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
        Writer w = new Writer(Endianness.BIG, 4, PlCdr1Framing.defaults(), false);
        for (MemberInfo m : info.members) {
            if (m.isKey) {
                encodeMember(w, m, get(m.field, sample));
            }
        }
        byte[] holder = w.finish();
        // RTPS 2.5 §9.6.3.8 step 5: the holder is zero-padded when its MAXIMUM serialized
        // size is 16 octets or less, else MD5 of it. A key member of unbounded size (a string
        // without a bound, a sequence, a map) makes the maximum exceed 16 whatever the
        // actual holder is.
        if (holder.length <= 16 && !hasKeyOfUnboundedSize()) {
            System.arraycopy(holder, 0, out, 0, holder.length);
            return out;
        }
        return Md5.hash(holder);
    }

    private boolean hasKeyOfUnboundedSize() {
        for (MemberInfo m : info.members) {
            if (!m.isKey) {
                continue;
            }
            Class<?> t = m.type;
            if (t == String.class) {
                long bound = m.rep != null && m.rep.bound.length == 1 ? m.rep.bound[0] : 0;
                if (bound == 0 || bound + 5 > 16) {
                    return true;
                }
            } else if (Collection.class.isAssignableFrom(t) || Map.class.isAssignableFrom(t)
                    || t.isArray()) {
                return true;
            }
        }
        return false;
    }

    private static Endianness order(EndianMode endian) {
        java.util.Objects.requireNonNull(endian, "endian");
        switch (endian) {
            case LITTLE_ENDIAN: return Endianness.LITTLE;
            case BIG_ENDIAN: return Endianness.BIG;
            default: throw new IllegalArgumentException("Unsupported byte order");
        }
    }

    // ------------------------------------------------------------------
    // Struct encode / decode
    // ------------------------------------------------------------------

    /** A member's value in the form its {@code @SerializeAs}, if any, gives it. */
    private static void encodeMember(Writer w, MemberInfo m, Object value) {
        if (value == null && !m.isOptional) {
            throw new XcdrException(m.field.getDeclaringClass().getName() + "." + m.name
                    + " is null and the member is not @Optional; a value is needed to serialize it");
        }
        if (m.rep != null) {
            m.rep.encode(w, m.type, m.elementType, value);
        } else {
            encodeValue(w, m.type, m.elementType, value);
        }
    }

    /**
     * A member of a FINAL or APPENDABLE structure (PLAIN_CDR2 and DELIMITED_CDR2): an optional
     * member is preceded by a boolean {@code is_present}, and when it is 0 nothing else is
     * written (XTypes 1.3 §7.4.3.5.2, rule 20).
     */
    private static void encodePlainMember(Writer w, MemberInfo m, Object value) {
        if (m.isOptional) {
            w.writeBool(value != null);
            if (value == null) {
                return;
            }
        }
        encodeMember(w, m, value);
    }

    private static Object decodePlainMember(Reader r, MemberInfo m) {
        if (m.isOptional && !r.readBool()) {
            return null;
        }
        return decodeMember(r, m);
    }

    /** The value of a member the stream did not carry: null for an optional one, else its default. */
    private static Object absentValue(MemberInfo m) {
        return m.isOptional && !m.type.isPrimitive() ? null : defaultValue(m.type);
    }

    private static Object decodeMember(Reader r, MemberInfo m) {
        return m.rep != null ? m.rep.decode(r, m.type, m.elementType)
                : decodeValue(r, m.type, m.elementType);
    }

    static void encodeStruct(Writer w, TypeInfo ti, Object sample) {
        if (ti.valueType) {
            encodeValue(w, ti.declaringClass, ti.declaringClass, sample);
            return;
        }
        if (ti.union) {
            encodeUnion(w, ti, sample);
            return;
        }
        switch (ti.extensibility) {
            // §8.6.1.2: EXTENSIBLE is PLAIN_CDR2 like FINAL; the two differ
            // in what a receiver does with a member it does not know.
            case FINAL:
            case EXTENSIBLE: {
                for (MemberInfo m : ti.members) {
                    encodePlainMember(w, m, get(m.field, sample));
                }
                break;
            }
            case APPENDABLE: {
                Writer.DHeader dh = w.writeDheader();
                for (MemberInfo m : ti.members) {
                    encodePlainMember(w, m, get(m.field, sample));
                }
                w.finishDheader(dh);
                break;
            }
            case MUTABLE: {
                Writer.DHeader dh = w.writeDheader();
                for (MemberInfo m : ti.members) {
                    Object value = get(m.field, sample);
                    if (m.isOptional && value == null) {
                        continue; // absent optional → no EMHEADER (V-11 None).
                    }
                    // Length code 4 (a plain NEXTINT) is the universal form:
                    // legal for any member type, so this reflective fallback
                    // does not need a layout plan to pick a narrower one.
                    Writer.EmHeader h = w.writeEmheader(m.memberId, 4, m.mustUnderstand);
                    encodeMember(w, m, value);
                    w.finishEmheader(h);
                }
                w.finishDheader(dh);
                break;
            }
            default:
                throw new XcdrException("unknown extensibility: " + ti.extensibility);
        }
    }

    static Object decodeStruct(Reader r, TypeInfo ti) {
        if (ti.valueType) {
            return decodeValue(r, ti.declaringClass, ti.declaringClass);
        }
        if (ti.union) {
            return decodeUnion(r, ti);
        }
        List<Object> values = new ArrayList<>(ti.members.size());
        switch (ti.extensibility) {
            case FINAL:
            case EXTENSIBLE: {
                for (MemberInfo m : ti.members) {
                    values.add(decodePlainMember(r, m));
                }
                break;
            }
            case APPENDABLE: {
                int size = r.readDheader();
                int end = r.position() + size;
                for (MemberInfo m : ti.members) {
                    if (r.position() >= end) {
                        values.add(absentValue(m)); // trailing member absent.
                    } else {
                        values.add(decodePlainMember(r, m));
                    }
                }
                // Skip any extra appended members we do not know about.
                while (r.position() < end) {
                    r.skip(1);
                }
                break;
            }
            case MUTABLE: {
                int size = r.readDheader();
                int end = r.position() + size;
                // Pre-fill with defaults; absent optionals stay null/default.
                Map<Integer, Object> byId = new LinkedHashMap<>();
                while (r.position() < end) {
                    Reader.EmHeaderRead e = r.readEmheader();
                    MemberInfo m = ti.byMemberId.get(e.memberId());
                    if (m == null) {
                        // Unknown member — skip its body using the length-code.
                        r.skipMember(e);
                        continue;
                    }
                    // Length code 4's NEXTINT is already consumed by
                    // `readEmheader()`; nothing left to skip before the value.
                    byId.put(e.memberId(), decodeMember(r, m));
                }
                for (MemberInfo m : ti.members) {
                    values.add(byId.containsKey(m.memberId)
                            ? byId.get(m.memberId)
                            : absentValue(m));
                }
                break;
            }
            default:
                throw new XcdrException("unknown extensibility: " + ti.extensibility);
        }
        return instantiate(ti, values);
    }

    // ------------------------------------------------------------------
    // Value encode / decode (recursive)
    // ------------------------------------------------------------------

    static void encodeValue(Writer w, Class<?> cls, Type generic, Object value) {
        if (cls == boolean.class || cls == Boolean.class) {
            w.writeBool((Boolean) value);
        } else if (cls == byte.class || cls == Byte.class) {
            w.writeU8(Byte.toUnsignedInt((Byte) value));
        } else if (cls == char.class || cls == Character.class) {
            w.writeChar((Character) value);
        } else if (cls == short.class || cls == Short.class) {
            w.writeI16((Short) value);
        } else if (cls == int.class || cls == Integer.class) {
            w.writeI32((Integer) value);
        } else if (cls == long.class || cls == Long.class) {
            w.writeI64((Long) value);
        } else if (cls == float.class || cls == Float.class) {
            w.writeF32((Float) value);
        } else if (cls == double.class || cls == Double.class) {
            w.writeF64((Double) value);
        } else if (cls == BigInteger.class) {
            // Table 8.2: UInt64 -> long or java.math.BigInteger. A value outside
            // 0 .. 2^64-1 is no UInt64: refused, not truncated by longValue().
            BigInteger big = (BigInteger) value;
            if (big.signum() < 0 || big.bitLength() > 64) {
                throw new XcdrException(big + " is outside the range of UINT_64 (0 to "
                        + "18446744073709551615); Table 8.2 maps BigInteger to UInt64");
            }
            w.writeU64(big.longValue());
        } else if (cls == BigDecimal.class) {
            // Table 8.2: Float128 -> double or java.math.BigDecimal.
            w.writeF128(Float128.fromBigDecimal((BigDecimal) value));
        } else if (cls.isEnum()) {
            // An enumeration is its enumerator value (XTypes 1.3 §7.3.4.5).
            w.writeI32(((Enum<?>) value).ordinal());
        } else if (cls == String.class) {
            NarrowStrings.write(w, (String) value, 0);
        } else if (cls.isArray()) {
            encodeSequence(w, cls.getComponentType(), null, arrayToList(value));
        } else if (Collection.class.isAssignableFrom(cls)) {
            // Table 8.1: java.util.Collection and every array are sequences.
            Class<?> elem = elementClass(generic, 0);
            encodeSequence(w, elem, elementType(generic, 0), asList(value));
        } else if (Map.class.isAssignableFrom(cls)) {
            encodeMap(w, generic, (Map<?, ?>) value);
        } else if (JdkValueTypes.supports(cls)) {
            JdkValueTypes.write(w, cls, value); // §8.1: a Serializable class of the JDK
        } else if (isJdkClass(cls) && JdkSerializedValues.supports(cls)) {
            JdkSerializedValues.write(w, cls, value);
        } else {
            encodeStruct(w, infoOf(cls), value); // nested aggregate.
        }
    }

    static Object decodeValue(Reader r, Class<?> cls, Type generic) {
        if (cls == boolean.class || cls == Boolean.class) {
            return r.readBool();
        } else if (cls == byte.class || cls == Byte.class) {
            return (byte) r.readU8();
        } else if (cls == char.class || cls == Character.class) {
            return (char) r.readChar();
        } else if (cls == short.class || cls == Short.class) {
            return (short) r.readI16();
        } else if (cls == int.class || cls == Integer.class) {
            return r.readI32();
        } else if (cls == long.class || cls == Long.class) {
            return r.readI64();
        } else if (cls == float.class || cls == Float.class) {
            return r.readF32();
        } else if (cls == double.class || cls == Double.class) {
            return r.readF64();
        } else if (cls == BigInteger.class) {
            return new BigInteger(Long.toUnsignedString(r.readU64()));
        } else if (cls == BigDecimal.class) {
            return r.readF128().toBigDecimal();
        } else if (cls.isEnum()) {
            return enumConstant(cls, r.readI32());
        } else if (cls == String.class) {
            return NarrowStrings.read(r, 0);
        } else if (cls.isArray()) {
            List<Object> list = decodeSequence(r, cls.getComponentType(), null);
            return listToArray(cls.getComponentType(), list);
        } else if (Collection.class.isAssignableFrom(cls)) {
            List<Object> values =
                    decodeSequence(r, elementClass(generic, 0), elementType(generic, 0));
            return collection(cls, elementClass(generic, 0), values);
        } else if (Map.class.isAssignableFrom(cls)) {
            return decodeMap(r, cls, generic);
        } else if (JdkValueTypes.supports(cls)) {
            return JdkValueTypes.read(r, cls); // §8.1: a Serializable class of the JDK
        } else if (isJdkClass(cls) && JdkSerializedValues.supports(cls)) {
            return JdkSerializedValues.read(r, cls);
        } else {
            return decodeStruct(r, infoOf(cls)); // nested aggregate.
        }
    }

    @SuppressWarnings({"unchecked", "rawtypes"})
    static Object enumConstant(Class<?> cls, int ordinal) {
        Object[] constants = cls.getEnumConstants();
        if (ordinal < 0 || ordinal >= constants.length) {
            throw new XcdrException("enumerator " + ordinal + " is not a member of "
                    + cls.getName());
        }
        return constants[ordinal];
    }

    /** The elements of a collection or array, in the order §8.5.3 names. */
    private static List<?> asList(Object value) {
        if (value instanceof Collection<?>) {
            Collection<?> collection = (Collection<?>) value;
            return new ArrayList<>(collection);
        }
        return arrayToList(value);
    }

    /**
     * A collection of the class {@code cls} that a field declares, holding
     * {@code values} in the order they were read in (§8.5.3).
     *
     * <p>The class of the field decides what is built. For an interface or
     * abstract class the first of {@code ArrayList}, {@code LinkedHashSet},
     * {@code TreeSet}, {@code ArrayDeque} and {@code LinkedBlockingDeque}
     * that is a {@code cls} is used, so {@code Collection} and {@code List}
     * are an {@code ArrayList}, {@code Set} a {@code LinkedHashSet},
     * {@code SortedSet} and {@code NavigableSet} a {@code TreeSet},
     * {@code Queue} and {@code Deque} an {@code ArrayDeque}; {@code EnumSet}
     * is built over {@code elem}. A concrete class is instantiated through
     * its no-argument constructor. A field type none of these can fill is an
     * {@link XcdrException}: a field the value cannot be set into is not
     * replaced by another collection.
     *
     * @param elem the element class the field declares
     */
    @SuppressWarnings({"unchecked", "rawtypes"})
    static Object collection(Class<?> cls, Class<?> elem, List<Object> values) {
        Collection<Object> built;
        if (cls == EnumSet.class) {
            if (!elem.isEnum()) {
                throw new XcdrException("an EnumSet field needs an enumeration as its element, and "
                        + elem.getName() + " is none");
            }
            built = (Collection<Object>) EnumSet.noneOf((Class) elem);
        } else if (cls.isInterface() || Modifier.isAbstract(cls.getModifiers())) {
            built = null;
            Class<?>[] candidates = {ArrayList.class, LinkedHashSet.class, TreeSet.class,
                    ArrayDeque.class, LinkedBlockingDeque.class};
            for (Class<?> candidate : candidates) {
                if (cls.isAssignableFrom(candidate)) {
                    built = newInstance(candidate);
                    break;
                }
            }
            if (built == null) {
                throw new XcdrException(cls.getName() + " is a collection no standard class"
                        + " implements; a field of this type cannot receive a sequence");
            }
        } else {
            built = newInstance(cls);
        }
        try {
            built.addAll(values);
        } catch (ClassCastException | NullPointerException e) {
            throw new XcdrException("the elements of the sequence cannot be added to a "
                    + built.getClass().getName() + ": " + e.getMessage());
        }
        return built;
    }

    @SuppressWarnings("unchecked")
    private static <C> C newInstance(Class<?> cls) {
        try {
            Constructor<?> ctor = cls.getDeclaredConstructor();
            ctor.setAccessible(true);
            return (C) ctor.newInstance();
        } catch (ReflectiveOperationException | RuntimeException e) {
            throw new XcdrException("cannot instantiate " + cls.getName()
                    + " through a no-argument constructor to receive a sequence or map: " + e);
        }
    }

    static void encodeSequence(Writer w, Class<?> elem, Type elemGeneric, List<?> values) {
        boolean primitive = isPrimitiveElement(elem);
        if (primitive) {
            w.writeU32(values.size());
            for (Object v : values) {
                encodeValue(w, elem, elemGeneric, v);
            }
        } else {
            // Non-primitive elements: DHEADER over [count + elements] (V-6).
            Writer.DHeader dh = w.writeDheader();
            w.writeU32(values.size());
            for (Object v : values) {
                encodeValue(w, elem, elemGeneric, v);
            }
            w.finishDheader(dh);
        }
    }

    static List<Object> decodeSequence(Reader r, Class<?> elem, Type elemGeneric) {
        boolean primitive = isPrimitiveElement(elem);
        List<Object> out = new ArrayList<>();
        if (primitive) {
            int n = r.readSequenceLength();
            for (int i = 0; i < n; i++) {
                out.add(decodeValue(r, elem, elemGeneric));
            }
        } else {
            r.readDheader();
            int n = r.readSequenceLength();
            for (int i = 0; i < n; i++) {
                out.add(decodeValue(r, elem, elemGeneric));
            }
        }
        return out;
    }

    static void encodeMap(Writer w, Type generic, Map<?, ?> map) {
        Class<?> kCls = elementClass(generic, 0);
        Class<?> vCls = elementClass(generic, 1);
        Type kGen = elementType(generic, 0);
        Type vGen = elementType(generic, 1);
        // Map is a non-primitive aggregate → DHEADER over [count + entries].
        Writer.DHeader dh = w.writeDheader();
        w.writeU32(map.size());
        for (Map.Entry<?, ?> e : map.entrySet()) {
            encodeValue(w, kCls, kGen, e.getKey());
            encodeValue(w, vCls, vGen, e.getValue());
        }
        w.finishDheader(dh);
    }

    static Map<Object, Object> decodeMap(Reader r, Class<?> cls, Type generic) {
        Class<?> kCls = elementClass(generic, 0);
        Class<?> vCls = elementClass(generic, 1);
        Type kGen = elementType(generic, 0);
        Type vGen = elementType(generic, 1);
        r.readDheader();
        int n = r.readSequenceLength();
        Map<Object, Object> out = map(cls, kCls);
        for (int i = 0; i < n; i++) {
            Object k = decodeValue(r, kCls, kGen);
            Object v = decodeValue(r, vCls, vGen);
            put(out, k, v);
        }
        return out;
    }

    /**
     * An empty map of the class {@code cls} that a field declares (§8.5.2): for an
     * interface or abstract class the first of {@code LinkedHashMap}, {@code TreeMap}
     * and {@code ConcurrentHashMap} that is a {@code cls} ({@code Map} is a
     * {@code LinkedHashMap}, {@code SortedMap} and {@code NavigableMap} a
     * {@code TreeMap}, {@code ConcurrentMap} a {@code ConcurrentHashMap}); an
     * {@code EnumMap} is built over the key class {@code key}; a concrete class
     * is instantiated through its no-argument constructor.
     */
    @SuppressWarnings({"unchecked", "rawtypes"})
    static Map<Object, Object> map(Class<?> cls, Class<?> key) {
        if (cls == EnumMap.class) {
            if (!key.isEnum()) {
                throw new XcdrException("an EnumMap field needs an enumeration as its key, and "
                        + key.getName() + " is none");
            }
            return new EnumMap((Class) key);
        }
        if (cls.isInterface() || Modifier.isAbstract(cls.getModifiers())) {
            Class<?>[] candidates = {LinkedHashMap.class, TreeMap.class, ConcurrentHashMap.class};
            for (Class<?> candidate : candidates) {
                if (cls.isAssignableFrom(candidate)) {
                    return newInstance(candidate);
                }
            }
            throw new XcdrException(cls.getName() + " is a map no standard class implements;"
                    + " a field of this type cannot receive a map");
        }
        return newInstance(cls);
    }

    static void put(Map<Object, Object> map, Object key, Object value) {
        try {
            map.put(key, value);
        } catch (ClassCastException | NullPointerException e) {
            throw new XcdrException("the entry cannot be put into a " + map.getClass().getName()
                    + ": " + e.getMessage());
        }
    }

    /**
     * Whether an element of a sequence is written inline (no DHEADER around
     * it) or as a delimited aggregate. §8.4's primitives and an
     * enumeration are inline; a string, a collection, a map and a structure
     * are delimited.
     */
    static boolean isPrimitiveElement(Class<?> c) {
        return c == boolean.class || c == Boolean.class
                || c == byte.class || c == Byte.class
                || c == char.class || c == Character.class
                || c == short.class || c == Short.class
                || c == int.class || c == Integer.class
                || c == long.class || c == Long.class
                || c == float.class || c == Float.class
                || c == double.class || c == Double.class
                || c == BigInteger.class || c == BigDecimal.class
                || c.isEnum();
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

    /**
     * Table 8.1 maps {@code String} and the primitive wrappers to DDS types of their own,
     * which are the whole topic type when a topic is declared with one of them: the
     * sample is the value, written as that type (XCDR2), and has no members or key.
     */
    private static boolean isValueType(Class<?> cls) {
        return cls == String.class || cls == Integer.class || cls == Short.class
                || cls == Long.class || cls == Float.class || cls == Double.class
                || cls == Character.class || cls == Byte.class || cls == Boolean.class;
    }

    private static TypeInfo introspect(Class<?> cls) {
        if (isValueType(cls)) {
            TypeInfo value = new TypeInfo();
            value.typeName = cls.getName().replace(".", "::");
            value.extensibility = ExtensibilityKind.FINAL;
            value.members = new ArrayList<>();
            value.declaringClass = cls;
            value.valueType = true;
            return value;
        }
        refuseJdkType(cls, cls, "the topic type " + cls.getName());
        ExtensibilityKind ext = defaultExtensibility(cls);
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
        for (Field f : fieldsOf(cls)) {
            MemberInfo m = new MemberInfo();
            m.field = f;
            m.name = f.getName();
            m.type = f.getType();
            m.elementType = f.getGenericType();
            m.rep = JavaRepresentation.ofField(f, cls.getName() + "." + f.getName());
            if (m.rep == null) {
                refuseJdkType(f.getGenericType(), f.getType(),
                        "the field " + cls.getName() + "." + f.getName());
            }
            m.isKey = annotation(f, "Key") != null;
            m.isOptional = annotation(f, "Optional") != null;
            m.mustUnderstand = annotation(f, "MustUnderstand") != null;
            Object idAnn = annotation(f, "Id");
            if (idAnn != null) {
                Object idv = annotationValue(idAnn, "value");
                if (!(idv instanceof Number)) {
                    throw new XcdrException("@Id on " + cls.getName() + "."
                            + f.getName() + " has no numeric value: " + idv);
                }
                m.memberId = ((Number) idv).intValue();
            } else {
                m.memberId = nextId;
            }
            nextId = m.memberId + 1;
            anyKey = anyKey || m.isKey;
            members.add(m);
        }
        TypeInfo ti = new TypeInfo();
        introspectUnion(cls, members, ti);
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

    /**
     * DDS-Java-PSM 1.0 §8.1 makes every {@code Serializable} type available, and Table 8.1
     * gives the JDK types it lists a DDS type ({@code String}, the wrappers, collections,
     * maps, arrays, {@code Object}). Another class of the JDK ({@code java.util.Date},
     * {@code java.util.UUID}, {@code java.time.*}) is {@code Serializable} but has no entry,
     * and its fields cannot be read reflectively (module {@code java.base} does not open its
     * packages): it is refused with the way out, not with the reflection error of the
     * module system at the first write.
     *
     * @param type the type to check, with its type arguments
     * @param raw the class of {@code type}
     * @param where what has the type, for the message
     */
    private static void refuseJdkType(Type type, Class<?> raw, String where) {
        if (type instanceof ParameterizedType) {
            for (Type argument : ((ParameterizedType) type).getActualTypeArguments()) {
                Class<?> argumentClass = rawClass(argument);
                if (argumentClass != null) {
                    refuseJdkType(argument, argumentClass, where);
                }
            }
        }
        Class<?> c = raw;
        while (c.isArray()) {
            c = c.getComponentType();
        }
        if (c.isPrimitive() || c.isEnum() || c == Object.class || c == String.class
                || c == BigInteger.class || c == BigDecimal.class
                || Collection.class.isAssignableFrom(c) || Map.class.isAssignableFrom(c)
                || JdkValueTypes.supports(c) || !isJdkClass(c)) {
            return;
        }
        if (isTable81Wrapper(c) || JdkSerializedValues.supports(c)) {
            return; // a form of JdkValueTypes or the Java serialization of §8.1
        }
        throw new XcdrException(where + " has the type " + c.getName() + ". It is a class of"
                + " the JDK that is not Serializable and is not a concrete class, and DDS-Java-PSM"
                + " 1.0 Table 8.1 has no DDS type for it. Use a Table 8.1 type in its place.");
    }

    private static boolean isJdkClass(Class<?> c) {
        String name = c.getName();
        return c.getClassLoader() == null || name.startsWith("java.") || name.startsWith("javax.");
    }

    private static boolean isTable81Wrapper(Class<?> c) {
        return c == Integer.class || c == Short.class || c == Long.class || c == Float.class
                || c == Double.class || c == Character.class || c == Byte.class
                || c == Boolean.class;
    }

    private static Class<?> rawClass(Type t) {
        if (t instanceof Class) {
            return (Class<?>) t;
        }
        if (t instanceof ParameterizedType) {
            return (Class<?>) ((ParameterizedType) t).getRawType();
        }
        return null;
    }

    /**
     * §8.6.2: a class annotated {@code @SerializeAs(UNION_TYPE)} is a union.
     * It annotates exactly one field {@code @UnionDiscriminator}, and every
     * other field that is neither transient nor static
     * {@code @UnionMember(discriminatorValue = ...)}; the values are distinct.
     * A class annotated with another kind of structure is refused, as it is
     * not a type of a topic.
     */
    private static void introspectUnion(Class<?> cls, List<MemberInfo> members, TypeInfo ti) {
        org.omg.dds.type.SerializeAs as = cls.getAnnotation(org.omg.dds.type.SerializeAs.class);
        boolean union = as != null && as.value() == org.omg.dds.type.TypeKind.UNION_TYPE;
        if (as != null && !union && as.value() != org.omg.dds.type.TypeKind.STRUCTURE_TYPE) {
            throw new XcdrException(cls.getName() + ": @SerializeAs(" + as.value()
                    + ") on a class; a class is a STRUCTURE_TYPE or a UNION_TYPE");
        }
        for (MemberInfo m : members) {
            boolean marked = m.field.isAnnotationPresent(org.omg.dds.type.UnionDiscriminator.class)
                    || m.field.isAnnotationPresent(org.omg.dds.type.UnionMember.class);
            if (marked && !union) {
                throw new XcdrException(cls.getName() + "." + m.name + " is annotated for a union,"
                        + " but the class is not @SerializeAs(UNION_TYPE) (§8.6.2)");
            }
        }
        if (!union) {
            return;
        }
        MemberInfo discriminator = null;
        java.util.Set<Long> seen = new LinkedHashSet<>();
        for (MemberInfo m : members) {
            org.omg.dds.type.UnionDiscriminator d =
                    m.field.getAnnotation(org.omg.dds.type.UnionDiscriminator.class);
            org.omg.dds.type.UnionMember u =
                    m.field.getAnnotation(org.omg.dds.type.UnionMember.class);
            if (d != null && u != null) {
                throw new XcdrException(cls.getName() + "." + m.name
                        + " is both the discriminator and a member (§8.6.2)");
            }
            if (d != null) {
                if (discriminator != null) {
                    throw new XcdrException(cls.getName() + " has two @UnionDiscriminator fields"
                            + " (§8.6.2: exactly one)");
                }
                Class<?> t = m.type;
                if (!(t == int.class || t == Integer.class || t == short.class || t == Short.class
                        || t == byte.class || t == Byte.class || t == long.class || t == Long.class
                        || t == char.class || t == Character.class || t == boolean.class
                        || t == Boolean.class || t.isEnum())) {
                    throw new XcdrException(cls.getName() + "." + m.name + " is the discriminator"
                            + " and has the type " + t.getName() + "; a discriminator is an"
                            + " integer, character, boolean or enumeration");
                }
                discriminator = m;
                m.discriminator = true;
            } else if (u != null) {
                m.label = u.discriminatorValue();
                if (!seen.add(m.label)) {
                    throw new XcdrException(cls.getName() + " has two members for the"
                            + " discriminator value " + m.label + " (§8.6.2)");
                }
            } else {
                throw new XcdrException(cls.getName() + "." + m.name + " is neither the"
                        + " discriminator nor a @UnionMember (§8.6.2: all other fields must be)");
            }
        }
        if (discriminator == null) {
            throw new XcdrException(cls.getName() + " is a union and has no @UnionDiscriminator"
                    + " field (§8.6.2: exactly one)");
        }
        ti.union = true;
        ti.unionDiscriminator = discriminator;
    }

    /** The discriminator value of a Java value: its integer, character, boolean or enum ordinal. */
    private static long labelOf(Object discriminator) {
        if (discriminator instanceof Boolean) {
            return (Boolean) discriminator ? 1 : 0;
        }
        if (discriminator instanceof Character) {
            return (Character) discriminator;
        }
        if (discriminator instanceof Enum) {
            return ((Enum<?>) discriminator).ordinal();
        }
        return ((Number) discriminator).longValue();
    }

    private static MemberInfo selectedMember(TypeInfo ti, long label) {
        for (MemberInfo m : ti.members) {
            if (!m.discriminator && m.label == label) {
                return m;
            }
        }
        return null;
    }

    /**
     * A union (§8.6.2) is its discriminator and the member that the
     * discriminator selects; a value that selects no member is the
     * discriminator alone. The framing follows the extensibility as a
     * structure's does: APPENDABLE and MUTABLE with a DHEADER, MUTABLE with
     * the discriminator as member 0 and the selected member under its id.
     */
    private static void encodeUnion(Writer w, TypeInfo ti, Object sample) {
        MemberInfo d = ti.unionDiscriminator;
        Object disc = get(d.field, sample);
        if (disc == null) {
            throw new XcdrException(ti.declaringClass.getName() + ": the discriminator is null");
        }
        MemberInfo selected = selectedMember(ti, labelOf(disc));
        Object value = selected == null ? null : get(selected.field, sample);
        if (selected != null && value == null) {
            throw new XcdrException(ti.declaringClass.getName() + "." + selected.name
                    + " is selected by the discriminator " + disc + " and is null");
        }
        switch (ti.extensibility) {
            case FINAL:
            case EXTENSIBLE:
                encodeMember(w, d, disc);
                if (selected != null) {
                    encodeMember(w, selected, value);
                }
                break;
            case APPENDABLE: {
                Writer.DHeader dh = w.writeDheader();
                encodeMember(w, d, disc);
                if (selected != null) {
                    encodeMember(w, selected, value);
                }
                w.finishDheader(dh);
                break;
            }
            case MUTABLE: {
                Writer.DHeader dh = w.writeDheader();
                Writer.EmHeader h = w.writeEmheader(0, 4, true);
                encodeMember(w, d, disc);
                w.finishEmheader(h);
                if (selected != null) {
                    Writer.EmHeader m = w.writeEmheader(selected.memberId, 4,
                            selected.mustUnderstand);
                    encodeMember(w, selected, value);
                    w.finishEmheader(m);
                }
                w.finishDheader(dh);
                break;
            }
            default:
                throw new XcdrException("unknown extensibility: " + ti.extensibility);
        }
    }

    private static Object decodeUnion(Reader r, TypeInfo ti) {
        MemberInfo d = ti.unionDiscriminator;
        Object disc = null;
        Map<Integer, Object> values = new LinkedHashMap<>();
        switch (ti.extensibility) {
            case FINAL:
            case EXTENSIBLE: {
                disc = decodeMember(r, d);
                MemberInfo selected = selectedMember(ti, labelOf(disc));
                if (selected != null) {
                    values.put(selected.memberId, decodeMember(r, selected));
                }
                break;
            }
            case APPENDABLE: {
                int end = r.position() + r.readDheader();
                disc = decodeMember(r, d);
                MemberInfo selected = selectedMember(ti, labelOf(disc));
                if (selected != null && r.position() < end) {
                    values.put(selected.memberId, decodeMember(r, selected));
                }
                while (r.position() < end) {
                    r.skip(1);
                }
                break;
            }
            case MUTABLE: {
                int end = r.position() + r.readDheader();
                Map<Integer, MemberInfo> byId = new LinkedHashMap<>();
                for (MemberInfo m : ti.members) {
                    if (!m.discriminator) {
                        byId.put(m.memberId, m);
                    }
                }
                while (r.position() < end) {
                    Reader.EmHeaderRead e = r.readEmheader();
                    if (e.memberId() == 0) {
                        disc = decodeMember(r, d);
                        continue;
                    }
                    MemberInfo m = byId.get(e.memberId());
                    if (m == null) {
                        r.skipMember(e);
                        continue;
                    }
                    values.put(m.memberId, decodeMember(r, m));
                }
                break;
            }
            default:
                throw new XcdrException("unknown extensibility: " + ti.extensibility);
        }
        if (disc == null) {
            throw new XcdrException(ti.declaringClass.getName() + ": the union has no discriminator");
        }
        List<Object> ordered = new ArrayList<>(ti.members.size());
        for (MemberInfo m : ti.members) {
            ordered.add(m.discriminator ? disc
                    : values.containsKey(m.memberId) ? values.get(m.memberId)
                            : defaultValue(m.type));
        }
        return instantiate(ti, ordered);
    }

    /**
     * §8.6.1.2: FINAL if the class extends {@code java.lang.Object}
     * directly and is final, EXTENSIBLE in all other cases; MUTABLE only if
     * the class says so.
     */
    private static ExtensibilityKind defaultExtensibility(Class<?> cls) {
        return Modifier.isFinal(cls.getModifiers())
                && cls.getSuperclass() == Object.class
                ? ExtensibilityKind.FINAL
                : ExtensibilityKind.APPENDABLE; // EXTENSIBLE of §8.6.1.2 (XTypes 1.3: APPENDABLE)
    }

    /**
     * The fields of {@code cls} in the order §8.6.1.1 puts them on the wire:
     * the ones of the superclass first (Java class extension maps to
     * structure inheritance), then its own. {@code java.lang.Object} and
     * transient, static or synthetic fields carry no member.
     */
    private static List<Field> fieldsOf(Class<?> cls) {
        List<Class<?>> hierarchy = new ArrayList<>();
        for (Class<?> c = cls; c != null && c != Object.class;
                c = c.getSuperclass()) {
            hierarchy.add(0, c); // base first.
        }
        List<Field> fields = new ArrayList<>();
        for (Class<?> c : hierarchy) {
            if (!serializesFieldsOf(cls, c)) {
                continue;
            }
            for (Field f : c.getDeclaredFields()) {
                int mod = f.getModifiers();
                if (Modifier.isStatic(mod) || Modifier.isTransient(mod)
                        || f.isSynthetic()) {
                    continue;
                }
                try {
                    f.setAccessible(true);
                } catch (RuntimeException e) {
                    // SecurityException, or the module system refusing access
                    // (InaccessibleObjectException since Java 9).
                    throw new XcdrException("cannot read the field " + c.getName()
                            + "." + f.getName() + ": " + e.getMessage());
                }
                fields.add(f);
            }
        }
        return fields;
    }

    /**
     * Whether the fields of {@code ancestor}, a class {@code cls} extends, are part of the type
     * (DDS-Java-PSM 1.0 8.6.1.1, "subject to the restrictions documented by the
     * java.io.Serializable interface"): of a Serializable class the fields of a base class that
     * is not Serializable are not serialized, since that base is initialized by its no-argument
     * constructor. A class that is not Serializable keeps all of its fields.
     */
    static boolean serializesFieldsOf(Class<?> cls, Class<?> ancestor) {
        return ancestor == cls || !java.io.Serializable.class.isAssignableFrom(cls)
                || java.io.Serializable.class.isAssignableFrom(ancestor);
    }

    private static ExtensibilityKind mapExtensibility(String name) {
        String n = name.toUpperCase();
        if (n.contains("APPENDABLE")) {
            return ExtensibilityKind.APPENDABLE;
        }
        if (n.contains("MUTABLE")) {
            return ExtensibilityKind.MUTABLE;
        }
        if (n.contains("EXTENSIBLE")) {
            return ExtensibilityKind.APPENDABLE;
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
        if (cls == BigInteger.class) {
            return BigInteger.ZERO;
        }
        if (cls == BigDecimal.class) {
            return BigDecimal.ZERO;
        }
        if (JdkValueTypes.supports(cls) && JdkValueTypes.defaultOf(cls) != null) {
            return JdkValueTypes.defaultOf(cls);
        }
        if (cls.isEnum()) {
            // §7.4.3.5: the first enumerator of a member that is absent.
            Object[] constants = cls.getEnumConstants();
            return constants.length == 0 ? null : constants[0];
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

    /**
     * The annotation on {@code el} that is one of the type-system
     * annotations, or {@code null}.
     *
     * <p>§8.3: those annotations are the ones of {@code org.omg.dds.type};
     * §8.9: a Java annotation is ignored unless it is one of them, so a
     * {@code jakarta.persistence.Id} or any other {@code @Key} of another
     * package is not metadata of the DDS type system. A type that wants an
     * annotation represented has to say so with {@code @SerializeAs}.
     */
    private static Object annotation(AnnotatedElement el, String simpleName) {
        for (Annotation a : el.getDeclaredAnnotations()) {
            String name = a.annotationType().getName();
            if ((name.startsWith(PSM_TYPE_PACKAGE + ".")
                    || name.startsWith(RUNTIME_TYPE_PACKAGE + "."))
                    && a.annotationType().getSimpleName().equalsIgnoreCase(simpleName)) {
                return a;
            }
        }
        return null;
    }

    /** The package the PSM keeps its type annotations in (§8.3). */
    private static final String PSM_TYPE_PACKAGE = "org.omg.dds.type";

    /**
     * The package the ZeroDDS IDL-Java runtime writes the same annotations
     * in; {@code ReflectionTypeSupport} honours both so that generated and
     * hand-written types map alike.
     */
    private static final String RUNTIME_TYPE_PACKAGE = "org.zerodds.types";

    private static Object annotationValue(Object annotation, String method) {
        try {
            return annotation.getClass().getMethod(method).invoke(annotation);
        } catch (ReflectiveOperationException e) {
            return null;
        }
    }

    static Class<?> elementClass(Type generic, int index) {
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

    static Type elementType(Type generic, int index) {
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

    // ------------------------------------------------------------------
    // Members by name (for the MultiTopic row model)
    // ------------------------------------------------------------------

    /**
     * One member of a Java aggregate as the type representation of
     * DDS-Java-PSM 1.0 §8 sees it: its name, its Java type, whether it is a
     * key member, and access to its value.
     */
    public static final class MemberRef {
        private final MemberInfo info;

        MemberRef(MemberInfo info) {
            this.info = info;
        }

        /** The member name (the Java field name). */
        public String getName() {
            return info.name;
        }

        /** The Java type of the member. */
        public Class<?> getType() {
            return info.type;
        }

        /** Whether the member is part of the key of its aggregate. */
        public boolean isKey() {
            return info.isKey;
        }

        /** The value of the member in {@code aggregate}. */
        public Object get(Object aggregate) {
            return ReflectionTypeSupport.get(info.field, aggregate);
        }
    }

    /**
     * The members of {@code cls} in wire order (§8.6.1.1: members of a base
     * class first).
     *
     * @throws XcdrException if a member cannot be read.
     */
    public static List<MemberRef> membersOf(Class<?> cls) {
        List<MemberRef> out = new ArrayList<>();
        for (MemberInfo m : infoOf(cls).members) {
            out.add(new MemberRef(m));
        }
        return out;
    }

    /**
     * Builds an instance of {@code cls} from member values by name, the way
     * a decoded sample is built: through the no-argument constructor and the
     * fields, or through the canonical constructor. A member that has no
     * entry in {@code values} takes the default value of its type
     * (§7.4.3.5: zero, the first enumerator, {@code null}).
     *
     * @throws XcdrException if {@code values} names a member {@code cls}
     *         does not have, or the class cannot be instantiated.
     */
    public static <T> T assemble(Class<T> cls, Map<String, Object> values) {
        TypeInfo ti = infoOf(cls);
        Set<String> known = new LinkedHashSet<>();
        List<Object> ordered = new ArrayList<>(ti.members.size());
        for (MemberInfo m : ti.members) {
            known.add(m.name);
            ordered.add(values.containsKey(m.name)
                    ? values.get(m.name) : defaultValue(m.type));
        }
        for (String name : values.keySet()) {
            if (!known.contains(name)) {
                throw new XcdrException(cls.getName() + " has no member '" + name + "'");
            }
        }
        return cls.cast(instantiate(ti, ordered));
    }

    static final class TypeInfo {
        String typeName;
        ExtensibilityKind extensibility;
        List<MemberInfo> members;
        boolean hasKey;
        /** The class is a union (§8.6.2). */
        boolean union;
        /** The class is {@code String} or a primitive wrapper: the sample is the value itself. */
        boolean valueType;
        MemberInfo unionDiscriminator;
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
        /** What @SerializeAs states for the member (§8.2), or null for the defaults of Table 8.1. */
        JavaRepresentation rep;
        /** The field is the discriminator of a union (§8.6.2). */
        boolean discriminator;
        /** The discriminator value a union member is selected by (§8.6.2). */
        long label;
    }
}
