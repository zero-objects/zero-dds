package org.zerodds.typeobject;

import java.util.HashMap;
import java.util.Map;

import org.omg.dds.core.ServiceEnvironment;
import org.omg.dds.type.TypeKind;
import org.omg.dds.type.dynamic.AnnotationDescriptor;
import org.omg.dds.type.dynamic.DynamicType;
import org.omg.dds.type.dynamic.DynamicTypeMember;
import org.omg.dds.type.dynamic.MemberDescriptor;
import org.omg.dds.type.dynamic.TypeDescriptor;
import org.zerodds.cdr.DynamicTypeFactoryImpl;
import org.zerodds.typeobject.generated.ZdTypes;

/**
 * The {@link DynamicType} a TypeObject of XTypes 1.1 describes (Annex C, {@code
 * create_type_w_type_object}). The inverse of {@link TypeObjectBuilder} for what that writes.
 *
 * <p>Annex B has no discriminator type for a union; the union gets {@code int32}. A bit set has
 * a bound and no named bits.
 */
public final class TypeObjectToDynamic {

    private final DynamicTypeFactoryImpl factory;
    private final Map<Long, ZdTypes.DDS.TypeLibraryElement> elements =
            new HashMap<Long, ZdTypes.DDS.TypeLibraryElement>();
    private final Map<Long, DynamicType> built = new HashMap<Long, DynamicType>();

    private TypeObjectToDynamic(ServiceEnvironment env) {
        this.factory = DynamicTypeFactoryImpl.getInstance(env);
    }

    /**
     * The type {@code object.the_type[0]} names.
     *
     * @throws IllegalArgumentException if the TypeObject names no type, or a type it refers to is
     *     not in its library
     */
    public static DynamicType of(ServiceEnvironment env, ZdTypes.DDS.TypeObject object) {
        if (object.the_type == null || object.the_type.length == 0) {
            throw new IllegalArgumentException("the TypeObject names no type");
        }
        TypeObjectToDynamic c = new TypeObjectToDynamic(env);
        if (object.library != null && object.library.element != null) {
            for (ZdTypes.DDS.TypeLibraryElement e : object.library.element) {
                if (e._d != ZdTypes.DDS.TypeLibraryElementKind.MODULE_ELEMENT) {
                    c.elements.put(TypeObjectCodec.propertyOf(e).type_id.constructed_type_id, e);
                }
            }
        }
        return c.type(object.the_type[0]);
    }

    private static TypeKind primitive(short kind) {
        switch (kind) {
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
            default: throw new IllegalArgumentException("not a primitive TypeId: " + kind);
        }
    }

    private DynamicType type(ZdTypes.DDS.TypeId id) {
        if (TypeObjectCodec.isPrimitive(id._d)) {
            return factory.getPrimitiveType(primitive(id._d));
        }
        DynamicType known = built.get(id.constructed_type_id);
        if (known != null) {
            return known;
        }
        ZdTypes.DDS.TypeLibraryElement e = elements.get(id.constructed_type_id);
        if (e == null) {
            throw new IllegalArgumentException("the type " + Long.toUnsignedString(
                    id.constructed_type_id) + " is not in the library of the TypeObject");
        }
        DynamicType t = element(e);
        built.put(id.constructed_type_id, t);
        return t;
    }

    private static boolean none(ZdTypes.DDS.TypeId id) {
        return id._d == ZdTypes.DDS.NO_TYPE;
    }

    private AnnotationDescriptor mark(String name, String value) {
        TypeDescriptor d = factory.newTypeDescriptor().withKind(TypeKind.ANNOTATION_TYPE)
                .withName(name);
        AnnotationDescriptor a = factory.newAnnotationDescriptor().withType(factory.createType(d));
        return value == null ? a : a.withValue("value", value);
    }

    private void extensibility(DynamicType t, ZdTypes.DDS.TypeProperty p) {
        String kind = (p.flag & ZdTypes.DDS.TypeFlag.IS_FINAL) != 0 ? "FINAL"
                : (p.flag & ZdTypes.DDS.TypeFlag.IS_MUTABLE) != 0 ? "MUTABLE" : "APPENDABLE";
        t.setAnnotation(mark("Extensibility", kind));
    }

    private void member(DynamicType owner, ZdTypes.DDS.MemberProperty p, int[] labels) {
        MemberDescriptor md = factory.newMemberDescriptor().withName(p.name)
                .withId((int) p.member_id).withType(type(p.type_id));
        if (labels != null) {
            md = md.withLabel(labels);
            if ((p.flag & ZdTypes.DDS.MemberFlag.IS_UNION_DEFAULT) != 0) {
                md = md.withDefaultLabel(true);
            }
        }
        DynamicTypeMember added = owner.addMember(md);
        java.util.List<AnnotationDescriptor> marks = new java.util.ArrayList<AnnotationDescriptor>();
        if ((p.flag & ZdTypes.DDS.MemberFlag.IS_KEY) != 0) {
            marks.add(mark("Key", null));
        }
        if ((p.flag & ZdTypes.DDS.MemberFlag.IS_OPTIONAL) != 0) {
            marks.add(mark("Optional", null));
        }
        if ((p.flag & ZdTypes.DDS.MemberFlag.IS_SHAREABLE) != 0) {
            marks.add(mark("Shared", null));
        }
        if (!marks.isEmpty()) {
            added.setAnnotations(marks.toArray(new AnnotationDescriptor[0]));
        }
    }

    private static int bound(long b) {
        return (int) b;
    }

    private DynamicType element(ZdTypes.DDS.TypeLibraryElement e) {
        switch (e._d) {
            case ZdTypes.DDS.TypeLibraryElementKind.STRUCTURE_TYPE_ELEMENT: {
                ZdTypes.DDS.StructureType s = e.structure_type;
                TypeDescriptor d = factory.newTypeDescriptor().withKind(TypeKind.STRUCTURE_TYPE)
                        .withName(s.property.name);
                if (!none(s.base_type)) {
                    d = d.withBaseType(type(s.base_type));
                }
                DynamicType t = factory.createType(d);
                extensibility(t, s.property);
                for (ZdTypes.DDS.Member m : s.member) {
                    member(t, m.property, null);
                }
                return t;
            }
            case ZdTypes.DDS.TypeLibraryElementKind.UNION_TYPE_ELEMENT: {
                ZdTypes.DDS.UnionType u = e.union_type;
                TypeDescriptor d = factory.newTypeDescriptor().withKind(TypeKind.UNION_TYPE)
                        .withName(u.property.name)
                        .withDiscriminatorType(factory.getPrimitiveType(TypeKind.INT_32_TYPE));
                DynamicType t = factory.createType(d);
                extensibility(t, u.property);
                for (ZdTypes.DDS.UnionMember m : u.member) {
                    member(t, m.property, m.label);
                }
                return t;
            }
            case ZdTypes.DDS.TypeLibraryElementKind.ENUMERATION_TYPE_ELEMENT: {
                ZdTypes.DDS.EnumerationType n = e.enumeration_type;
                DynamicType t = factory.createType(factory.newTypeDescriptor()
                        .withKind(TypeKind.ENUMERATION_TYPE).withName(n.property.name));
                extensibility(t, n.property);
                for (ZdTypes.DDS.EnumeratedConstant c : n.constant) {
                    t.addMember(factory.newMemberDescriptor().withName(c.name).withId(c.value));
                }
                return t;
            }
            case ZdTypes.DDS.TypeLibraryElementKind.ALIAS_TYPE_ELEMENT: {
                ZdTypes.DDS.AliasType a = e.alias_type;
                return factory.createType(factory.newTypeDescriptor().withKind(TypeKind.ALIAS_TYPE)
                        .withName(a.property.name).withBaseType(type(a.base_type)));
            }
            case ZdTypes.DDS.TypeLibraryElementKind.BITSET_TYPE_ELEMENT:
                return factory.createBitSetType(bound(e.bitset_type.bit_bound));
            case ZdTypes.DDS.TypeLibraryElementKind.STRING_TYPE_ELEMENT: {
                ZdTypes.DDS.StringType s = e.string_type;
                return s.element_type._d == ZdTypes.DDS.CHAR_32_TYPE
                        ? factory.createWStringType(bound(s.bound))
                        : factory.createStringType(bound(s.bound));
            }
            case ZdTypes.DDS.TypeLibraryElementKind.SEQUENCE_TYPE_ELEMENT: {
                ZdTypes.DDS.SequenceType s = e.sequence_type;
                return s.bound == 0 ? factory.createSequenceType(type(s.element_type))
                        : factory.createSequenceType(type(s.element_type), bound(s.bound));
            }
            case ZdTypes.DDS.TypeLibraryElementKind.ARRAY_TYPE_ELEMENT: {
                ZdTypes.DDS.ArrayType a = e.array_type;
                int[] bounds = new int[a.bound.length];
                for (int i = 0; i < bounds.length; i++) {
                    bounds[i] = bound(a.bound[i]);
                }
                return factory.createArrayType(type(a.element_type), bounds);
            }
            case ZdTypes.DDS.TypeLibraryElementKind.MAP_TYPE_ELEMENT: {
                ZdTypes.DDS.MapType m = e.map_type;
                return factory.createMapType(type(m.key_element_type), type(m.element_type),
                        bound(m.bound));
            }
            case ZdTypes.DDS.TypeLibraryElementKind.ANNOTATION_TYPE_ELEMENT: {
                ZdTypes.DDS.AnnotationType a = e.annotation_type;
                DynamicType t = factory.createType(factory.newTypeDescriptor()
                        .withKind(TypeKind.ANNOTATION_TYPE).withName(a.property.name));
                for (ZdTypes.DDS.AnnotationMember m : a.member) {
                    member(t, m.property, null);
                }
                return t;
            }
            default:
                throw new IllegalArgumentException("a module is no type");
        }
    }
}
