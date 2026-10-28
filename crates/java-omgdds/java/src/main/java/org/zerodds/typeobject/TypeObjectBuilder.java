package org.zerodds.typeobject;

import java.util.ArrayList;
import java.util.Collections;
import java.util.Comparator;
import java.util.IdentityHashMap;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

import org.omg.dds.type.TypeKind;
import org.omg.dds.type.dynamic.AnnotationDescriptor;
import org.omg.dds.type.dynamic.DynamicType;
import org.omg.dds.type.dynamic.DynamicTypeMember;
import org.omg.dds.type.dynamic.MemberDescriptor;
import org.zerodds.typeobject.generated.ZdTypes;

/**
 * The XTypes 1.1 TypeObject (Annex B) of a {@link DynamicType}: one library element per
 * constructed type, each referred to by its TypeId (XTypes 1.1 7.3.4.1.2, bottom up).
 *
 * <p>What the TypeObject of Annex B states is represented: the kind, the scoped name, the
 * extensibility and the nested flag, the base type, the members with id, name, type, key,
 * optional and union labels, the enumeration constants, the bounds and the element types.
 * Annotations other than these are not written; the annotation usage list stays empty.
 * A bit set lists no bits (a DynamicType does not hold their names); a union states no
 * discriminator type (Annex B has no member for it). A type that contains itself has no
 * hash of its own in this scheme and is refused with an {@link IllegalArgumentException}.
 */
public final class TypeObjectBuilder {

    private final Map<Long, ZdTypes.DDS.TypeLibraryElement> elements = new LinkedHashMap<>();
    private final Map<DynamicType, ZdTypes.DDS.TypeId> done = new IdentityHashMap<>();
    private final Map<DynamicType, Boolean> inProgress = new IdentityHashMap<>();

    private TypeObjectBuilder() {
    }

    /** The TypeObject of {@code root}. */
    public static ZdTypes.DDS.TypeObject build(DynamicType root) {
        if (root == null) {
            throw new IllegalArgumentException("no type");
        }
        TypeObjectBuilder b = new TypeObjectBuilder();
        ZdTypes.DDS.TypeId id = b.idOf(root);
        ZdTypes.DDS.TypeObject object = new ZdTypes.DDS.TypeObject();
        object.library = new ZdTypes.DDS.TypeLibrary();
        object.library.element = b.elements.values()
                .toArray(new ZdTypes.DDS.TypeLibraryElement[0]);
        object.the_type = new ZdTypes.DDS.TypeId[] {id};
        return object;
    }

    private static ZdTypes.DDS.TypeId none() {
        return TypeObjectCodec.constructed(ZdTypes.DDS.NO_TYPE, 0L);
    }

    private ZdTypes.DDS.TypeId idOf(DynamicType type) {
        short kind = wire(type.getKind());
        if (TypeObjectCodec.isPrimitive(kind)) {
            return TypeObjectCodec.primitive(kind);
        }
        ZdTypes.DDS.TypeId known = done.get(type);
        if (known != null) {
            return known;
        }
        if (inProgress.put(type, Boolean.TRUE) != null) {
            throw new IllegalArgumentException(type.getName()
                    + " contains itself; its TypeId would be the hash of itself (XTypes 1.1 7.3.4.1.2)");
        }
        try {
            ZdTypes.DDS.TypeLibraryElement element = element(type, kind);
            long value = TypeObjectCodec.constructedTypeId(element);
            ZdTypes.DDS.TypeId id = TypeObjectCodec.constructed(kind, value);
            TypeObjectCodec.propertyOf(element).type_id = id;
            if (!elements.containsKey(value)) {
                elements.put(value, element);
            }
            done.put(type, id);
            return id;
        } finally {
            inProgress.remove(type);
        }
    }

    private static short wire(TypeKind kind) {
        switch (kind) {
            case NO_TYPE: return ZdTypes.DDS.NO_TYPE;
            case BOOLEAN_TYPE: return ZdTypes.DDS.BOOLEAN_TYPE;
            case BYTE_TYPE: return ZdTypes.DDS.BYTE_TYPE;
            case INT_16_TYPE: return ZdTypes.DDS.INT_16_TYPE;
            case UINT_16_TYPE: return ZdTypes.DDS.UINT_16_TYPE;
            case INT_32_TYPE: return ZdTypes.DDS.INT_32_TYPE;
            case UINT_32_TYPE: return ZdTypes.DDS.UINT_32_TYPE;
            case INT_64_TYPE: return ZdTypes.DDS.INT_64_TYPE;
            case UINT_64_TYPE: return ZdTypes.DDS.UINT_64_TYPE;
            case FLOAT_32_TYPE: return ZdTypes.DDS.FLOAT_32_TYPE;
            case FLOAT_64_TYPE: return ZdTypes.DDS.FLOAT_64_TYPE;
            case FLOAT_128_TYPE: return ZdTypes.DDS.FLOAT_128_TYPE;
            case CHAR_8_TYPE: return ZdTypes.DDS.CHAR_8_TYPE;
            case CHAR_32_TYPE: return ZdTypes.DDS.CHAR_32_TYPE;
            case ENUMERATION_TYPE: return ZdTypes.DDS.ENUMERATION_TYPE;
            case BITSET_TYPE: return ZdTypes.DDS.BITSET_TYPE;
            case ALIAS_TYPE: return ZdTypes.DDS.ALIAS_TYPE;
            case ARRAY_TYPE: return ZdTypes.DDS.ARRAY_TYPE;
            case SEQUENCE_TYPE: return ZdTypes.DDS.SEQUENCE_TYPE;
            case STRING_TYPE: return ZdTypes.DDS.STRING_TYPE;
            case MAP_TYPE: return ZdTypes.DDS.MAP_TYPE;
            case UNION_TYPE: return ZdTypes.DDS.UNION_TYPE;
            case STRUCTURE_TYPE: return ZdTypes.DDS.STRUCTURE_TYPE;
            case ANNOTATION_TYPE: return ZdTypes.DDS.ANNOTATION_TYPE;
            default: throw new IllegalArgumentException("unknown kind " + kind);
        }
    }

    private static boolean hasAnnotation(Iterable<AnnotationDescriptor> marks, String name) {
        for (AnnotationDescriptor mark : marks) {
            if (mark.getType() != null && name.equals(mark.getType().getName())) {
                return true;
            }
        }
        return false;
    }

    private static int typeFlag(DynamicType type) {
        int flag = 0;
        AnnotationDescriptor ext = type.getAnnotation("Extensibility");
        String kind = ext == null ? null : ext.getValue("value");
        if ("FINAL".equals(kind) || "FINAL_EXTENSIBILITY".equals(kind)) {
            flag |= ZdTypes.DDS.TypeFlag.IS_FINAL;
        } else if ("MUTABLE".equals(kind) || "MUTABLE_EXTENSIBILITY".equals(kind)) {
            flag |= ZdTypes.DDS.TypeFlag.IS_MUTABLE;
        }
        if (type.getAnnotation("Nested") != null) {
            flag |= ZdTypes.DDS.TypeFlag.IS_NESTED;
        }
        return flag;
    }

    private static ZdTypes.DDS.TypeProperty property(DynamicType type) {
        ZdTypes.DDS.TypeProperty p = new ZdTypes.DDS.TypeProperty();
        p.flag = typeFlag(type);
        p.type_id = none();
        p.name = type.getName() == null ? "" : type.getName();
        return p;
    }

    private static long bound(DynamicType type) {
        List<Integer> bound = type.getDescriptor().getBound();
        return bound == null || bound.isEmpty() ? 0 : bound.get(0) & 0xFFFFFFFFL;
    }

    private List<DynamicTypeMember> ordered(DynamicType type) {
        List<DynamicTypeMember> members = new ArrayList<>(type.getAllMembers().values());
        Collections.sort(members, new Comparator<DynamicTypeMember>() {
            @Override
            public int compare(DynamicTypeMember a, DynamicTypeMember b) {
                int byIndex = Integer.compare(a.getDescriptor().getIndex(),
                        b.getDescriptor().getIndex());
                return byIndex != 0 ? byIndex : Integer.compare(a.getId(), b.getId());
            }
        });
        return members;
    }

    private ZdTypes.DDS.MemberProperty memberProperty(DynamicTypeMember member, boolean union) {
        MemberDescriptor md = member.getDescriptor();
        ZdTypes.DDS.MemberProperty p = new ZdTypes.DDS.MemberProperty();
        List<AnnotationDescriptor> marks = member.getAnnotations();
        if (hasAnnotation(marks, "Key")) {
            p.flag |= ZdTypes.DDS.MemberFlag.IS_KEY;
        }
        if (hasAnnotation(marks, "Optional")) {
            p.flag |= ZdTypes.DDS.MemberFlag.IS_OPTIONAL;
        }
        if (hasAnnotation(marks, "Shared")) {
            p.flag |= ZdTypes.DDS.MemberFlag.IS_SHAREABLE;
        }
        if (union && md.isDefaultLabel()) {
            p.flag |= ZdTypes.DDS.MemberFlag.IS_UNION_DEFAULT;
        }
        p.member_id = member.getId() & 0xFFFFFFFFL;
        p.type_id = idOf(md.getType());
        p.name = member.getName();
        return p;
    }

    private ZdTypes.DDS.TypeLibraryElement element(DynamicType type, short kind) {
        ZdTypes.DDS.TypeLibraryElement e = new ZdTypes.DDS.TypeLibraryElement();
        e._d = kind;
        ZdTypes.DDS.AnnotationUsage[] noUsage = new ZdTypes.DDS.AnnotationUsage[0];
        DynamicType base = type.getDescriptor().getBaseType();
        switch (kind) {
            case ZdTypes.DDS.STRUCTURE_TYPE: {
                ZdTypes.DDS.StructureType s = new ZdTypes.DDS.StructureType();
                s.property = property(type);
                s.annotation = noUsage;
                s.base_type = base == null ? none() : idOf(base);
                List<ZdTypes.DDS.Member> members = new ArrayList<>();
                for (DynamicTypeMember m : ordered(type)) {
                    ZdTypes.DDS.Member member = new ZdTypes.DDS.Member();
                    member.property = memberProperty(m, false);
                    member.annotation = noUsage;
                    members.add(member);
                }
                s.member = members.toArray(new ZdTypes.DDS.Member[0]);
                e.structure_type = s;
                break;
            }
            case ZdTypes.DDS.UNION_TYPE: {
                ZdTypes.DDS.UnionType u = new ZdTypes.DDS.UnionType();
                u.property = property(type);
                u.annotation = noUsage;
                List<ZdTypes.DDS.UnionMember> members = new ArrayList<>();
                for (DynamicTypeMember m : ordered(type)) {
                    ZdTypes.DDS.UnionMember member = new ZdTypes.DDS.UnionMember();
                    member.property = memberProperty(m, true);
                    member.annotation = noUsage;
                    List<Integer> labels = m.getDescriptor().getLabel();
                    member.label = new int[labels == null ? 0 : labels.size()];
                    for (int i = 0; i < member.label.length; i++) {
                        member.label[i] = labels.get(i);
                    }
                    members.add(member);
                }
                u.member = members.toArray(new ZdTypes.DDS.UnionMember[0]);
                e.union_type = u;
                break;
            }
            case ZdTypes.DDS.ENUMERATION_TYPE: {
                ZdTypes.DDS.EnumerationType n = new ZdTypes.DDS.EnumerationType();
                n.property = property(type);
                n.annotation = noUsage;
                n.bit_bound = 32;
                List<ZdTypes.DDS.EnumeratedConstant> constants = new ArrayList<>();
                for (DynamicTypeMember m : ordered(type)) {
                    ZdTypes.DDS.EnumeratedConstant c = new ZdTypes.DDS.EnumeratedConstant();
                    c.value = m.getId();
                    c.name = m.getName();
                    constants.add(c);
                }
                n.constant = constants.toArray(new ZdTypes.DDS.EnumeratedConstant[0]);
                e.enumeration_type = n;
                break;
            }
            case ZdTypes.DDS.BITSET_TYPE: {
                ZdTypes.DDS.BitSetType b = new ZdTypes.DDS.BitSetType();
                b.property = property(type);
                b.annotation = noUsage;
                b.bit_bound = bound(type);
                b.bit = new ZdTypes.DDS.Bit[0];
                e.bitset_type = b;
                break;
            }
            case ZdTypes.DDS.ALIAS_TYPE: {
                ZdTypes.DDS.AliasType a = new ZdTypes.DDS.AliasType();
                a.property = property(type);
                a.annotation = noUsage;
                a.base_type = idOf(base);
                e.alias_type = a;
                break;
            }
            case ZdTypes.DDS.ARRAY_TYPE: {
                ZdTypes.DDS.ArrayType a = new ZdTypes.DDS.ArrayType();
                a.property = property(type);
                a.annotation = noUsage;
                a.element_type = idOf(type.getDescriptor().getElementType());
                List<Integer> bounds = type.getDescriptor().getBound();
                a.bound = new long[bounds.size()];
                for (int i = 0; i < a.bound.length; i++) {
                    a.bound[i] = bounds.get(i) & 0xFFFFFFFFL;
                }
                e.array_type = a;
                break;
            }
            case ZdTypes.DDS.SEQUENCE_TYPE: {
                ZdTypes.DDS.SequenceType s = new ZdTypes.DDS.SequenceType();
                s.property = property(type);
                s.annotation = noUsage;
                s.element_type = idOf(type.getDescriptor().getElementType());
                s.bound = bound(type);
                e.sequence_type = s;
                break;
            }
            case ZdTypes.DDS.STRING_TYPE: {
                ZdTypes.DDS.StringType s = new ZdTypes.DDS.StringType();
                s.property = property(type);
                s.annotation = noUsage;
                DynamicType character = type.getDescriptor().getElementType();
                s.element_type = character != null ? idOf(character)
                        : TypeObjectCodec.primitive(org.zerodds.cdr.TypeAssignability.isWide(type)
                                ? ZdTypes.DDS.CHAR_32_TYPE : ZdTypes.DDS.CHAR_8_TYPE);
                s.bound = bound(type);
                e.string_type = s;
                break;
            }
            case ZdTypes.DDS.MAP_TYPE: {
                ZdTypes.DDS.MapType m = new ZdTypes.DDS.MapType();
                m.property = property(type);
                m.annotation = noUsage;
                m.element_type = idOf(type.getDescriptor().getElementType());
                m.key_element_type = idOf(type.getDescriptor().getKeyElementType());
                m.bound = bound(type);
                e.map_type = m;
                break;
            }
            case ZdTypes.DDS.ANNOTATION_TYPE: {
                ZdTypes.DDS.AnnotationType a = new ZdTypes.DDS.AnnotationType();
                a.property = property(type);
                a.annotation = noUsage;
                a.base_type = base == null ? none() : idOf(base);
                List<ZdTypes.DDS.AnnotationMember> members = new ArrayList<>();
                for (DynamicTypeMember m : ordered(type)) {
                    ZdTypes.DDS.AnnotationMember member = new ZdTypes.DDS.AnnotationMember();
                    member.property = memberProperty(m, false);
                    member.annotation = noUsage;
                    member.default_value = new ZdTypes.DDS.AnnotationMemberValue();
                    member.default_value._d = ZdTypes.DDS.STRING_TYPE;
                    String text = m.getDescriptor().getDefaultValue();
                    member.default_value.string_value = text == null ? "" : text;
                    members.add(member);
                }
                a.member = members.toArray(new ZdTypes.DDS.AnnotationMember[0]);
                e.annotation_type = a;
                break;
            }
            default:
                throw new IllegalArgumentException("no TypeObject for the kind " + type.getKind());
        }
        return e;
    }
}
