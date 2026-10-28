// SPDX-License-Identifier: Apache-2.0
package org.zerodds.cdr;

import org.omg.dds.core.ServiceEnvironment;
import org.zerodds.core.PreconditionNotMetException;
import org.omg.dds.type.SerializeAs;
import org.omg.dds.type.TypeKind;
import org.omg.dds.type.dynamic.AnnotationDescriptor;
import org.omg.dds.type.dynamic.DynamicType;
import org.omg.dds.type.dynamic.DynamicTypeMember;
import org.omg.dds.type.dynamic.DynamicTypeFactory;
import org.omg.dds.type.dynamic.MemberDescriptor;
import org.omg.dds.type.dynamic.TypeDescriptor;

import java.util.ArrayList;
import java.util.Collection;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

/**
 * ZeroDDS {@link DynamicTypeFactory} (DDS-Java-PSM 1.0 §7.8.1.1).
 *
 * <p>§7.8.1.1 makes this a per-{@link ServiceEnvironment} singleton; the
 * instance lives in the environment's SPI, so two environments never share
 * their types.
 *
 * <p>{@link #createType(Class)} builds a type from the Java Type
 * Representation (Clause 8) by reflection, as §7.8.1.3 requires. The remaining
 * operations build the structured types [DDS-XTypes] defines out of primitive
 * ones. {@link #loadType(String)} resolves a document URL to a topic type of
 * the Java Type Representation — the document names the generated or
 * hand-written Java class — and returns its {@link DynamicType}.
 */
public final class DynamicTypeFactoryImpl extends DynamicTypeFactory {

    private final ServiceEnvironment environment;
    private final Map<TypeKind, DynamicType> primitives = new LinkedHashMap<>();

    public DynamicTypeFactoryImpl(ServiceEnvironment environment) {
        this.environment = environment;
        for (TypeKind kind : TypeKind.values()) {
            if (kind.ordinal() <= TypeKind.CHAR_32_TYPE.ordinal()) {
                primitives.put(kind, build(null, kind, null, null, new int[0]));
            }
        }
    }

    /**
     * The type factory of {@code env} as this implementation, which has the
     * {@link #createType(Class)} that DDS-Java-PSM 1.0 §7.8.1.3 adds to the factory.
     * {@code org.omg.dds.type.dynamic.DynamicTypeFactory} is the unchanged OMG class and
     * does not declare that method; the factory this returns is the one
     * {@link DynamicTypeFactory#getInstance(ServiceEnvironment)} returns.
     *
     * @throws ClassCastException if {@code env} belongs to another implementation
     */
    public static DynamicTypeFactoryImpl getInstance(ServiceEnvironment env) {
        return (DynamicTypeFactoryImpl) DynamicTypeFactory.getInstance(env);
    }

    @Override
    public ServiceEnvironment getEnvironment() {
        return environment;
    }

    @Override
    public DynamicType getPrimitiveType(TypeKind kind) {
        DynamicType type = primitives.get(kind);
        if (type == null) {
            throw new IllegalArgumentException(kind + " is not a primitive TypeKind");
        }
        return type;
    }

    @Override
    public DynamicType createType(TypeDescriptor descriptor) {
        if (descriptor == null) {
            throw new IllegalArgumentException("a TypeDescriptor must not be null");
        }
        return new DynamicTypeImpl(environment, descriptor);
    }

    @Override
    public DynamicType createStringType(int bound) {
        return build("String", TypeKind.STRING_TYPE, null, null, new int[]{bound});
    }

    @Override
    public DynamicType createWStringType(int bound) {
        return build("WString", TypeKind.STRING_TYPE, null, null, new int[]{bound});
    }

    @Override
    public DynamicType createSequenceType(DynamicType elementType) {
        return build(null, TypeKind.SEQUENCE_TYPE, elementType, null, new int[0]);
    }

    @Override
    public DynamicType createSequenceType(DynamicType elementType, int bound) {
        return build(null, TypeKind.SEQUENCE_TYPE, elementType, null, new int[]{bound});
    }

    @Override
    public DynamicType createArrayType(DynamicType elementType, int... bound) {
        return build(null, TypeKind.ARRAY_TYPE, elementType, null, bound);
    }

    @Override
    public DynamicType createMapType(DynamicType keyElementType, DynamicType elementType, int bound) {
        return build(null, TypeKind.MAP_TYPE, elementType, keyElementType, new int[]{bound});
    }

    @Override
    public DynamicType createBitSetType(int bound) {
        return build("BitSet", TypeKind.BITSET_TYPE, null, null, new int[]{bound});
    }

    @Override
    public DynamicType loadType(String documentUrl) {
        return loadType(documentUrl, null);
    }

    @Override
    public Collection<DynamicType> loadTypes(String documentUrl) {
        DynamicType type = loadType(documentUrl);
        Collection<DynamicType> types = new ArrayList<>();
        types.add(type);
        return types;
    }

    @Override
    public DynamicType loadType(String documentUrl, String name) {
        Class<?> type = resolve(documentUrl, name);
        return createType(type);
    }

    @Override
    public Collection<DynamicType> loadTypes(String documentUrl, String... names) {
        Collection<DynamicType> types = new ArrayList<>();
        for (String name : names) {
            types.add(loadType(documentUrl, name));
        }
        return types;
    }

    /**
     * §7.8.1.3: "inspect the given type reflectively in accordance with the
     * Java Type Representation and instantiate an equivalent DynamicType
     * object."
     *
     * <p>The kind of a member follows Table 8.1 ({@code int} INT_32,
     * {@code short} INT_16, {@code long} INT_64, {@code float} FLOAT_32,
     * {@code double} FLOAT_64, {@code char} CHAR_8, {@code byte} BYTE,
     * {@code boolean} BOOLEAN, the boxed types alike, {@code String} a
     * string, {@code Map} a map, {@code Collection} and arrays a sequence,
     * any other class a structure, an enumeration an enumeration), or what
     * {@code @SerializeAs} and {@code @BitSet} (§8.2, §8.4 to §8.7) put in its
     * place. A class marked {@code @SerializeAs(UNION_TYPE)} is a union
     * (§8.6.2); an annotation type marked {@code @SerializeAs(ANNOTATION_TYPE)}
     * is an annotation type (§8.9). Extensibility, keys and optional members
     * are the annotations {@code Extensibility}, {@code Key},
     * {@code Optional} and {@code MustUnderstand} of the type.
     *
     * @throws IllegalArgumentException if a member's Java type maps to no
     *     {@link TypeKind} of Clause 8.
     */
    public DynamicType createType(Class<?> type) {
        if (type == null) {
            throw new IllegalArgumentException("a Class to inspect must not be null");
        }
        try {
            return reflect(type, new ArrayList<Class<?>>());
        } catch (XcdrException e) {
            throw new IllegalArgumentException(e.getMessage(), e);
        }
    }

    private DynamicType reflect(Class<?> type, List<Class<?>> inProgress) {
        if (inProgress.contains(type)) {
            throw new IllegalArgumentException(type.getName() + " contains itself; the DDS Type"
                    + " System has no cycles (§8.6)");
        }
        inProgress.add(type);
        try {
            if (type.isAnnotation()) {
                return reflectAnnotationType(type, inProgress);
            }
            ReflectionTypeSupport.TypeInfo info = ReflectionTypeSupport.infoOf(type);
            TypeDescriptorImpl descriptor = (TypeDescriptorImpl) new TypeDescriptorImpl(
                    environment, info.union ? TypeKind.UNION_TYPE : TypeKind.STRUCTURE_TYPE)
                    .withName(info.typeName);
            if (info.union) {
                descriptor = (TypeDescriptorImpl) descriptor.withDiscriminatorType(
                        typeOf(info.unionDiscriminator, inProgress));
            }
            // §8.6.1.1: class extension is structure inheritance. The superclass is the
            // base type, and the type lists only the members its own class declares.
            Class<?> superclass = type.getSuperclass();
            if (!info.union && superclass != null && superclass != Object.class
                    && ReflectionTypeSupport.serializesFieldsOf(type, superclass)) {
                descriptor = (TypeDescriptorImpl) descriptor.withBaseType(
                        reflect(superclass, inProgress));
            }
            DynamicTypeImpl dynamicType = new DynamicTypeImpl(environment, descriptor);
            dynamicType.setAnnotation(annotation("Extensibility", info.extensibility.name()));
            for (ReflectionTypeSupport.MemberInfo member : info.members) {
                if (member.discriminator || member.field.getDeclaringClass() != type) {
                    continue;
                }
                MemberDescriptorImpl md = new MemberDescriptorImpl(environment, member.name,
                        member.memberId, typeOf(member, inProgress));
                MemberDescriptor described = info.union ? md.withLabel((int) member.label) : md;
                DynamicTypeMember added = dynamicType.addMember(described);
                // setAnnotation(s) replaces what a member has, so they go in together
                List<AnnotationDescriptor> marks = new ArrayList<>();
                if (member.isKey) {
                    marks.add(annotation("Key", null));
                }
                if (member.isOptional) {
                    marks.add(annotation("Optional", null));
                }
                if (member.mustUnderstand) {
                    marks.add(annotation("MustUnderstand", null));
                }
                if (!marks.isEmpty()) {
                    added.setAnnotations(marks.toArray(new AnnotationDescriptor[0]));
                }
            }
            return dynamicType;
        } finally {
            inProgress.remove(type);
        }
    }

    /**
     * §8.9: "This Type Representation ignores Java annotation types by
     * default. Java annotations that are intended to be represented
     * explicitly within the DDS Type System must be so annotated with
     * &#64;SerializeAs." An annotation type has a member per element.
     */
    private DynamicType reflectAnnotationType(Class<?> type, List<Class<?>> inProgress) {
        SerializeAs as = type.getAnnotation(SerializeAs.class);
        if (as == null || as.value() != TypeKind.ANNOTATION_TYPE) {
            throw new IllegalArgumentException(type.getName() + " is a Java annotation type and"
                    + " is not represented unless it is marked @SerializeAs(ANNOTATION_TYPE)"
                    + " (§8.9)");
        }
        DynamicTypeImpl dynamicType = new DynamicTypeImpl(environment,
                new TypeDescriptorImpl(environment, TypeKind.ANNOTATION_TYPE)
                        .withName(type.getName().replace('$', '.').replace(".", "::")));
        int id = 1;
        for (java.lang.reflect.Method element : type.getDeclaredMethods()) {
            if (element.getParameterCount() != 0 || element.isSynthetic()) {
                continue;
            }
            MemberDescriptor md = new MemberDescriptorImpl(environment, element.getName(), id++,
                    typeOfClass(element.getReturnType(), element.getGenericReturnType(), null,
                            inProgress));
            Object fallback = element.getDefaultValue();
            dynamicType.addMember(fallback == null ? md : md.withDefaultValue(fallback.toString()));
        }
        return dynamicType;
    }

    private AnnotationDescriptor annotation(String name, String value) {
        DynamicType type = new DynamicTypeImpl(environment,
                new TypeDescriptorImpl(environment, TypeKind.ANNOTATION_TYPE).withName(name));
        AnnotationDescriptor descriptor =
                new AnnotationDescriptorImpl(environment, type);
        return value == null ? descriptor : descriptor.withValue("value", value);
    }

    private DynamicType typeOf(ReflectionTypeSupport.MemberInfo member, List<Class<?>> inProgress) {
        return typeOfClass(member.type, member.elementType, member.rep, inProgress);
    }

    /** The type of a Java type under a representation ({@code null}: Table 8.1). */
    private DynamicType typeOfClass(Class<?> cls, java.lang.reflect.Type generic,
            JavaRepresentation rep, List<Class<?>> inProgress) {
        TypeKind kind = rep == null ? null : rep.kind;
        if (kind != null && kind.ordinal() <= TypeKind.CHAR_32_TYPE.ordinal()) {
            return getPrimitiveType(kind);
        }
        if (kind == null || kind == TypeKind.STRUCTURE_TYPE || kind == TypeKind.UNION_TYPE) {
            TypeKind primitive = defaultKind(cls);
            if (primitive != null) {
                return getPrimitiveType(primitive);
            }
        }
        if (kind == TypeKind.BITSET_TYPE) {
            return createBitSetType((int) rep.bound[0]);
        }
        if (kind == TypeKind.ENUMERATION_TYPE || (kind == null && cls.isEnum())) {
            return enumeration(cls);
        }
        if (kind == TypeKind.STRING_TYPE || (kind == null && cls == String.class)) {
            int bound = rep != null && rep.bound.length == 1 ? (int) rep.bound[0] : 0;
            return rep != null && rep.element == TypeKind.CHAR_32_TYPE
                    ? createWStringType(bound) : createStringType(bound);
        }
        if (kind == TypeKind.MAP_TYPE || (kind == null && Map.class.isAssignableFrom(cls))) {
            java.lang.reflect.Type[] args = typeArguments(generic, 2, cls);
            int bound = rep != null && rep.bound.length == 1 ? (int) rep.bound[0] : 0;
            return createMapType(
                    typeOfClass(raw(args[0]), args[0], rep == null ? null : rep.keyRep(),
                            inProgress),
                    typeOfClass(raw(args[1]), args[1], rep == null ? null : rep.elementRep(),
                            inProgress), bound);
        }
        boolean collection = Collection.class.isAssignableFrom(cls) || cls.isArray();
        if (kind == TypeKind.SEQUENCE_TYPE || kind == TypeKind.ARRAY_TYPE
                || (kind == null && collection)) {
            boolean array = kind == TypeKind.ARRAY_TYPE;
            int dimensions = array ? rep.bound.length : 1;
            java.lang.reflect.Type element = generic;
            Class<?> elementClass = cls;
            if (cls.isArray()) {
                for (int i = 0; i < dimensions && elementClass.isArray(); i++) {
                    element = element instanceof java.lang.reflect.GenericArrayType
                            ? ((java.lang.reflect.GenericArrayType) element)
                                    .getGenericComponentType()
                            : elementClass.getComponentType();
                    elementClass = raw(element);
                }
            } else {
                element = typeArguments(generic, 1, cls)[0];
                elementClass = raw(element);
            }
            DynamicType elementType = typeOfClass(elementClass, element,
                    rep == null ? null : rep.elementRep(), inProgress);
            if (array) {
                int[] bound = new int[rep.bound.length];
                for (int i = 0; i < bound.length; i++) {
                    bound[i] = (int) rep.bound[i];
                }
                return createArrayType(elementType, bound);
            }
            return rep != null && rep.bound.length == 1
                    ? createSequenceType(elementType, (int) rep.bound[0])
                    : createSequenceType(elementType);
        }
        if (cls.isPrimitive() || cls.isInterface() || cls == Object.class) {
            throw new IllegalArgumentException("the Java type " + cls.getName()
                    + " maps to no TypeKind of DDS-Java-PSM 1.0 Clause 8 (Table 8.1)");
        }
        return reflect(cls, inProgress);
    }

    private DynamicType enumeration(Class<?> cls) {
        if (!cls.isEnum()) {
            throw new IllegalArgumentException(cls.getName() + " is not an enumeration (§8.7)");
        }
        DynamicTypeImpl type = new DynamicTypeImpl(environment,
                new TypeDescriptorImpl(environment, TypeKind.ENUMERATION_TYPE)
                        .withName(cls.getName().replace('$', '.').replace(".", "::")));
        for (Object constant : cls.getEnumConstants()) {
            Enum<?> e = (Enum<?>) constant;
            type.addMember(new MemberDescriptorImpl(environment, e.name(), e.ordinal(), null));
        }
        return type;
    }

    /** Table 8.1: the kind of a Java primitive or its box, or {@code null}. */
    private static TypeKind defaultKind(Class<?> cls) {
        if (cls == int.class || cls == Integer.class) {
            return TypeKind.INT_32_TYPE;
        }
        if (cls == short.class || cls == Short.class) {
            return TypeKind.INT_16_TYPE;
        }
        if (cls == long.class || cls == Long.class) {
            return TypeKind.INT_64_TYPE;
        }
        if (cls == float.class || cls == Float.class) {
            return TypeKind.FLOAT_32_TYPE;
        }
        if (cls == double.class || cls == Double.class) {
            return TypeKind.FLOAT_64_TYPE;
        }
        if (cls == char.class || cls == Character.class) {
            return TypeKind.CHAR_8_TYPE;
        }
        if (cls == byte.class || cls == Byte.class) {
            return TypeKind.BYTE_TYPE;
        }
        if (cls == boolean.class || cls == Boolean.class) {
            return TypeKind.BOOLEAN_TYPE;
        }
        return null;
    }

    private static java.lang.reflect.Type[] typeArguments(java.lang.reflect.Type generic, int count,
            Class<?> cls) {
        if (generic instanceof java.lang.reflect.ParameterizedType) {
            java.lang.reflect.Type[] args =
                    ((java.lang.reflect.ParameterizedType) generic).getActualTypeArguments();
            if (args.length == count) {
                return args;
            }
        }
        throw new IllegalArgumentException("the element type of " + cls.getName()
                + " is not stated; declare it with type arguments (§8.5)");
    }

    private static Class<?> raw(java.lang.reflect.Type type) {
        if (type instanceof Class) {
            return (Class<?>) type;
        }
        if (type instanceof java.lang.reflect.ParameterizedType) {
            return (Class<?>) ((java.lang.reflect.ParameterizedType) type).getRawType();
        }
        if (type instanceof java.lang.reflect.GenericArrayType) {
            return java.lang.reflect.Array.newInstance(
                    raw(((java.lang.reflect.GenericArrayType) type).getGenericComponentType()), 0)
                    .getClass();
        }
        throw new IllegalArgumentException("the Java type " + type
                + " maps to no TypeKind of DDS-Java-PSM 1.0 Clause 8");
    }

    @Override
    public TypeDescriptor newTypeDescriptor() {
        return new TypeDescriptorImpl(environment, TypeKind.STRUCTURE_TYPE);
    }

    @Override
    public MemberDescriptor newMemberDescriptor() {
        return new MemberDescriptorImpl(environment, null, 0, null);
    }

    @Override
    public AnnotationDescriptor newAnnotationDescriptor() {
        return new AnnotationDescriptorImpl(environment, null);
    }

    /**
     * The Java class a document URL names: the {@code name} argument when given,
     * else the last path or package segment of the URL.
     */
    private Class<?> resolve(String documentUrl, String name) {
        if (name == null) {
            if (documentUrl == null || documentUrl.isEmpty()) {
                throw new PreconditionNotMetException(
                        "loadType needs a document URL or a type name");
            }
            String trimmed = documentUrl;
            int cut = Math.max(trimmed.lastIndexOf('/'), trimmed.lastIndexOf('\\'));
            if (cut >= 0) {
                trimmed = trimmed.substring(cut + 1);
            }
            int dot = trimmed.lastIndexOf('.');
            name = dot >= 0 ? trimmed.substring(0, dot) : trimmed;
        }
        try {
            return Class.forName(name, false, classLoader());
        } catch (ClassNotFoundException e) {
            throw new PreconditionNotMetException(
                    "no Java topic type named '" + name + "' for document " + documentUrl, e);
        }
    }

    private ClassLoader classLoader() {
        ClassLoader loader = Thread.currentThread().getContextClassLoader();
        return loader != null ? loader : DynamicTypeFactoryImpl.class.getClassLoader();
    }

    private DynamicType build(String name, TypeKind kind, DynamicType element,
            DynamicType keyElement, int[] bound) {
        TypeDescriptorImpl descriptor =
                (TypeDescriptorImpl) new TypeDescriptorImpl(environment, kind).withKind(kind);
        if (name != null) {
            descriptor = (TypeDescriptorImpl) descriptor.withName(name);
        }
        if (element != null) {
            descriptor = (TypeDescriptorImpl) descriptor.withElementType(element);
        }
        if (keyElement != null) {
            descriptor = (TypeDescriptorImpl) descriptor.withKeyElementType(keyElement);
        }
        if (bound.length > 0) {
            descriptor = (TypeDescriptorImpl) descriptor.withBound(bound);
        }
        return new DynamicTypeImpl(environment, descriptor);
    }
}