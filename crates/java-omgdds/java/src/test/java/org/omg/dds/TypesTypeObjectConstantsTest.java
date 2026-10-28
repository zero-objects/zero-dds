// SPDX-License-Identifier: Apache-2.0
package org.omg.dds;

import org.junit.jupiter.api.Test;
import org.omg.dds.type.ID;
import org.omg.dds.type.TypeKind;
import org.omg.dds.type.typeobject.AliasType;
import org.omg.dds.type.typeobject.AnnotationMember;
import org.omg.dds.type.typeobject.AnnotationType;
import org.omg.dds.type.typeobject.ArrayType;
import org.omg.dds.type.typeobject.BitSetType;
import org.omg.dds.type.typeobject.CollectionType;
import org.omg.dds.type.typeobject.EnumerationType;
import org.omg.dds.type.typeobject.MapType;
import org.omg.dds.type.typeobject.Member;
import org.omg.dds.type.typeobject.SequenceType;
import org.omg.dds.type.typeobject.StringType;
import org.omg.dds.type.typeobject.StructureType;
import org.omg.dds.type.typeobject.Type;
import org.omg.dds.type.typeobject.TypeId;
import org.omg.dds.type.typeobject.UnionMember;
import org.omg.dds.type.typeobject.UnionType;

import java.lang.reflect.Constructor;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.lang.reflect.Modifier;
import java.util.Map;
import java.util.Set;
import java.util.TreeMap;
import java.util.TreeSet;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * DDS-Java-PSM 1.0 §7.8.3: the types of the TypeObject representation are expressed by the
 * mapping rules of the document, with two changes. The constants of the top level are moved
 * into the interfaces that relate to them ({@code Member.MEMBER_ID_INVALID}), and the member
 * ids of a type are the constant integer fields of a final class {@code MemberId} nested in
 * the interface ({@code MapType.MemberId.BOUND_MAPTYPE_MEMBER_ID}).
 *
 * <p>The values of the member ids are those of the OMG sources of the PSM. The TypeObject
 * type library they belong to is that of DDS-XTypes 1.1 Annex B, which is not among the
 * specifications of this repository (DDS-XTypes 1.3 replaced it with the TypeIdentifier
 * based TypeObject), so these tests check the constants against the sources of the interfaces
 * and against the {@code @ID} annotations of the same interfaces, not against the text of that annex.
 */
class TypesTypeObjectConstantsTest {

    private static Map<String, Integer> constantsOf(Class<?> memberId) throws Exception {
        Map<String, Integer> out = new TreeMap<>();
        for (Field f : memberId.getDeclaredFields()) {
            if (f.isSynthetic()) {
                continue;
            }
            int m = f.getModifiers();
            assertTrue(Modifier.isPublic(m) && Modifier.isStatic(m) && Modifier.isFinal(m),
                    f + " is a public static final constant");
            assertEquals(int.class, f.getType(), f + " is an int");
            out.put(f.getName(), f.getInt(null));
        }
        return out;
    }

    private static Map<String, Integer> expected(Object... nameAndValue) {
        Map<String, Integer> out = new TreeMap<>();
        for (int i = 0; i < nameAndValue.length; i += 2) {
            out.put((String) nameAndValue[i], (Integer) nameAndValue[i + 1]);
        }
        return out;
    }

    /** The nested MemberId of an interface: a final class with a private constructor and the constants. */
    private static void checkMemberId(Class<?> owner, Map<String, Integer> constants) throws Exception {
        Class<?> memberId = null;
        for (Class<?> nested : owner.getDeclaredClasses()) {
            if (nested.getSimpleName().equals("MemberId")) {
                memberId = nested;
            }
        }
        assertTrue(memberId != null, owner.getSimpleName() + " has a nested class MemberId");
        assertTrue(Modifier.isFinal(memberId.getModifiers()), owner.getSimpleName() + ".MemberId is final");
        assertTrue(Modifier.isPublic(memberId.getModifiers()));
        assertTrue(Modifier.isStatic(memberId.getModifiers()));
        for (Constructor<?> c : memberId.getDeclaredConstructors()) {
            assertTrue(Modifier.isPrivate(c.getModifiers()), "no public constructor");
        }
        assertEquals(constants, constantsOf(memberId), owner.getSimpleName() + ".MemberId");

        // The member ids are the ids of the properties of the interface: every constant is the
        // @ID of one getter of the interface, and every @ID of a getter is one of the constants.
        Set<Integer> annotated = new TreeSet<>();
        for (Method m : owner.getDeclaredMethods()) {
            ID id = m.getAnnotation(ID.class);
            if (id != null) {
                annotated.add((int) id.value());
            }
        }
        assertEquals(new TreeSet<>(constants.values()), annotated,
                owner.getSimpleName() + ": the @ID of the getters");
    }

    @Test
    void memberIdsAreNestedFinalClassesOfIntConstants() throws Exception {
        // This is the example of the clause.
        assertEquals(201, MapType.MemberId.BOUND_MAPTYPE_MEMBER_ID);
        assertEquals(200, MapType.MemberId.KEY_ELEMENT_TYPE_MAPTYPE_MEMBER_ID);

        checkMemberId(AliasType.class, expected("BASE_TYPE_ALIASTYPE_MEMBER_ID", 100));
        checkMemberId(AnnotationMember.class,
                expected("DEFAULT_VALUE_ANNOTATIONMEMBER_MEMBER_ID", 100));
        checkMemberId(AnnotationType.class, expected(
                "BASE_TYPE_ANNOTATIONTYPE_MEMBER_ID", 100, "MEMBER_ANNOTATIONTYPE_MEMBER_ID", 101));
        checkMemberId(ArrayType.class, expected("BOUND_ARRAYTYPE_MEMBER_ID", 200));
        checkMemberId(BitSetType.class, expected(
                "BIT_BOUND_BITSETTYPE_MEMBER_ID", 100, "BIT_BITSETTYPE_MEMBER_ID", 101));
        checkMemberId(CollectionType.class, expected("ELEMENT_TYPE_COLLECTIONTYPE_MEMBER_ID", 100));
        checkMemberId(EnumerationType.class, expected(
                "BIT_BOUND_ENUMERATIONTYPE_MEMBER_ID", 100, "CONSTANT_ENUMERATIONTYPE_MEMBER_ID", 101));
        checkMemberId(MapType.class, expected(
                "KEY_ELEMENT_TYPE_MAPTYPE_MEMBER_ID", 200, "BOUND_MAPTYPE_MEMBER_ID", 201));
        checkMemberId(Member.class, expected(
                "PROPERTY_MEMBER_MEMBER_ID", 0, "ANNOTATION_MEMBER_MEMBER_ID", 1));
        checkMemberId(SequenceType.class, expected("BOUND_SEQUENCETYPE_MEMBER_ID", 200));
        checkMemberId(StringType.class, expected("BOUND_STRINGTYPE_MEMBER_ID", 200));
        checkMemberId(StructureType.class, expected(
                "BASE_TYPE_STRUCTURETYPE_MEMBER_ID", 100, "MEMBER_STRUCTURETYPE_MEMBER_ID", 101));
        checkMemberId(Type.class, expected(
                "PROPERTY_TYPE_MEMBER_ID", 0, "ANNOTATION_TYPE_MEMBER_ID", 1));
        checkMemberId(UnionMember.class, expected("LABEL_UNIONMEMBER_MEMBER_ID", 100));
        checkMemberId(UnionType.class, expected("MEMBER_UNIONTYPE_MEMBER_ID", 100));
    }

    @Test
    void theConstantsOfTheTopLevelAreMovedIntoTheInterfaces() {
        // DDS-XTypes 1.3 §7.2.2.4.4.4.4: a member id has 28 bits, from 0 to 0x0FFFFFFF; the
        // sentinel for a missing id is the last of them, as in crates/types (MEMBER_ID_INVALID).
        assertEquals(0x0FFFFFFF, Member.MEMBER_ID_INVALID);
        assertEquals(268435455, Member.MEMBER_ID_INVALID);
        assertEquals(0L, CollectionType.UNBOUNDED_COLLECTION);
        // The ids of the primitive types are those of the type kinds.
        assertEquals(TypeKind.Values.NO_TYPE_VALUE, TypeId.NO_TYPE_ID);
        assertEquals(TypeKind.Values.BOOLEAN_TYPE_VALUE, TypeId.BOOLEAN_TYPE_ID);
        assertEquals(TypeKind.Values.BYTE_TYPE_VALUE, TypeId.BYTE_TYPE_ID);
        assertEquals(TypeKind.Values.INT_16_TYPE_VALUE, TypeId.INT_16_TYPE_ID);
        assertEquals(TypeKind.Values.UINT_16_TYPE_VALUE, TypeId.UINT_16_TYPE_ID);
        assertEquals(TypeKind.Values.INT_32_TYPE_VALUE, TypeId.INT_32_TYPE_ID);
        assertEquals(TypeKind.Values.UINT_32_TYPE_VALUE, TypeId.UINT_32_TYPE_ID);
        assertEquals(TypeKind.Values.INT_64_TYPE_VALUE, TypeId.INT_64_TYPE_ID);
        assertEquals(TypeKind.Values.UINT_64_TYPE_VALUE, TypeId.UINT_64_TYPE_ID);
        assertEquals(TypeKind.Values.FLOAT_32_TYPE_VALUE, TypeId.FLOAT_32_TYPE_ID);
        assertEquals(TypeKind.Values.FLOAT_64_TYPE_VALUE, TypeId.FLOAT_64_TYPE_ID);
        assertEquals(TypeKind.Values.FLOAT_128_TYPE_VALUE, TypeId.FLOAT_128_TYPE_ID);
        assertEquals(TypeKind.Values.CHAR_8_TYPE_VALUE, TypeId.CHAR_8_TYPE_ID);
        assertEquals(TypeKind.Values.CHAR_32_TYPE_VALUE, TypeId.CHAR_32_TYPE_ID);
    }

    @Test
    void theInterfacesTheConstantsLiveInAreTheOnesOfTheTopLevelTypes() {
        // The constant is a field of the interface: it is reached as Member.MEMBER_ID_INVALID
        // and not as a member of a holder class.
        for (Field f : Member.class.getDeclaredFields()) {
            if (f.getName().equals("MEMBER_ID_INVALID")) {
                assertTrue(Modifier.isPublic(f.getModifiers()) && Modifier.isStatic(f.getModifiers())
                        && Modifier.isFinal(f.getModifiers()));
                assertEquals(int.class, f.getType());
                return;
            }
        }
        throw new AssertionError("Member.MEMBER_ID_INVALID is not declared in Member");
    }
}
