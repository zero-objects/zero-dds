// SPDX-License-Identifier: Apache-2.0
package org.zerodds.typeobject;

import java.util.ArrayList;
import java.util.List;

import org.junit.jupiter.api.Test;
import org.omg.dds.type.Extensibility;
import org.omg.dds.type.Optional;
import org.omg.dds.type.dynamic.DynamicType;
import org.omg.dds.type.typeobject.EnumerationType;
import org.omg.dds.type.typeobject.Member;
import org.omg.dds.type.typeobject.MemberFlag;
import org.omg.dds.type.typeobject.SequenceType;
import org.omg.dds.type.typeobject.StructureType;
import org.omg.dds.type.typeobject.TypeFlag;
import org.omg.dds.type.typeobject.TypeId;
import org.omg.dds.type.typeobject.TypeLibraryElement;
import org.omg.dds.type.typeobject.TypeObject;
import org.zerodds.cdr.DynamicTypeFactoryImpl;
import org.zerodds.cdr.ReflectionTypeSupport;
import org.zerodds.typeobject.generated.ZdTypes;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * The XTypes 1.1 TypeObject of Annex B ({@code type}, PID 0x0072) and its Java PSM interfaces.
 */
class TypeObjectTest {

    public enum Color { RED, GREEN }

    @Extensibility(Extensibility.Kind.MUTABLE_EXTENSIBILITY)
    public static final class Shape {
        @org.omg.dds.type.Key
        public int id;
        public String label;
        @Optional
        public Integer weight;
        public List<Short> points;
        public Color color;
    }

    private static byte[] hex(String h) {
        byte[] out = new byte[h.length() / 2];
        for (int i = 0; i < out.length; i++) {
            out[i] = (byte) Integer.parseInt(h.substring(2 * i, 2 * i + 2), 16);
        }
        return out;
    }

    private static DynamicType shapeType() {
        return DynamicTypeFactoryImpl.getInstance(org.omg.dds.Qos.env()).createType(Shape.class);
    }

    /**
     * A TypeObject with an empty library and the primitive int32 as its type, written by hand
     * from Annex B: the mutable structure is a parameter list of PL_CDR (XCDR1, little-endian),
     * member id 0 holds the library (an empty sequence, ulong 0), member id 1 holds
     * {@code the_type} (a sequence of one TypeId: ulong 1, the discriminator short 5, the
     * primitive id short 5), the sentinel ends the list.
     */
    @Test
    void theMinimalTypeObjectHasTheOctetsAnnexBImplies() {
        ZdTypes.DDS.TypeObject object = new ZdTypes.DDS.TypeObject();
        object.library = new ZdTypes.DDS.TypeLibrary();
        object.library.element = new ZdTypes.DDS.TypeLibraryElement[0];
        object.the_type = new ZdTypes.DDS.TypeId[] {
            TypeObjectCodec.primitive(ZdTypes.DDS.INT_32_TYPE)};
        byte[] expected = hex("0000" + "0400" + "00000000"
                + "0100" + "0800" + "01000000" + "0500" + "0500"
                + "027f" + "0000");
        assertArrayEquals(expected, TypeObjectCodec.encode(object));
        ZdTypes.DDS.TypeObject back = TypeObjectCodec.decode(expected);
        assertEquals(1, back.the_type.length);
        assertEquals(ZdTypes.DDS.INT_32_TYPE, back.the_type[0].primitive_type_id);
    }

    @Test
    void aJavaClassBecomesAStructureTypeWithItsMembers() {
        byte[] bytes = TypeObjects.announce(org.omg.dds.Qos.env(),
                ReflectionTypeSupport.of(Shape.class));
        assertNotNull(bytes);
        TypeObject object = TypeObjects.decodeOrNull(org.omg.dds.Qos.env(), bytes);
        assertNotNull(object);
        TypeLibraryElement root = TypeObjectModel.element(object.getLibrary(), object.getTheType());
        assertTrue(root instanceof StructureType);
        StructureType shape = (StructureType) root;
        assertTrue(shape.getProperty().getName().endsWith("Shape"));
        assertEquals(TypeFlag.IS_MUTABLE, shape.getProperty().getFlag());
        assertEquals(0, shape.getBaseType());
        List<Member> members = shape.getMember();
        assertEquals(5, members.size());
        assertEquals("id", members.get(0).getProperty().getName());
        assertEquals(TypeId.INT_32_TYPE_ID, members.get(0).getProperty().getTypeId());
        assertEquals(MemberFlag.IS_KEY, members.get(0).getProperty().getFlag());
        assertEquals(MemberFlag.IS_OPTIONAL, members.get(2).getProperty().getFlag());
        assertNull(members.get(1).getProperty().getFlag());

        TypeLibraryElement text = TypeObjectModel.element(object.getLibrary(),
                members.get(1).getProperty().getTypeId());
        assertTrue(text instanceof org.omg.dds.type.typeobject.StringType);
        TypeLibraryElement points = TypeObjectModel.element(object.getLibrary(),
                members.get(3).getProperty().getTypeId());
        assertTrue(points instanceof SequenceType);
        assertEquals(TypeId.INT_16_TYPE_ID, ((SequenceType) points).getElementType());
        assertEquals(0L, ((SequenceType) points).getBound());
        TypeLibraryElement color = TypeObjectModel.element(object.getLibrary(),
                members.get(4).getProperty().getTypeId());
        assertTrue(color instanceof EnumerationType);
        assertEquals("RED", ((EnumerationType) color).getConstant().get(0).getName());
        assertEquals(1, ((EnumerationType) color).getConstant().get(1).getValue());
    }

    @Test
    void theTypeIdIsTheLowerHalfOfTheMd5OfTheBigEndianSerialization() {
        ZdTypes.DDS.TypeObject object = TypeObjectBuilder.build(shapeType());
        ZdTypes.DDS.TypeId root = object.the_type[0];
        assertEquals(ZdTypes.DDS.STRUCTURE_TYPE, root._d);
        ZdTypes.DDS.TypeLibraryElement element = null;
        for (ZdTypes.DDS.TypeLibraryElement e : object.library.element) {
            if (e._d == ZdTypes.DDS.TypeLibraryElementKind.STRUCTURE_TYPE_ELEMENT) {
                element = e;
            }
        }
        assertNotNull(element);
        // the element carries its own id; hashing needs the placeholder of the standard
        ZdTypes.DDS.TypeId own = element.structure_type.property.type_id;
        element.structure_type.property.type_id = TypeObjectCodec.constructed(ZdTypes.DDS.NO_TYPE, 0L);
        long again = TypeObjectCodec.constructedTypeId(element);
        element.structure_type.property.type_id = own;
        assertEquals(root.constructed_type_id, again);
        assertEquals(root.constructed_type_id, own.constructed_type_id);
    }

    @Test
    void aChangeOfTheTypeChangesItsId() {
        long before = TypeObjectBuilder.build(shapeType()).the_type[0].constructed_type_id;
        long same = TypeObjectBuilder.build(shapeType()).the_type[0].constructed_type_id;
        assertEquals(before, same);
        DynamicType other = DynamicTypeFactoryImpl.getInstance(org.omg.dds.Qos.env())
                .createType(Other.class);
        assertNotEquals(before, TypeObjectBuilder.build(other).the_type[0].constructed_type_id);
    }

    public static final class Other {
        public int id;
    }

    @Test
    void theEncodedTypeObjectIsDecodedToTheSameTypeObject() {
        ZdTypes.DDS.TypeObject object = TypeObjectBuilder.build(shapeType());
        byte[] bytes = TypeObjectCodec.encode(object);
        assertArrayEquals(bytes, TypeObjectCodec.encode(TypeObjectCodec.decode(bytes)));
    }

    @Test
    void bytesThatAreNoTypeObjectGiveNoType() {
        assertNull(TypeObjects.decodeOrNull(org.omg.dds.Qos.env(), new byte[] {1, 2, 3}));
    }

    @Test
    void theBuiltinDataDescribesItsTypeFromTheAnnouncedBytes() {
        byte[] bytes = TypeObjects.announce(org.omg.dds.Qos.env(),
                ReflectionTypeSupport.of(Shape.class));
        org.zerodds.topic.PublicationBuiltinTopicDataImpl data =
                new org.zerodds.topic.PublicationBuiltinTopicDataImpl(org.omg.dds.Qos.env());
        assertNull(data.getType());
        data.setTypeObjectBytes(bytes);
        assertNotNull(data.getType());
        assertNotNull(TypeObjectModel.element(data.getType().getLibrary(),
                data.getType().getTheType()));
        assertArrayEquals(bytes, data.getTypeObjectBytes());
    }

    /**
     * The member ids of the MemberId classes are those of the enumerations of Annex B of
     * XTypes 1.1 (TypeMemberId, MemberMemberId, StructureTypeMemberId, UnionMemberMemberId,
     * UnionTypeMemberId, AnnotationMemberMemberId, AnnotationTypeMemberId, AliasTypeMemberId,
     * CollectionTypeMemberId, ArrayTypeMemberId, MapTypeMemberId, SequenceTypeMemberId,
     * StringTypeMemberId, BitSetTypeMemberId, EnumerationTypeMemberId); CollectionType.MemberId of the OMG source
     * has no ELEMENT_SHARED_COLLECTIONTYPE_MEMBER_ID (101), which Annex B has.
     */
    @Test
    void theMemberIdsAreThoseOfAnnexBOfXTypes11() {
        assertEquals(0, org.omg.dds.type.typeobject.Type.MemberId.PROPERTY_TYPE_MEMBER_ID);
        assertEquals(1, org.omg.dds.type.typeobject.Type.MemberId.ANNOTATION_TYPE_MEMBER_ID);
        assertEquals(0, org.omg.dds.type.typeobject.Member.MemberId.PROPERTY_MEMBER_MEMBER_ID);
        assertEquals(1, org.omg.dds.type.typeobject.Member.MemberId.ANNOTATION_MEMBER_MEMBER_ID);
        assertEquals(100, StructureType.MemberId.BASE_TYPE_STRUCTURETYPE_MEMBER_ID);
        assertEquals(101, StructureType.MemberId.MEMBER_STRUCTURETYPE_MEMBER_ID);
        assertEquals(100, org.omg.dds.type.typeobject.UnionMember.MemberId.LABEL_UNIONMEMBER_MEMBER_ID);
        assertEquals(100, org.omg.dds.type.typeobject.UnionType.MemberId.MEMBER_UNIONTYPE_MEMBER_ID);
        assertEquals(100, org.omg.dds.type.typeobject.AnnotationMember.MemberId
                .DEFAULT_VALUE_ANNOTATIONMEMBER_MEMBER_ID);
        assertEquals(100, org.omg.dds.type.typeobject.AnnotationType.MemberId.BASE_TYPE_ANNOTATIONTYPE_MEMBER_ID);
        assertEquals(101, org.omg.dds.type.typeobject.AnnotationType.MemberId.MEMBER_ANNOTATIONTYPE_MEMBER_ID);
        assertEquals(100, org.omg.dds.type.typeobject.AliasType.MemberId.BASE_TYPE_ALIASTYPE_MEMBER_ID);
        assertEquals(100, org.omg.dds.type.typeobject.CollectionType.MemberId
                .ELEMENT_TYPE_COLLECTIONTYPE_MEMBER_ID);
        assertEquals(200, org.omg.dds.type.typeobject.ArrayType.MemberId.BOUND_ARRAYTYPE_MEMBER_ID);
        assertEquals(200, org.omg.dds.type.typeobject.MapType.MemberId.KEY_ELEMENT_TYPE_MAPTYPE_MEMBER_ID);
        assertEquals(201, org.omg.dds.type.typeobject.MapType.MemberId.BOUND_MAPTYPE_MEMBER_ID);
        assertEquals(200, org.omg.dds.type.typeobject.SequenceType.MemberId.BOUND_SEQUENCETYPE_MEMBER_ID);
        assertEquals(200, org.omg.dds.type.typeobject.StringType.MemberId.BOUND_STRINGTYPE_MEMBER_ID);
        assertEquals(100, org.omg.dds.type.typeobject.BitSetType.MemberId.BIT_BOUND_BITSETTYPE_MEMBER_ID);
        assertEquals(101, org.omg.dds.type.typeobject.BitSetType.MemberId.BIT_BITSETTYPE_MEMBER_ID);
        assertEquals(100, org.omg.dds.type.typeobject.EnumerationType.MemberId
                .BIT_BOUND_ENUMERATIONTYPE_MEMBER_ID);
        assertEquals(101, org.omg.dds.type.typeobject.EnumerationType.MemberId
                .CONSTANT_ENUMERATIONTYPE_MEMBER_ID);
    }
}
