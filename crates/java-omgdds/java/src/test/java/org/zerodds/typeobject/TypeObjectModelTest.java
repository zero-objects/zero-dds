// SPDX-License-Identifier: Apache-2.0
package org.zerodds.typeobject;

import java.io.ByteArrayInputStream;
import java.io.ByteArrayOutputStream;
import java.io.ObjectInputStream;
import java.io.ObjectOutputStream;
import java.util.ArrayList;
import java.util.HashSet;
import java.util.List;
import java.util.Set;

import org.junit.jupiter.api.Test;
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
import org.omg.dds.type.typeobject.CollectionType;
import org.omg.dds.type.typeobject.EnumeratedConstant;
import org.omg.dds.type.typeobject.EnumerationType;
import org.omg.dds.type.typeobject.MapType;
import org.omg.dds.type.typeobject.Member;
import org.omg.dds.type.typeobject.MemberProperty;
import org.omg.dds.type.typeobject.Module;
import org.omg.dds.type.typeobject.SequenceType;
import org.omg.dds.type.typeobject.StringType;
import org.omg.dds.type.typeobject.StructureType;
import org.omg.dds.type.typeobject.Type;
import org.omg.dds.type.typeobject.TypeLibrary;
import org.omg.dds.type.typeobject.TypeLibraryElement;
import org.omg.dds.type.typeobject.TypeObject;
import org.omg.dds.type.typeobject.TypeProperty;
import org.omg.dds.type.typeobject.UnionMember;
import org.omg.dds.type.typeobject.UnionType;
import org.zerodds.typeobject.generated.ZdTypes;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * Every interface of {@code org.omg.dds.type.typeobject} has an implementation (DDS-Java-PSM 1.0
 * 7.2.1, 7.8): a TypeObject of XTypes 1.1 Annex B that holds one type of each kind, a module, an
 * annotation usage with a value, goes over the wire and comes back as objects of all interfaces.
 */
class TypeObjectModelTest {

    private static ZdTypes.DDS.TypeProperty property(String name) {
        ZdTypes.DDS.TypeProperty p = new ZdTypes.DDS.TypeProperty();
        p.name = name;
        p.type_id = TypeObjectCodec.constructed(ZdTypes.DDS.NO_TYPE, 0L);
        return p;
    }

    private static ZdTypes.DDS.MemberProperty member(String name, long id, ZdTypes.DDS.TypeId type,
            int flag) {
        ZdTypes.DDS.MemberProperty p = new ZdTypes.DDS.MemberProperty();
        p.name = name;
        p.member_id = id;
        p.type_id = type;
        p.flag = flag;
        return p;
    }

    private static ZdTypes.DDS.TypeLibraryElement element(int kind) {
        ZdTypes.DDS.TypeLibraryElement e = new ZdTypes.DDS.TypeLibraryElement();
        e._d = kind;
        return e;
    }

    private static final ZdTypes.DDS.AnnotationUsage[] NONE = new ZdTypes.DDS.AnnotationUsage[0];

    private static ZdTypes.DDS.TypeObject sample() {
        ZdTypes.DDS.TypeId int32 = TypeObjectCodec.primitive(ZdTypes.DDS.INT_32_TYPE);
        List<ZdTypes.DDS.TypeLibraryElement> elements = new ArrayList<>();

        ZdTypes.DDS.TypeLibraryElement alias = element(ZdTypes.DDS.TypeLibraryElementKind.ALIAS_TYPE_ELEMENT);
        alias.alias_type = new ZdTypes.DDS.AliasType();
        alias.alias_type.property = property("A");
        alias.alias_type.annotation = NONE;
        alias.alias_type.base_type = int32;
        elements.add(alias);

        ZdTypes.DDS.TypeLibraryElement array = element(ZdTypes.DDS.TypeLibraryElementKind.ARRAY_TYPE_ELEMENT);
        array.array_type = new ZdTypes.DDS.ArrayType();
        array.array_type.property = property("Arr");
        array.array_type.annotation = NONE;
        array.array_type.element_type = int32;
        array.array_type.bound = new long[] {3, 4};
        elements.add(array);

        ZdTypes.DDS.TypeLibraryElement sequence = element(ZdTypes.DDS.TypeLibraryElementKind.SEQUENCE_TYPE_ELEMENT);
        sequence.sequence_type = new ZdTypes.DDS.SequenceType();
        sequence.sequence_type.property = property("Seq");
        sequence.sequence_type.annotation = NONE;
        sequence.sequence_type.element_type = int32;
        sequence.sequence_type.bound = 7;
        elements.add(sequence);

        ZdTypes.DDS.TypeLibraryElement string = element(ZdTypes.DDS.TypeLibraryElementKind.STRING_TYPE_ELEMENT);
        string.string_type = new ZdTypes.DDS.StringType();
        string.string_type.property = property("Str");
        string.string_type.annotation = NONE;
        string.string_type.element_type = TypeObjectCodec.primitive(ZdTypes.DDS.CHAR_8_TYPE);
        string.string_type.bound = 16;
        elements.add(string);

        ZdTypes.DDS.TypeLibraryElement map = element(ZdTypes.DDS.TypeLibraryElementKind.MAP_TYPE_ELEMENT);
        map.map_type = new ZdTypes.DDS.MapType();
        map.map_type.property = property("Map");
        map.map_type.annotation = NONE;
        map.map_type.element_type = int32;
        map.map_type.key_element_type = int32;
        map.map_type.bound = 9;
        elements.add(map);

        ZdTypes.DDS.TypeLibraryElement bitset = element(ZdTypes.DDS.TypeLibraryElementKind.BITSET_TYPE_ELEMENT);
        bitset.bitset_type = new ZdTypes.DDS.BitSetType();
        bitset.bitset_type.property = property("Bits");
        bitset.bitset_type.annotation = NONE;
        bitset.bitset_type.bit_bound = 8;
        ZdTypes.DDS.Bit bit = new ZdTypes.DDS.Bit();
        bit.index = 2;
        bit.name = "flag";
        bitset.bitset_type.bit = new ZdTypes.DDS.Bit[] {bit};
        elements.add(bitset);

        ZdTypes.DDS.TypeLibraryElement enumeration =
                element(ZdTypes.DDS.TypeLibraryElementKind.ENUMERATION_TYPE_ELEMENT);
        enumeration.enumeration_type = new ZdTypes.DDS.EnumerationType();
        enumeration.enumeration_type.property = property("En");
        enumeration.enumeration_type.annotation = NONE;
        enumeration.enumeration_type.bit_bound = 16;
        ZdTypes.DDS.EnumeratedConstant constant = new ZdTypes.DDS.EnumeratedConstant();
        constant.value = 4;
        constant.name = "FOUR";
        enumeration.enumeration_type.constant = new ZdTypes.DDS.EnumeratedConstant[] {constant};
        elements.add(enumeration);

        // an annotation type with a member whose default is a string
        ZdTypes.DDS.TypeLibraryElement annotation =
                element(ZdTypes.DDS.TypeLibraryElementKind.ANNOTATION_TYPE_ELEMENT);
        annotation.annotation_type = new ZdTypes.DDS.AnnotationType();
        annotation.annotation_type.property = property("Ann");
        annotation.annotation_type.annotation = NONE;
        annotation.annotation_type.base_type = TypeObjectCodec.constructed(ZdTypes.DDS.NO_TYPE, 0L);
        ZdTypes.DDS.AnnotationMember annotationMember = new ZdTypes.DDS.AnnotationMember();
        annotationMember.property = member("value", 1, int32, 0);
        annotationMember.annotation = NONE;
        annotationMember.default_value = new ZdTypes.DDS.AnnotationMemberValue();
        annotationMember.default_value._d = ZdTypes.DDS.INT_32_TYPE;
        annotationMember.default_value.int_32_value = 12;
        annotation.annotation_type.member = new ZdTypes.DDS.AnnotationMember[] {annotationMember};
        elements.add(annotation);

        // a union
        ZdTypes.DDS.TypeLibraryElement union = element(ZdTypes.DDS.TypeLibraryElementKind.UNION_TYPE_ELEMENT);
        union.union_type = new ZdTypes.DDS.UnionType();
        union.union_type.property = property("Un");
        union.union_type.annotation = NONE;
        ZdTypes.DDS.UnionMember unionMember = new ZdTypes.DDS.UnionMember();
        unionMember.property = member("x", 1, int32, 0);
        unionMember.annotation = NONE;
        unionMember.label = new int[] {1, 2};
        union.union_type.member = new ZdTypes.DDS.UnionMember[] {unionMember};
        elements.add(union);

        // a structure with a member that carries an annotation usage
        ZdTypes.DDS.TypeLibraryElement struct = element(ZdTypes.DDS.TypeLibraryElementKind.STRUCTURE_TYPE_ELEMENT);
        struct.structure_type = new ZdTypes.DDS.StructureType();
        struct.structure_type.property = property("St");
        struct.structure_type.annotation = NONE;
        struct.structure_type.base_type = TypeObjectCodec.constructed(ZdTypes.DDS.NO_TYPE, 0L);
        ZdTypes.DDS.Member m = new ZdTypes.DDS.Member();
        m.property = member("k", 5, int32, ZdTypes.DDS.MemberFlag.IS_KEY | ZdTypes.DDS.MemberFlag.IS_OPTIONAL);
        ZdTypes.DDS.AnnotationUsage usage = new ZdTypes.DDS.AnnotationUsage();
        usage.type_id = int32;
        ZdTypes.DDS.AnnotationUsageMember usageMember = new ZdTypes.DDS.AnnotationUsageMember();
        usageMember.member_id = 1;
        usageMember.value = new ZdTypes.DDS.AnnotationMemberValue();
        usageMember.value._d = ZdTypes.DDS.STRING_TYPE;
        usageMember.value.string_value = "text";
        usage.member = new ZdTypes.DDS.AnnotationUsageMember[] {usageMember};
        m.annotation = new ZdTypes.DDS.AnnotationUsage[] {usage};
        struct.structure_type.member = new ZdTypes.DDS.Member[] {m};
        elements.add(struct);

        // a module with an empty library
        ZdTypes.DDS.TypeLibraryElement module = element(ZdTypes.DDS.TypeLibraryElementKind.MODULE_ELEMENT);
        module.mod = new ZdTypes.DDS.Module();
        module.mod.name = "mod";
        module.mod.library = new ZdTypes.DDS.TypeLibrary();
        module.mod.library.element = new ZdTypes.DDS.TypeLibraryElement[0];
        elements.add(module);

        // the ids of the types are the hashes of their serialization
        for (ZdTypes.DDS.TypeLibraryElement e : elements) {
            if (e._d != ZdTypes.DDS.TypeLibraryElementKind.MODULE_ELEMENT) {
                long id = TypeObjectCodec.constructedTypeId(e);
                TypeObjectCodec.propertyOf(e).type_id = TypeObjectCodec.constructed((short) e._d, id);
            }
        }
        ZdTypes.DDS.TypeObject object = new ZdTypes.DDS.TypeObject();
        object.library = new ZdTypes.DDS.TypeLibrary();
        object.library.element = elements.toArray(new ZdTypes.DDS.TypeLibraryElement[0]);
        object.the_type = new ZdTypes.DDS.TypeId[] {TypeObjectCodec.propertyOf(struct).type_id};
        return object;
    }

    @Test
    void everyInterfaceOfThePackageIsImplementedAndAnObjectOfItComesBackOverTheWire() {
        byte[] bytes = TypeObjectCodec.encode(sample());
        TypeObject object = TypeObjectModel.of(org.omg.dds.Qos.env(), TypeObjectCodec.decode(bytes));
        Set<Class<?>> seen = new HashSet<>();
        seen.add(TypeObject.class);
        TypeLibrary library = object.getLibrary();
        seen.add(TypeLibrary.class);
        for (TypeLibraryElement e : library.getElement()) {
            seen.add(TypeLibraryElement.class);
            if (e instanceof Type) {
                seen.add(Type.class);
                TypeProperty p = ((Type) e).getProperty();
                assertNotNull(p);
                seen.add(TypeProperty.class);
            }
            if (e instanceof CollectionType) {
                seen.add(CollectionType.class);
            }
            if (e instanceof AliasType) {
                seen.add(AliasType.class);
            } else if (e instanceof ArrayType) {
                seen.add(ArrayType.class);
                assertEquals(List.of(3L, 4L), ((ArrayType) e).getBound());
            } else if (e instanceof SequenceType) {
                seen.add(SequenceType.class);
                assertEquals(7L, ((SequenceType) e).getBound());
            } else if (e instanceof StringType) {
                seen.add(StringType.class);
            } else if (e instanceof MapType) {
                seen.add(MapType.class);
                assertEquals(9L, ((MapType) e).getBound());
            } else if (e instanceof BitSetType) {
                seen.add(BitSetType.class);
                for (Bit b : ((BitSetType) e).getBit()) {
                    seen.add(Bit.class);
                    assertEquals("flag", b.getName());
                    assertEquals(2, b.getIndex());
                }
            } else if (e instanceof EnumerationType) {
                seen.add(EnumerationType.class);
                for (EnumeratedConstant c : ((EnumerationType) e).getConstant()) {
                    seen.add(EnumeratedConstant.class);
                    assertEquals("FOUR", c.getName());
                }
            } else if (e instanceof AnnotationType) {
                seen.add(AnnotationType.class);
                for (AnnotationMember am : ((AnnotationType) e).getMember()) {
                    seen.add(AnnotationMember.class);
                    seen.add(Member.class);
                    AnnotationMemberValue value = am.getDefaultValue();
                    seen.add(AnnotationMemberValue.class);
                    assertEquals(TypeKind.INT_32_TYPE, value.kind());
                    assertEquals(12, value.value());
                }
            } else if (e instanceof UnionType) {
                seen.add(UnionType.class);
                for (UnionMember um : ((UnionType) e).getMember()) {
                    seen.add(UnionMember.class);
                    seen.add(Member.class);
                    assertEquals(List.of(1, 2), um.getLabel());
                }
            } else if (e instanceof StructureType) {
                seen.add(StructureType.class);
                for (Member m : ((StructureType) e).getMember()) {
                    seen.add(Member.class);
                    MemberProperty mp = m.getProperty();
                    seen.add(MemberProperty.class);
                    assertEquals("k", mp.getName());
                    assertEquals(5, mp.getMemberId());
                    assertEquals(TypeObjectModel.memberFlags(mp),
                            ZdTypes.DDS.MemberFlag.IS_KEY | ZdTypes.DDS.MemberFlag.IS_OPTIONAL);
                    for (AnnotationUsage usage : m.getAnnotation()) {
                        seen.add(AnnotationUsage.class);
                        for (AnnotationUsageMember um : usage.getMember()) {
                            seen.add(AnnotationUsageMember.class);
                            assertEquals(1, um.getMemberId());
                            assertEquals("text", um.getValue().value());
                        }
                    }
                }
            } else if (e instanceof Module) {
                seen.add(Module.class);
                assertEquals("mod", ((Module) e).getName());
                assertTrue(((Module) e).getLibrary().getElement().isEmpty());
            }
        }
        Set<Class<?>> missing = new HashSet<>();
        for (Class<?> c : all()) {
            if (!seen.contains(c)) {
                missing.add(c);
            }
        }
        assertTrue(missing.isEmpty(), "no object of: " + missing);
        // the root is the structure
        assertTrue(TypeObjectModel.element(library, object.getTheType()) instanceof StructureType);
    }

    private static Class<?>[] all() {
        return new Class<?>[] {AliasType.class, AnnotationMember.class, AnnotationMemberValue.class,
            AnnotationType.class, AnnotationUsage.class, AnnotationUsageMember.class, ArrayType.class,
            Bit.class, BitSetType.class, CollectionType.class, EnumeratedConstant.class,
            EnumerationType.class, MapType.class, Member.class, MemberProperty.class, Module.class,
            SequenceType.class, StringType.class, StructureType.class, Type.class, TypeLibrary.class,
            TypeLibraryElement.class, TypeObject.class, TypeProperty.class, UnionMember.class,
            UnionType.class};
    }

    @Test
    void theObjectsAreSerializable() throws Exception {
        TypeObject object = TypeObjectModel.of(org.omg.dds.Qos.env(), sample());
        ByteArrayOutputStream out = new ByteArrayOutputStream();
        try (ObjectOutputStream o = new ObjectOutputStream(out)) {
            o.writeObject(object);
        }
        try (ObjectInputStream in = new ObjectInputStream(new ByteArrayInputStream(out.toByteArray()))) {
            TypeObject back = (TypeObject) in.readObject();
            assertEquals(object.getLibrary().getElement().size(), back.getLibrary().getElement().size());
            assertEquals(object.getTheType(), back.getTheType());
        }
    }

    @Test
    void equalStateIsEqualObjects() {
        TypeObject one = TypeObjectModel.of(org.omg.dds.Qos.env(), sample());
        TypeObject two = TypeObjectModel.of(org.omg.dds.Qos.env(), sample());
        assertEquals(one, two);
        assertEquals(one.hashCode(), two.hashCode());
        assertEquals(one.getLibrary().getElement().get(0), two.getLibrary().getElement().get(0));
        ZdTypes.DDS.TypeObject changed = sample();
        changed.library.element[0].alias_type.property.name = "Other";
        org.junit.jupiter.api.Assertions.assertNotEquals(one.getLibrary().getElement().get(0),
                TypeObjectModel.of(org.omg.dds.Qos.env(), changed).getLibrary().getElement().get(0));
    }

    @Test
    void theIdsOfTheLibraryAreTheHashesOfTheirTypes() {
        ZdTypes.DDS.TypeObject object = sample();
        for (ZdTypes.DDS.TypeLibraryElement e : object.library.element) {
            if (e._d == ZdTypes.DDS.TypeLibraryElementKind.MODULE_ELEMENT) {
                continue;
            }
            ZdTypes.DDS.TypeId id = TypeObjectCodec.propertyOf(e).type_id;
            TypeObjectCodec.propertyOf(e).type_id = TypeObjectCodec.constructed(ZdTypes.DDS.NO_TYPE, 0L);
            assertEquals(id.constructed_type_id, TypeObjectCodec.constructedTypeId(e));
            TypeObjectCodec.propertyOf(e).type_id = id;
        }
        byte[] first = TypeObjectCodec.encode(object);
        assertArrayEquals(first, TypeObjectCodec.encode(TypeObjectCodec.decode(first)));
    }
}
