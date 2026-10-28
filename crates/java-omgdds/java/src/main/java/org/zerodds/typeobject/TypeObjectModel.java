package org.zerodds.typeobject;

import java.util.ArrayList;
import java.util.Collections;
import java.util.HashMap;
import java.util.List;
import java.util.Map;

import org.omg.dds.core.ServiceEnvironment;
import org.omg.dds.type.TypeKind;
import org.omg.dds.type.typeobject.AliasType;
import org.omg.dds.type.typeobject.AnnotationMember;
import org.omg.dds.type.typeobject.AnnotationMemberValue;
import org.omg.dds.type.typeobject.AnnotationType;
import org.omg.dds.type.typeobject.AnnotationUsage;
import org.omg.dds.type.typeobject.AnnotationUsageMember;
import org.omg.dds.type.typeobject.ArrayType;
import org.omg.dds.type.typeobject.Bit;
import org.omg.dds.type.typeobject.BitSetType;
import org.omg.dds.type.typeobject.EnumeratedConstant;
import org.omg.dds.type.typeobject.EnumerationType;
import org.omg.dds.type.typeobject.MapType;
import org.omg.dds.type.typeobject.Member;
import org.omg.dds.type.typeobject.MemberFlag;
import org.omg.dds.type.typeobject.MemberProperty;
import org.omg.dds.type.typeobject.Module;
import org.omg.dds.type.typeobject.SequenceType;
import org.omg.dds.type.typeobject.StringType;
import org.omg.dds.type.typeobject.StructureType;
import org.omg.dds.type.typeobject.Type;
import org.omg.dds.type.typeobject.TypeFlag;
import org.omg.dds.type.typeobject.TypeLibrary;
import org.omg.dds.type.typeobject.TypeLibraryElement;
import org.omg.dds.type.typeobject.TypeObject;
import org.omg.dds.type.typeobject.TypeProperty;
import org.omg.dds.type.typeobject.UnionMember;
import org.omg.dds.type.typeobject.UnionType;
import org.zerodds.typeobject.generated.ZdTypes;

/**
 * The {@code org.omg.dds.type.typeobject} interfaces of DDS-Java-PSM 1.0 over a decoded
 * XTypes 1.1 TypeObject.
 *
 * <p>Mapping of what the interfaces cannot hold as stated in Annex B:
 * <ul>
 * <li>A type id is an {@code int} in the interfaces and a 64-bit hash on the wire. The id of a
 *   primitive type is the {@code TypeKind.Values} constant of the PSM ({@code TypeId}); the id
 *   of a constructed type is {@link #CONSTRUCTED_BASE} plus the index of its element in
 *   {@code getLibrary().getElement()}. {@link #element(TypeLibrary, int)} resolves it.
 * <li>{@code TypeObject.getTheType()} is the id of the first entry of {@code the_type}
 *   (Annex B has a sequence of ids, the interface one id), 0 (NO_TYPE) if there is none.
 * <li>{@code getFlag()} of a type or member property returns the flag with the lowest bit set,
 *   or {@code null}; {@link #typeFlags(TypeProperty)} and {@link #memberFlags(MemberProperty)}
 *   return the bits (the Annex B holder is a bit set, the interface type is a single enum).
 * <li>{@code AnnotationMemberValue.value()} of a FLOAT_128_TYPE is the binary128 bit pattern as a
 *   {@code BigInteger}; the interface has no 128-bit float type.
 * <li>Unsigned wire values ({@code unsigned long}) are cast to {@code int}, as the interfaces
 *   declare, or kept as {@code long} where the interface says {@code long}.
 * </ul>
 */
public final class TypeObjectModel {

    /** Added to the library index of a constructed type to give its {@code int} id. */
    public static final int CONSTRUCTED_BASE = 0x10000;

    private final ServiceEnvironment env;
    private final Map<Long, Integer> indexOf = new HashMap<Long, Integer>();

    private TypeObjectModel(ServiceEnvironment env) {
        this.env = env;
    }

    /** The interfaces over {@code object}. */
    public static TypeObject of(ServiceEnvironment env, ZdTypes.DDS.TypeObject object) {
        TypeObjectModel m = new TypeObjectModel(env);
        ZdTypes.DDS.TypeLibraryElement[] elements = object.library == null || object.library.element == null
                ? new ZdTypes.DDS.TypeLibraryElement[0] : object.library.element;
        for (int i = 0; i < elements.length; i++) {
            if (elements[i]._d != ZdTypes.DDS.TypeLibraryElementKind.MODULE_ELEMENT) {
                m.indexOf.put(TypeObjectCodec.propertyOf(elements[i]).type_id.constructed_type_id, i);
            }
        }
        List<TypeLibraryElement> list = new ArrayList<TypeLibraryElement>();
        for (ZdTypes.DDS.TypeLibraryElement element : elements) {
            list.add(m.element(element));
        }
        final TypeLibrary library = new Library(env, list);
        int root = object.the_type == null || object.the_type.length == 0
                ? 0 : m.id(object.the_type[0]);
        return new TypeObjectImpl(env, library, root);
    }

    /** The element a constructed type id of this model names, or {@code null}. */
    public static TypeLibraryElement element(TypeLibrary library, int id) {
        int index = id - CONSTRUCTED_BASE;
        List<TypeLibraryElement> all = library.getElement();
        return index >= 0 && index < all.size() ? all.get(index) : null;
    }

    /** The flag bits of {@code property}, as the Annex B holder states them. */
    public static int typeFlags(TypeProperty property) {
        return ((TypePropertyImpl) property).flagBits;
    }

    /** The flag bits of {@code property}, as the Annex B holder states them. */
    public static int memberFlags(MemberProperty property) {
        return ((MemberPropertyImpl) property).flagBits;
    }

    // -------------------------------------------------------------------------------------

    private int id(ZdTypes.DDS.TypeId wire) {
        if (TypeObjectCodec.isPrimitive(wire._d)) {
            return primitiveId(wire._d);
        }
        if (wire._d == ZdTypes.DDS.NO_TYPE && wire.constructed_type_id == 0L) {
            return 0;
        }
        Integer index = indexOf.get(wire.constructed_type_id);
        return index == null ? 0 : CONSTRUCTED_BASE + index;
    }

    private static int primitiveId(short kind) {
        switch (kind) {
            case ZdTypes.DDS.BOOLEAN_TYPE: return TypeKind.Values.BOOLEAN_TYPE_VALUE;
            case ZdTypes.DDS.BYTE_TYPE: return TypeKind.Values.BYTE_TYPE_VALUE;
            case ZdTypes.DDS.INT_16_TYPE: return TypeKind.Values.INT_16_TYPE_VALUE;
            case ZdTypes.DDS.UINT_16_TYPE: return TypeKind.Values.UINT_16_TYPE_VALUE;
            case ZdTypes.DDS.INT_32_TYPE: return TypeKind.Values.INT_32_TYPE_VALUE;
            case ZdTypes.DDS.UINT_32_TYPE: return TypeKind.Values.UINT_32_TYPE_VALUE;
            case ZdTypes.DDS.INT_64_TYPE: return TypeKind.Values.INT_64_TYPE_VALUE;
            case ZdTypes.DDS.UINT_64_TYPE: return TypeKind.Values.UINT_64_TYPE_VALUE;
            case ZdTypes.DDS.FLOAT_32_TYPE: return TypeKind.Values.FLOAT_32_TYPE_VALUE;
            case ZdTypes.DDS.FLOAT_64_TYPE: return TypeKind.Values.FLOAT_64_TYPE_VALUE;
            case ZdTypes.DDS.FLOAT_128_TYPE: return TypeKind.Values.FLOAT_128_TYPE_VALUE;
            case ZdTypes.DDS.CHAR_8_TYPE: return TypeKind.Values.CHAR_8_TYPE_VALUE;
            default: return TypeKind.Values.CHAR_32_TYPE_VALUE;
        }
    }

    private static TypeKind kindOf(short wire) {
        switch (wire) {
            case ZdTypes.DDS.BOOLEAN_TYPE: return TypeKind.BOOLEAN_TYPE;
            case ZdTypes.DDS.BYTE_TYPE: return TypeKind.BYTE_TYPE;
            case ZdTypes.DDS.INT_16_TYPE: return TypeKind.INT_16_TYPE;
            case ZdTypes.DDS.UINT_16_TYPE: return TypeKind.UINT_16_TYPE;
            case ZdTypes.DDS.INT_32_TYPE: return TypeKind.INT_32_TYPE;
            case ZdTypes.DDS.UINT_32_TYPE: return TypeKind.UINT_32_TYPE;
            case ZdTypes.DDS.INT_64_TYPE: return TypeKind.INT_64_TYPE;
            case ZdTypes.DDS.UINT_64_TYPE: return TypeKind.UINT_64_TYPE;
            case ZdTypes.DDS.FLOAT_32_TYPE: return TypeKind.FLOAT_32_TYPE;
            case ZdTypes.DDS.FLOAT_64_TYPE: return TypeKind.FLOAT_64_TYPE;
            case ZdTypes.DDS.FLOAT_128_TYPE: return TypeKind.FLOAT_128_TYPE;
            case ZdTypes.DDS.CHAR_8_TYPE: return TypeKind.CHAR_8_TYPE;
            case ZdTypes.DDS.CHAR_32_TYPE: return TypeKind.CHAR_32_TYPE;
            case ZdTypes.DDS.ENUMERATION_TYPE: return TypeKind.ENUMERATION_TYPE;
            case ZdTypes.DDS.STRING_TYPE: return TypeKind.STRING_TYPE;
            default: return TypeKind.NO_TYPE;
        }
    }

    private TypeProperty property(ZdTypes.DDS.TypeProperty p) {
        return new TypePropertyImpl(env, p.flag, id(p.type_id), p.name);
    }

    private MemberProperty property(ZdTypes.DDS.MemberProperty p) {
        return new MemberPropertyImpl(env, p.flag, (int) p.member_id, id(p.type_id), p.name);
    }

    private List<AnnotationUsage> usages(ZdTypes.DDS.AnnotationUsage[] wire) {
        List<AnnotationUsage> out = new ArrayList<AnnotationUsage>();
        if (wire != null) {
            for (ZdTypes.DDS.AnnotationUsage u : wire) {
                List<AnnotationUsageMember> members = new ArrayList<AnnotationUsageMember>();
                if (u.member != null) {
                    for (ZdTypes.DDS.AnnotationUsageMember m : u.member) {
                        members.add(new UsageMember(env, (int) m.member_id, value(m.value)));
                    }
                }
                out.add(new Usage(env, id(u.type_id), members));
            }
        }
        return out;
    }

    private AnnotationMemberValue value(ZdTypes.DDS.AnnotationMemberValue v) {
        java.io.Serializable held;
        switch (v._d) {
            case ZdTypes.DDS.BOOLEAN_TYPE: held = Boolean.valueOf(v.boolean_value); break;
            case ZdTypes.DDS.BYTE_TYPE: held = Integer.valueOf(v.byte_value); break;
            case ZdTypes.DDS.INT_16_TYPE: held = Short.valueOf(v.int_16_value); break;
            case ZdTypes.DDS.UINT_16_TYPE: held = Integer.valueOf(v.uint_16_value); break;
            case ZdTypes.DDS.INT_32_TYPE: held = Integer.valueOf(v.int_32_value); break;
            case ZdTypes.DDS.UINT_32_TYPE: held = Long.valueOf(v.uint_32_value); break;
            case ZdTypes.DDS.INT_64_TYPE: held = Long.valueOf(v.int_64_value); break;
            case ZdTypes.DDS.UINT_64_TYPE: held = Long.valueOf(v.uint_64_value); break;
            case ZdTypes.DDS.FLOAT_32_TYPE: held = Float.valueOf(v.float_32_value); break;
            case ZdTypes.DDS.FLOAT_64_TYPE: held = Double.valueOf(v.float_64_value); break;
            case ZdTypes.DDS.FLOAT_128_TYPE: held = v.float_128_value == null ? null : v.float_128_value.toBits(); break; // the IEEE 754 binary128 bit pattern
            case ZdTypes.DDS.CHAR_8_TYPE: held = Character.valueOf(v.character_value); break;
            case ZdTypes.DDS.CHAR_32_TYPE: held = Integer.valueOf(v.wide_character_value); break;
            case ZdTypes.DDS.ENUMERATION_TYPE: held = Integer.valueOf(v.enumeration_value); break;
            case ZdTypes.DDS.STRING_TYPE: held = v.string_value; break;
            default: held = null; break;
        }
        return new Value(env, kindOf(v._d), held);
    }

    private List<Member> members(ZdTypes.DDS.Member[] wire) {
        List<Member> out = new ArrayList<Member>();
        if (wire != null) {
            for (ZdTypes.DDS.Member m : wire) {
                out.add(new MemberImpl(env, property(m.property), usages(m.annotation)));
            }
        }
        return out;
    }

    private static List<Bit> bits(ZdTypes.DDS.Bit[] wire, ServiceEnvironment env) {
        List<Bit> out = new ArrayList<Bit>();
        if (wire != null) {
            for (ZdTypes.DDS.Bit b : wire) {
                out.add(new BitImpl(env, b.index, b.name));
            }
        }
        return out;
    }

    private TypeLibraryElement element(ZdTypes.DDS.TypeLibraryElement e) {
        switch (e._d) {
            case ZdTypes.DDS.TypeLibraryElementKind.STRUCTURE_TYPE_ELEMENT: {
                ZdTypes.DDS.StructureType s = e.structure_type;
                return new StructureTypeImpl(env, property(s.property), usages(s.annotation),
                        id(s.base_type), members(s.member));
            }
            case ZdTypes.DDS.TypeLibraryElementKind.UNION_TYPE_ELEMENT: {
                ZdTypes.DDS.UnionType u = e.union_type;
                List<UnionMember> members = new ArrayList<UnionMember>();
                for (ZdTypes.DDS.UnionMember m : u.member) {
                    List<Integer> labels = new ArrayList<Integer>();
                    for (int l : m.label) {
                        labels.add(l);
                    }
                    members.add(new UnionMemberImpl(env, property(m.property), usages(m.annotation),
                            labels));
                }
                return new UnionTypeImpl(env, property(u.property), usages(u.annotation), members);
            }
            case ZdTypes.DDS.TypeLibraryElementKind.ENUMERATION_TYPE_ELEMENT: {
                ZdTypes.DDS.EnumerationType n = e.enumeration_type;
                List<EnumeratedConstant> constants = new ArrayList<EnumeratedConstant>();
                for (ZdTypes.DDS.EnumeratedConstant c : n.constant) {
                    constants.add(new ConstantImpl(env, c.value, c.name));
                }
                return new EnumerationTypeImpl(env, property(n.property), usages(n.annotation),
                        (int) n.bit_bound, constants);
            }
            case ZdTypes.DDS.TypeLibraryElementKind.BITSET_TYPE_ELEMENT: {
                ZdTypes.DDS.BitSetType b = e.bitset_type;
                return new BitSetTypeImpl(env, property(b.property), usages(b.annotation),
                        (int) b.bit_bound, bits(b.bit, env));
            }
            case ZdTypes.DDS.TypeLibraryElementKind.ALIAS_TYPE_ELEMENT: {
                ZdTypes.DDS.AliasType a = e.alias_type;
                return new AliasTypeImpl(env, property(a.property), usages(a.annotation),
                        id(a.base_type));
            }
            case ZdTypes.DDS.TypeLibraryElementKind.ARRAY_TYPE_ELEMENT: {
                ZdTypes.DDS.ArrayType a = e.array_type;
                List<Long> bounds = new ArrayList<Long>();
                for (long b : a.bound) {
                    bounds.add(b);
                }
                return new ArrayTypeImpl(env, property(a.property), usages(a.annotation),
                        id(a.element_type), bounds);
            }
            case ZdTypes.DDS.TypeLibraryElementKind.SEQUENCE_TYPE_ELEMENT: {
                ZdTypes.DDS.SequenceType s = e.sequence_type;
                return new SequenceTypeImpl(env, property(s.property), usages(s.annotation),
                        id(s.element_type), s.bound);
            }
            case ZdTypes.DDS.TypeLibraryElementKind.STRING_TYPE_ELEMENT: {
                ZdTypes.DDS.StringType s = e.string_type;
                return new StringTypeImpl(env, property(s.property), usages(s.annotation),
                        id(s.element_type), s.bound);
            }
            case ZdTypes.DDS.TypeLibraryElementKind.MAP_TYPE_ELEMENT: {
                ZdTypes.DDS.MapType m = e.map_type;
                return new MapTypeImpl(env, property(m.property), usages(m.annotation),
                        id(m.element_type), id(m.key_element_type), m.bound);
            }
            case ZdTypes.DDS.TypeLibraryElementKind.ANNOTATION_TYPE_ELEMENT: {
                ZdTypes.DDS.AnnotationType a = e.annotation_type;
                List<AnnotationMember> members = new ArrayList<AnnotationMember>();
                for (ZdTypes.DDS.AnnotationMember m : a.member) {
                    members.add(new AnnotationMemberImpl(env, property(m.property),
                            usages(m.annotation), value(m.default_value)));
                }
                return new AnnotationTypeImpl(env, property(a.property), usages(a.annotation),
                        id(a.base_type), members);
            }
            default: {
                ZdTypes.DDS.Module m = e.mod;
                return new ModuleImpl(env, m.name, m.library == null ? null : of(env, m.library));
            }
        }
    }

    private TypeLibrary of(ServiceEnvironment e, ZdTypes.DDS.TypeLibrary library) {
        ZdTypes.DDS.TypeObject wrapper = new ZdTypes.DDS.TypeObject();
        wrapper.library = library;
        wrapper.the_type = new ZdTypes.DDS.TypeId[0];
        return of(e, wrapper).getLibrary();
    }

    // -------------------------------------------------------------------------------------

    private abstract static class Base implements org.omg.dds.core.DDSObject, java.io.Serializable {
        private static final long serialVersionUID = 1L;
        private final transient ServiceEnvironment env;

        Base(ServiceEnvironment env) {
            this.env = env;
        }

        @Override
        public ServiceEnvironment getEnvironment() {
            return env;
        }

        /** Value semantics (DDS-Java-PSM 1.0 7.3.3): equal when the state is equal. */
        @Override
        public boolean equals(Object other) {
            if (this == other) {
                return true;
            }
            if (other == null || other.getClass() != getClass()) {
                return false;
            }
            try {
                for (Class<?> c = getClass(); c != Base.class; c = c.getSuperclass()) {
                    for (java.lang.reflect.Field f : c.getDeclaredFields()) {
                        if (java.lang.reflect.Modifier.isStatic(f.getModifiers())
                                || java.lang.reflect.Modifier.isTransient(f.getModifiers())) {
                            continue;
                        }
                        f.setAccessible(true);
                        if (!java.util.Objects.deepEquals(f.get(this), f.get(other))) {
                            return false;
                        }
                    }
                }
            } catch (IllegalAccessException e) {
                throw new IllegalStateException(e);
            }
            return true;
        }

        @Override
        public int hashCode() {
            int hash = getClass().hashCode();
            try {
                for (Class<?> c = getClass(); c != Base.class; c = c.getSuperclass()) {
                    for (java.lang.reflect.Field f : c.getDeclaredFields()) {
                        if (java.lang.reflect.Modifier.isStatic(f.getModifiers())
                                || java.lang.reflect.Modifier.isTransient(f.getModifiers())) {
                            continue;
                        }
                        f.setAccessible(true);
                        hash = 31 * hash + java.util.Arrays.deepHashCode(new Object[] {f.get(this)});
                    }
                }
            } catch (IllegalAccessException e) {
                throw new IllegalStateException(e);
            }
            return hash;
        }
    }

    private static int lowestFlag(int bits, int count) {
        for (int i = 0; i < count; i++) {
            if ((bits & (1 << i)) != 0) {
                return i;
            }
        }
        return -1;
    }

    private static final class TypeObjectImpl extends Base implements TypeObject {
        private static final long serialVersionUID = 1L;
        private final TypeLibrary library;
        private final int root;

        TypeObjectImpl(ServiceEnvironment env, TypeLibrary library, int root) {
            super(env);
            this.library = library;
            this.root = root;
        }

        @Override
        public TypeLibrary getLibrary() {
            return library;
        }

        @Override
        public int getTheType() {
            return root;
        }
    }

    private static final class Library extends Base implements TypeLibrary {
        private static final long serialVersionUID = 1L;
        private final ArrayList<TypeLibraryElement> elements;

        Library(ServiceEnvironment env, List<TypeLibraryElement> elements) {
            super(env);
            this.elements = new ArrayList<TypeLibraryElement>(elements);
        }

        @Override
        public List<TypeLibraryElement> getElement() {
            return Collections.unmodifiableList(elements);
        }
    }

    private static final class ModuleImpl extends Base implements Module {
        private static final long serialVersionUID = 1L;
        private final String name;
        private final TypeLibrary library;

        ModuleImpl(ServiceEnvironment env, String name, TypeLibrary library) {
            super(env);
            this.name = name;
            this.library = library;
        }

        @Override
        public String getName() {
            return name;
        }

        @Override
        public TypeLibrary getLibrary() {
            return library;
        }
    }

    private static final class TypePropertyImpl extends Base implements TypeProperty {
        private static final long serialVersionUID = 1L;
        final int flagBits;
        private final int typeId;
        private final String name;

        TypePropertyImpl(ServiceEnvironment env, int flagBits, int typeId, String name) {
            super(env);
            this.flagBits = flagBits;
            this.typeId = typeId;
            this.name = name;
        }

        @Override
        public TypeFlag getFlag() {
            int bit = lowestFlag(flagBits, 3);
            return bit < 0 ? null : TypeFlag.values()[bit];
        }

        @Override
        public int getTypeId() {
            return typeId;
        }

        @Override
        public String getName() {
            return name;
        }
    }

    private static final class MemberPropertyImpl extends Base implements MemberProperty {
        private static final long serialVersionUID = 1L;
        final int flagBits;
        private final int memberId;
        private final int typeId;
        private final String name;

        MemberPropertyImpl(ServiceEnvironment env, int flagBits, int memberId, int typeId,
                String name) {
            super(env);
            this.flagBits = flagBits;
            this.memberId = memberId;
            this.typeId = typeId;
            this.name = name;
        }

        @Override
        public MemberFlag getFlag() {
            int bit = lowestFlag(flagBits, 4);
            return bit < 0 ? null : MemberFlag.values()[bit];
        }

        @Override
        public int getMemberId() {
            return memberId;
        }

        @Override
        public int getTypeId() {
            return typeId;
        }

        @Override
        public String getName() {
            return name;
        }
    }

    private static final class Value extends Base implements AnnotationMemberValue {
        private static final long serialVersionUID = 1L;
        private final TypeKind kind;
        private final java.io.Serializable value;

        Value(ServiceEnvironment env, TypeKind kind, java.io.Serializable value) {
            super(env);
            this.kind = kind;
            this.value = value;
        }

        @Override
        public TypeKind kind() {
            return kind;
        }

        @Override
        public Object value() {
            return value;
        }
    }

    private static final class UsageMember extends Base implements AnnotationUsageMember {
        private static final long serialVersionUID = 1L;
        private final int memberId;
        private final AnnotationMemberValue value;

        UsageMember(ServiceEnvironment env, int memberId, AnnotationMemberValue value) {
            super(env);
            this.memberId = memberId;
            this.value = value;
        }

        @Override
        public int getMemberId() {
            return memberId;
        }

        @Override
        public AnnotationMemberValue getValue() {
            return value;
        }
    }

    private static final class Usage extends Base implements AnnotationUsage {
        private static final long serialVersionUID = 1L;
        private final int typeId;
        private final ArrayList<AnnotationUsageMember> members;

        Usage(ServiceEnvironment env, int typeId, List<AnnotationUsageMember> members) {
            super(env);
            this.typeId = typeId;
            this.members = new ArrayList<AnnotationUsageMember>(members);
        }

        @Override
        public int getTypeId() {
            return typeId;
        }

        @Override
        public List<AnnotationUsageMember> getMember() {
            return Collections.unmodifiableList(members);
        }
    }

    private static class MemberImpl extends Base implements Member {
        private static final long serialVersionUID = 1L;
        private final MemberProperty property;
        private final ArrayList<AnnotationUsage> annotation;

        MemberImpl(ServiceEnvironment env, MemberProperty property, List<AnnotationUsage> annotation) {
            super(env);
            this.property = property;
            this.annotation = new ArrayList<AnnotationUsage>(annotation);
        }

        @Override
        public MemberProperty getProperty() {
            return property;
        }

        @Override
        public List<AnnotationUsage> getAnnotation() {
            return Collections.unmodifiableList(annotation);
        }
    }

    private static final class UnionMemberImpl extends MemberImpl implements UnionMember {
        private static final long serialVersionUID = 1L;
        private final ArrayList<Integer> label;

        UnionMemberImpl(ServiceEnvironment env, MemberProperty property,
                List<AnnotationUsage> annotation, List<Integer> label) {
            super(env, property, annotation);
            this.label = new ArrayList<Integer>(label);
        }

        @Override
        public List<Integer> getLabel() {
            return Collections.unmodifiableList(label);
        }
    }

    private static final class AnnotationMemberImpl extends MemberImpl implements AnnotationMember {
        private static final long serialVersionUID = 1L;
        private final AnnotationMemberValue value;

        AnnotationMemberImpl(ServiceEnvironment env, MemberProperty property,
                List<AnnotationUsage> annotation, AnnotationMemberValue value) {
            super(env, property, annotation);
            this.value = value;
        }

        @Override
        public AnnotationMemberValue getDefaultValue() {
            return value;
        }
    }

    private static final class BitImpl extends Base implements Bit {
        private static final long serialVersionUID = 1L;
        private final int index;
        private final String name;

        BitImpl(ServiceEnvironment env, int index, String name) {
            super(env);
            this.index = index;
            this.name = name;
        }

        @Override
        public int getIndex() {
            return index;
        }

        @Override
        public String getName() {
            return name;
        }

        @Override
        public int compareTo(Bit other) {
            return Integer.compare(index, other.getIndex());
        }
    }

    private static final class ConstantImpl extends Base implements EnumeratedConstant {
        private static final long serialVersionUID = 1L;
        private final int value;
        private final String name;

        ConstantImpl(ServiceEnvironment env, int value, String name) {
            super(env);
            this.value = value;
            this.name = name;
        }

        @Override
        public int getValue() {
            return value;
        }

        @Override
        public String getName() {
            return name;
        }
    }

    private abstract static class TypeImpl extends Base implements Type {
        private static final long serialVersionUID = 1L;
        private final TypeProperty property;
        private final ArrayList<AnnotationUsage> annotation;

        TypeImpl(ServiceEnvironment env, TypeProperty property, List<AnnotationUsage> annotation) {
            super(env);
            this.property = property;
            this.annotation = new ArrayList<AnnotationUsage>(annotation);
        }

        @Override
        public TypeProperty getProperty() {
            return property;
        }

        @Override
        public List<AnnotationUsage> getAnnotation() {
            return Collections.unmodifiableList(annotation);
        }
    }

    private static final class StructureTypeImpl extends TypeImpl implements StructureType {
        private static final long serialVersionUID = 1L;
        private final int base;
        private final ArrayList<Member> members;

        StructureTypeImpl(ServiceEnvironment env, TypeProperty p, List<AnnotationUsage> a, int base,
                List<Member> members) {
            super(env, p, a);
            this.base = base;
            this.members = new ArrayList<Member>(members);
        }

        @Override
        public int getBaseType() {
            return base;
        }

        @Override
        public List<Member> getMember() {
            return Collections.unmodifiableList(members);
        }
    }

    private static final class UnionTypeImpl extends TypeImpl implements UnionType {
        private static final long serialVersionUID = 1L;
        private final ArrayList<UnionMember> members;

        UnionTypeImpl(ServiceEnvironment env, TypeProperty p, List<AnnotationUsage> a,
                List<UnionMember> members) {
            super(env, p, a);
            this.members = new ArrayList<UnionMember>(members);
        }

        @Override
        public List<UnionMember> getMember() {
            return Collections.unmodifiableList(members);
        }
    }

    private static final class AnnotationTypeImpl extends TypeImpl implements AnnotationType {
        private static final long serialVersionUID = 1L;
        private final int base;
        private final ArrayList<AnnotationMember> members;

        AnnotationTypeImpl(ServiceEnvironment env, TypeProperty p, List<AnnotationUsage> a, int base,
                List<AnnotationMember> members) {
            super(env, p, a);
            this.base = base;
            this.members = new ArrayList<AnnotationMember>(members);
        }

        @Override
        public int getBaseType() {
            return base;
        }

        @Override
        public List<AnnotationMember> getMember() {
            return Collections.unmodifiableList(members);
        }
    }

    private static final class AliasTypeImpl extends TypeImpl implements AliasType {
        private static final long serialVersionUID = 1L;
        private final int base;

        AliasTypeImpl(ServiceEnvironment env, TypeProperty p, List<AnnotationUsage> a, int base) {
            super(env, p, a);
            this.base = base;
        }

        @Override
        public int getBaseType() {
            return base;
        }
    }

    private static final class EnumerationTypeImpl extends TypeImpl implements EnumerationType {
        private static final long serialVersionUID = 1L;
        private final int bitBound;
        private final ArrayList<EnumeratedConstant> constants;

        EnumerationTypeImpl(ServiceEnvironment env, TypeProperty p, List<AnnotationUsage> a,
                int bitBound, List<EnumeratedConstant> constants) {
            super(env, p, a);
            this.bitBound = bitBound;
            this.constants = new ArrayList<EnumeratedConstant>(constants);
        }

        @Override
        public int getBitBound() {
            return bitBound;
        }

        @Override
        public List<EnumeratedConstant> getConstant() {
            return Collections.unmodifiableList(constants);
        }
    }

    private static final class BitSetTypeImpl extends TypeImpl implements BitSetType {
        private static final long serialVersionUID = 1L;
        private final int bitBound;
        private final ArrayList<Bit> bits;

        BitSetTypeImpl(ServiceEnvironment env, TypeProperty p, List<AnnotationUsage> a, int bitBound,
                List<Bit> bits) {
            super(env, p, a);
            this.bitBound = bitBound;
            this.bits = new ArrayList<Bit>(bits);
        }

        @Override
        public int getBitBound() {
            return bitBound;
        }

        @Override
        public List<Bit> getBit() {
            return Collections.unmodifiableList(bits);
        }
    }

    private abstract static class CollectionImpl extends TypeImpl
            implements org.omg.dds.type.typeobject.CollectionType {
        private static final long serialVersionUID = 1L;
        private final int element;

        CollectionImpl(ServiceEnvironment env, TypeProperty p, List<AnnotationUsage> a, int element) {
            super(env, p, a);
            this.element = element;
        }

        @Override
        public int getElementType() {
            return element;
        }
    }

    private static final class ArrayTypeImpl extends CollectionImpl implements ArrayType {
        private static final long serialVersionUID = 1L;
        private final ArrayList<Long> bound;

        ArrayTypeImpl(ServiceEnvironment env, TypeProperty p, List<AnnotationUsage> a, int element,
                List<Long> bound) {
            super(env, p, a, element);
            this.bound = new ArrayList<Long>(bound);
        }

        @Override
        public List<Long> getBound() {
            return Collections.unmodifiableList(bound);
        }
    }

    private static final class SequenceTypeImpl extends CollectionImpl implements SequenceType {
        private static final long serialVersionUID = 1L;
        private final long bound;

        SequenceTypeImpl(ServiceEnvironment env, TypeProperty p, List<AnnotationUsage> a, int element,
                long bound) {
            super(env, p, a, element);
            this.bound = bound;
        }

        @Override
        public long getBound() {
            return bound;
        }
    }

    private static final class StringTypeImpl extends CollectionImpl implements StringType {
        private static final long serialVersionUID = 1L;
        private final long bound;

        StringTypeImpl(ServiceEnvironment env, TypeProperty p, List<AnnotationUsage> a, int element,
                long bound) {
            super(env, p, a, element);
            this.bound = bound;
        }

        @Override
        public long getBound() {
            return bound;
        }
    }

    private static final class MapTypeImpl extends CollectionImpl implements MapType {
        private static final long serialVersionUID = 1L;
        private final int key;
        private final long bound;

        MapTypeImpl(ServiceEnvironment env, TypeProperty p, List<AnnotationUsage> a, int element,
                int key, long bound) {
            super(env, p, a, element);
            this.key = key;
            this.bound = bound;
        }

        @Override
        public int getKeyElementType() {
            return key;
        }

        @Override
        public long getBound() {
            return bound;
        }
    }
}
