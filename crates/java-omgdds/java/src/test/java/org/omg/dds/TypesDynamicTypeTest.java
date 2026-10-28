// SPDX-License-Identifier: Apache-2.0
package org.omg.dds;

import org.junit.jupiter.api.Test;
import org.omg.dds.core.ServiceEnvironment;
import org.omg.dds.type.TypeKind;
import org.omg.dds.type.dynamic.AnnotationDescriptor;
import org.omg.dds.type.dynamic.DynamicData;
import org.omg.dds.type.dynamic.DynamicDataFactory;
import org.omg.dds.type.dynamic.DynamicType;
import org.omg.dds.type.dynamic.DynamicTypeFactory;
import org.omg.dds.type.dynamic.DynamicTypeMember;
import org.omg.dds.type.dynamic.MemberDescriptor;
import org.omg.dds.type.dynamic.TypeDescriptor;
import org.zerodds.ServiceEnvironmentImpl;
import org.zerodds.cdr.DynamicDataTypeSupport;
import org.zerodds.cdr.DynamicTypeFactoryImpl;
import org.zerodds.cdr.ReflectionTypeSupport;

import java.lang.reflect.Method;
import java.util.Arrays;
import java.util.List;
import java.util.Map;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotEquals;
import static org.junit.jupiter.api.Assertions.assertNotSame;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * DDS-Java-PSM 1.0 §7.8.1.1, §7.8.1.3 and §7.8.1.5: the {@code DynamicTypeFactory} of a
 * {@code ServiceEnvironment}, the interfaces {@code DynamicType} and
 * {@code DynamicTypeMember} (equals and clone as {@code Object.equals} and
 * {@code Object.clone}, unmodifiable annotation lists), {@code createType(Class)} with the
 * superclass as base type (§8.6.1.1) and the immutable descriptors.
 */
class TypesDynamicTypeTest {

    private static final ServiceEnvironment ENV = Qos.env();
    private static final DynamicTypeFactory TYPES = DynamicTypeFactory.getInstance(ENV);

    private static DynamicType prim(TypeKind kind) {
        return TYPES.getPrimitiveType(kind);
    }

    private static AnnotationDescriptor annotation(String name, String value) {
        DynamicType type = TYPES.createType(TYPES.newTypeDescriptor()
                .withKind(TypeKind.ANNOTATION_TYPE).withName(name));
        AnnotationDescriptor a = TYPES.newAnnotationDescriptor().withType(type);
        return value == null ? a : a.withValue("value", value);
    }

    private static MemberDescriptor descriptor(String name, int id, DynamicType type) {
        return TYPES.newMemberDescriptor().withName(name).withId(id).withType(type);
    }

    private static DynamicType struct(String name) {
        return TYPES.createType(TYPES.newTypeDescriptor()
                .withKind(TypeKind.STRUCTURE_TYPE).withName(name));
    }

    /** struct Point3 { long x; long y; long z; } with the ids 1, 2 and 3. */
    private static DynamicType point3() {
        DynamicType type = struct("TypesPoint3");
        type.addMembers(descriptor("x", 1, prim(TypeKind.INT_32_TYPE)),
                descriptor("y", 2, prim(TypeKind.INT_32_TYPE)),
                descriptor("z", 3, prim(TypeKind.INT_32_TYPE)));
        return type;
    }

    // --- §7.8.1.1 The factory ------------------------------------------------

    @Test
    void theFactoryIsASingletonPerServiceEnvironment() {
        DynamicTypeFactory first = DynamicTypeFactory.getInstance(ENV);
        assertSame(first, DynamicTypeFactory.getInstance(ENV));
        assertSame(ENV.getSPI().getTypeFactory(), first);
        assertSame(ENV, first.getEnvironment());

        ServiceEnvironment other = new ServiceEnvironmentImpl();
        DynamicTypeFactory second = DynamicTypeFactory.getInstance(other);
        assertNotSame(first, second);
        assertSame(second, DynamicTypeFactory.getInstance(other));
        assertSame(other, second.getEnvironment());
        // What a factory builds belongs to it: the two do not share their primitive types.
        assertNotSame(first.getPrimitiveType(TypeKind.INT_32_TYPE),
                second.getPrimitiveType(TypeKind.INT_32_TYPE));
        assertSame(first.getPrimitiveType(TypeKind.INT_32_TYPE),
                first.getPrimitiveType(TypeKind.INT_32_TYPE));
        assertSame(other, second.getPrimitiveType(TypeKind.INT_32_TYPE).getEnvironment());
    }

    @Test
    void thereIsNoDeleteInstanceAndTheServiceManagesTheLifeCycle() {
        // §7.8.1.1: the static delete_instance operations of DDS-XTypes are omitted.
        for (Class<?> cls : new Class<?>[] {DynamicTypeFactory.class, DynamicTypeFactoryImpl.class}) {
            for (Method m : cls.getMethods()) {
                assertFalse(m.getName().toLowerCase().contains("delete"), m.toString());
            }
            for (Method m : cls.getDeclaredMethods()) {
                assertFalse(m.getName().toLowerCase().contains("delete"), m.toString());
            }
        }
        // The one static operation the abstract factory has is getInstance(ServiceEnvironment).
        int statics = 0;
        for (Method m : DynamicTypeFactory.class.getDeclaredMethods()) {
            if (java.lang.reflect.Modifier.isStatic(m.getModifiers())) {
                statics++;
                assertEquals("getInstance", m.getName());
            }
        }
        assertEquals(1, statics);
    }

    // --- §7.8.1.3 DynamicType and DynamicTypeMember --------------------------

    @Test
    void membersAreFoundByIdAndByName() {
        DynamicType type = point3();
        assertEquals("TypesPoint3", type.getName());
        assertEquals(TypeKind.STRUCTURE_TYPE, type.getKind());
        assertEquals("TypesPoint3", type.getDescriptor().getName());

        DynamicTypeMember y = type.getMember(2);
        assertEquals(2, y.getId());
        assertEquals("y", y.getName());
        assertEquals("y", y.getDescriptor().getName());
        assertEquals(TypeKind.INT_32_TYPE, y.getDescriptor().getType().getKind());
        assertSame(y, type.getMember("y"));
        assertNull(type.getMember(99));
        assertNull(type.getMember("w"));

        Map<Integer, DynamicTypeMember> all = type.getAllMembers();
        assertEquals(Arrays.asList(1, 2, 3), Arrays.asList(all.keySet().toArray()));
        Map<Integer, DynamicTypeMember> byId = type.getAllMembers(3, 1, 99);
        assertEquals(2, byId.size());
        assertSame(type.getMember(3), byId.get(3));
        assertSame(type.getMember(1), byId.get(1));
        Map<String, DynamicTypeMember> byName = type.getAllMembers("z", "nope", "x");
        assertEquals(2, byName.size());
        assertSame(type.getMember("z"), byName.get("z"));
        assertSame(type.getMember("x"), byName.get("x"));
    }

    @Test
    void addMembersAddsEachDescriptorAndReturnsTheMembersOfTheType() {
        DynamicType type = struct("TypesPair");
        List<DynamicTypeMember> added = type.addMembers(
                descriptor("a", 1, prim(TypeKind.INT_16_TYPE)),
                descriptor("b", 2, prim(TypeKind.FLOAT_64_TYPE)));
        assertEquals(2, added.size());
        assertSame(type.getMember(1), added.get(0));
        assertSame(type.getMember(2), added.get(1));
        assertEquals(0, type.addMembers().size());
        // addMember returns the member the type holds: what is set on it is seen through the type.
        DynamicTypeMember c = type.addMember(descriptor("c", 3, prim(TypeKind.BOOLEAN_TYPE)));
        c.setAnnotation(annotation("Key", null));
        assertEquals(1, type.getMember("c").getAnnotations().size());
    }

    @Test
    void annotationsOfATypeAreReadByIndexAndByName() {
        DynamicType type = point3();
        assertEquals(0, type.getAllAnnotations().size());
        assertNull(type.getAnnotation(0));
        assertNull(type.getAnnotation("Extensibility"));

        type.setAnnotation(annotation("Nested", null));
        assertEquals(1, type.getAllAnnotations().size());
        assertEquals("Nested", type.getAnnotation(0).getType().getName());

        // setAnnotations replaces what the type had.
        type.setAnnotations(annotation("Extensibility", "FINAL"), annotation("Nested", null));
        Map<Integer, AnnotationDescriptor> all = type.getAllAnnotations();
        assertEquals(2, all.size());
        assertEquals("Extensibility", type.getAnnotation(0).getType().getName());
        assertEquals("FINAL", type.getAnnotation("Extensibility").getValue("value"));
        assertEquals("Nested", type.getAnnotation(1).getType().getName());
        assertNull(type.getAnnotation(2));
        assertNull(type.getAnnotation("Verbatim"));

        Map<Integer, AnnotationDescriptor> byIndex = type.getAnnotations(1, 7);
        assertEquals(1, byIndex.size());
        assertEquals("Nested", byIndex.get(1).getType().getName());
        Map<String, AnnotationDescriptor> byName = type.getAnnotations("Nested", "Verbatim");
        assertEquals(1, byName.size());
        assertSame(type.getAnnotation("Nested"), byName.get("Nested"));
    }

    @Test
    void annotationsOfAMemberAreAListInTheOrderTheyWereSet() {
        DynamicType type = point3();
        DynamicTypeMember x = type.getMember("x");
        assertTrue(x.getAnnotations().isEmpty());
        x.setAnnotations(annotation("Key", null), annotation("ID", "7"));
        List<AnnotationDescriptor> list = x.getAnnotations();
        assertEquals(2, list.size());
        assertEquals("Key", list.get(0).getType().getName());
        assertEquals("ID", list.get(1).getType().getName());
        assertEquals("7", list.get(1).getValue("value"));
        x.setAnnotation(annotation("Optional", null));
        assertEquals(1, x.getAnnotations().size());
        // Another member is not affected.
        assertTrue(type.getMember("y").getAnnotations().isEmpty());
    }

    @Test
    void theListsAndMapsOfAnnotationsCannotBeModified() {
        DynamicType type = point3();
        type.setAnnotations(annotation("Extensibility", "FINAL"), annotation("Nested", null));
        DynamicTypeMember x = type.getMember("x");
        x.setAnnotation(annotation("Key", null));
        AnnotationDescriptor extra = annotation("Optional", null);

        assertThrows(UnsupportedOperationException.class, () -> x.getAnnotations().add(extra));
        assertThrows(UnsupportedOperationException.class, () -> x.getAnnotations().clear());
        assertThrows(UnsupportedOperationException.class, () -> x.getAnnotations().remove(0));
        assertThrows(UnsupportedOperationException.class, () -> type.getAllAnnotations().put(9, extra));
        assertThrows(UnsupportedOperationException.class, () -> type.getAllAnnotations().clear());
        assertThrows(UnsupportedOperationException.class, () -> type.getAnnotations(0).put(9, extra));
        assertThrows(UnsupportedOperationException.class,
                () -> type.getAnnotations("Nested").put("Optional", extra));

        // What was refused did not change the type.
        assertEquals(1, x.getAnnotations().size());
        assertEquals(2, type.getAllAnnotations().size());
    }

    // --- equals and clone as Object.equals and Object.clone ------------------

    @Test
    void twoTypesWithTheSamePropertiesAreEqual() {
        DynamicType a = point3();
        DynamicType b = point3();
        assertNotSame(a, b);
        assertEquals(a, b);
        assertEquals(b, a);
        assertEquals(a.hashCode(), b.hashCode());
        assertEquals(a, a);
        assertNotEquals(a, null);
        assertNotEquals(a, "TypesPoint3");

        // A member added to one: no longer equal; added to the other too: equal again.
        a.addMember(descriptor("w", 4, prim(TypeKind.INT_32_TYPE)));
        assertNotEquals(a, b);
        b.addMember(descriptor("w", 4, prim(TypeKind.INT_32_TYPE)));
        assertEquals(a, b);

        // A different name, member type or annotation.
        DynamicType named = struct("TypesPoint3b");
        named.addMembers(descriptor("x", 1, prim(TypeKind.INT_32_TYPE)),
                descriptor("y", 2, prim(TypeKind.INT_32_TYPE)),
                descriptor("z", 3, prim(TypeKind.INT_32_TYPE)),
                descriptor("w", 4, prim(TypeKind.INT_32_TYPE)));
        assertNotEquals(a, named);

        DynamicType retyped = point3();
        retyped.addMember(descriptor("w", 4, prim(TypeKind.INT_64_TYPE)));
        assertNotEquals(a, retyped);

        b.setAnnotation(annotation("Nested", null));
        assertNotEquals(a, b);
        a.setAnnotation(annotation("Nested", null));
        assertEquals(a, b);
        // An annotation on a member counts.
        b.getMember("x").setAnnotation(annotation("Key", null));
        assertNotEquals(a, b);
    }

    @Test
    void membersAreEqualWhenTheirTypeDescriptorAndAnnotationsAre() {
        DynamicType a = point3();
        DynamicType b = point3();
        assertEquals(a.getMember("x"), b.getMember("x"));
        assertEquals(a.getMember("x").hashCode(), b.getMember("x").hashCode());
        assertNotEquals(a.getMember("x"), a.getMember("y"));
        assertNotEquals(a.getMember("x"), null);

        a.getMember("x").setAnnotation(annotation("Key", null));
        assertNotEquals(a.getMember("x"), b.getMember("x"));
        b.getMember("x").setAnnotation(annotation("Key", null));
        assertEquals(a.getMember("x"), b.getMember("x"));

        // §7.5.2.6.3: members are equal only if they belong to the same type.
        DynamicType other = struct("TypesOther");
        other.addMember(descriptor("x", 1, prim(TypeKind.INT_32_TYPE)))
                .setAnnotation(annotation("Key", null));
        assertNotEquals(a.getMember("x"), other.getMember("x"));
    }

    @Test
    void cloneIsAnEqualTypeThatSharesNoMemberWithTheOriginal() {
        DynamicType original = point3();
        original.setAnnotation(annotation("Nested", null));
        original.getMember("y").setAnnotation(annotation("Key", null));

        DynamicType copy = original.clone();
        assertNotSame(original, copy);
        assertEquals(original, copy);
        assertEquals(original.hashCode(), copy.hashCode());
        assertNotSame(original.getMember("y"), copy.getMember("y"));
        assertEquals(original.getMember("y"), copy.getMember("y"));
        assertEquals("Key", copy.getMember("y").getAnnotations().get(0).getType().getName());
        assertEquals("Nested", copy.getAnnotation(0).getType().getName());

        // Changes to the copy are not changes to the original, and the other way round.
        copy.addMember(descriptor("w", 4, prim(TypeKind.INT_32_TYPE)));
        assertNull(original.getMember(4));
        assertNotEquals(original, copy);
        copy.getMember("y").setAnnotation(annotation("Optional", null));
        assertEquals("Key", original.getMember("y").getAnnotations().get(0).getType().getName());
        original.setAnnotation(annotation("Verbatim", null));
        assertEquals("Nested", copy.getAnnotation(0).getType().getName());
    }

    // --- §7.8.1.5 The descriptors are immutable -----------------------------

    @Test
    void aTypeDescriptorWithLeavesTheOriginalAsItWas() {
        TypeDescriptor empty = TYPES.newTypeDescriptor();
        assertEquals(TypeKind.STRUCTURE_TYPE, empty.getKind());
        assertNull(empty.getName());
        assertTrue(empty.getBound().isEmpty());

        TypeDescriptor named = empty.withName("A").withKind(TypeKind.ARRAY_TYPE).withBound(2, 3);
        assertNull(empty.getName());
        assertEquals(TypeKind.STRUCTURE_TYPE, empty.getKind());
        assertTrue(empty.getBound().isEmpty());
        assertEquals("A", named.getName());
        assertEquals(TypeKind.ARRAY_TYPE, named.getKind());
        assertEquals(Arrays.asList(2, 3), named.getBound());

        TypeDescriptor rebound = named.withBound(7);
        assertEquals(Arrays.asList(2, 3), named.getBound());
        assertEquals(Arrays.asList(7), rebound.getBound());

        DynamicType element = prim(TypeKind.INT_32_TYPE);
        TypeDescriptor withElement = named.withElementType(element).withKeyElementType(element)
                .withBaseType(element).withDiscriminatorType(element);
        assertNull(named.getElementType());
        assertNull(named.getKeyElementType());
        assertNull(named.getBaseType());
        assertNull(named.getDiscriminatorType());
        assertSame(element, withElement.getElementType());
        assertSame(element, withElement.getKeyElementType());
        assertSame(element, withElement.getBaseType());
        assertSame(element, withElement.getDiscriminatorType());

        assertThrows(UnsupportedOperationException.class, () -> named.getBound().add(9));
        assertThrows(UnsupportedOperationException.class, () -> named.getBound().clear());
        assertEquals(Arrays.asList(2, 3), named.getBound());
    }

    @Test
    void typeDescriptorsAreEqualWhenAllTheirPropertiesAre() {
        TypeDescriptor a = TYPES.newTypeDescriptor().withName("A").withBound(1, 2);
        TypeDescriptor b = TYPES.newTypeDescriptor().withName("A").withBound(1, 2);
        assertNotSame(a, b);
        assertEquals(a, b);
        assertEquals(a.hashCode(), b.hashCode());
        assertNotEquals(a, b.withName("B"));
        assertNotEquals(a, b.withBound(1));
        assertNotEquals(a, b.withKind(TypeKind.UNION_TYPE));
        assertNotEquals(a, b.withElementType(prim(TypeKind.INT_32_TYPE)));
        assertNotEquals(a, null);
    }

    @Test
    void aMemberDescriptorWithLeavesTheOriginalAsItWas() {
        MemberDescriptor empty = TYPES.newMemberDescriptor();
        assertNull(empty.getName());
        assertTrue(empty.getLabel().isEmpty());
        assertTrue(empty.isDefaultLabel());

        MemberDescriptor full = empty.withName("m").withId(4).withIndex(2).withDefaultValue("5")
                .withType(prim(TypeKind.INT_32_TYPE)).withLabel(1, 2);
        assertNull(empty.getName());
        assertEquals(0, empty.getId());
        assertNull(empty.getType());
        assertNull(empty.getDefaultValue());
        assertTrue(empty.getLabel().isEmpty());
        assertTrue(empty.isDefaultLabel());

        assertEquals("m", full.getName());
        assertEquals(4, full.getId());
        assertEquals(2, full.getIndex());
        assertEquals("5", full.getDefaultValue());
        assertEquals(TypeKind.INT_32_TYPE, full.getType().getKind());
        assertEquals(Arrays.asList(1, 2), full.getLabel());
        assertFalse(full.isDefaultLabel());
        assertTrue(full.withDefaultLabel(true).isDefaultLabel());
        assertFalse(full.isDefaultLabel());

        assertThrows(UnsupportedOperationException.class, () -> full.getLabel().add(3));
        assertThrows(UnsupportedOperationException.class, () -> full.getLabel().clear());
        assertEquals(Arrays.asList(1, 2), full.getLabel());

        MemberDescriptor same = TYPES.newMemberDescriptor().withName("m").withId(4).withIndex(2)
                .withDefaultValue("5").withType(prim(TypeKind.INT_32_TYPE)).withLabel(1, 2);
        assertEquals(full, same);
        assertEquals(full.hashCode(), same.hashCode());
        assertNotEquals(full, same.withId(5));
        assertNotEquals(full, same.withLabel(1));
    }

    @Test
    void anAnnotationDescriptorWithLeavesTheOriginalAsItWas() {
        AnnotationDescriptor empty = TYPES.newAnnotationDescriptor();
        assertNull(empty.getType());
        assertTrue(empty.getAllValue().isEmpty());

        AnnotationDescriptor typed = annotation("ID", "3");
        AnnotationDescriptor more = typed.withValue("other", "x");
        assertNull(typed.getValue("other"));
        assertEquals("x", more.getValue("other"));
        assertEquals("3", more.getValue("value"));
        assertEquals(1, typed.getAllValue().size());
        assertEquals(2, more.getAllValue().size());
        // The map handed out is a copy.
        typed.getAllValue().put("changed", "y");
        assertNull(typed.getValue("changed"));

        assertEquals(annotation("ID", "3"), typed);
        assertEquals(annotation("ID", "3").hashCode(), typed.hashCode());
        assertNotEquals(annotation("ID", "4"), typed);
        assertNotEquals(annotation("Key", "3"), typed);
    }

    // --- §7.8.1.3 createType(Class<?>) --------------------------------------

    /** A class that is not final: EXTENSIBLE (APPENDABLE), with the member a (id 1). */
    public static class Base {
        public int a;
    }

    /** Extends Base: the base type of its structure, with the members b (id 2) and c (id 3). */
    public static final class Derived extends Base {
        public String b;
        public double c;
    }

    private static final DynamicTypeFactoryImpl FACTORY = DynamicTypeFactoryImpl.getInstance(ENV);

    @Test
    void createTypeIsReachedThroughTheImplementationOfTheFactory() throws Exception {
        // The OMG class is unchanged and does not declare createType(Class<?>), which
        // DDS-Java-PSM 1.0 §7.8.1.3 adds; this implementation has it on its factory class.
        assertThrows(NoSuchMethodException.class,
                () -> DynamicTypeFactory.class.getMethod("createType", Class.class));
        assertEquals(DynamicTypeFactoryImpl.class,
                DynamicTypeFactoryImpl.class.getMethod("createType", Class.class).getDeclaringClass());
        // getInstance of the implementation is the factory of the environment, not another one.
        assertSame(DynamicTypeFactory.getInstance(ENV), FACTORY);
        assertSame(ENV, FACTORY.getEnvironment());
        assertThrows(IllegalArgumentException.class, () -> FACTORY.createType((Class<?>) null));
        assertEquals("org::omg::dds::TypesDynamicTypeTest::Base", FACTORY.createType(Base.class).getName());
    }

    @Test
    void aDerivedClassIsAStructureWithTheSuperclassAsBaseType() {
        DynamicType derived = FACTORY.createType(Derived.class);
        assertEquals("org::omg::dds::TypesDynamicTypeTest::Derived", derived.getName());
        assertEquals(TypeKind.STRUCTURE_TYPE, derived.getKind());

        DynamicType base = derived.getDescriptor().getBaseType();
        assertEquals("org::omg::dds::TypesDynamicTypeTest::Base", base.getName());
        assertEquals(TypeKind.STRUCTURE_TYPE, base.getKind());
        assertNull(base.getDescriptor().getBaseType(), "Base extends Object: no base type");

        // The type lists only the members its class declares; the base type has the others.
        assertEquals(Arrays.asList(2, 3), Arrays.asList(derived.getAllMembers().keySet().toArray()));
        assertEquals("b", derived.getMember(2).getName());
        assertEquals(TypeKind.STRING_TYPE, derived.getMember(2).getDescriptor().getType().getKind());
        assertEquals("c", derived.getMember(3).getName());
        assertEquals(TypeKind.FLOAT_64_TYPE, derived.getMember(3).getDescriptor().getType().getKind());
        assertNull(derived.getMember("a"));
        assertEquals(Arrays.asList(1), Arrays.asList(base.getAllMembers().keySet().toArray()));
        assertEquals("a", base.getMember(1).getName());
        assertEquals(TypeKind.INT_32_TYPE, base.getMember(1).getDescriptor().getType().getKind());
    }

    @Test
    void theBytesOfADerivedDynamicTypeAreThoseOfTheClass() {
        DynamicType derived = FACTORY.createType(Derived.class);
        DynamicData data = DynamicDataFactory.getInstance(ENV).newDynamicData(derived)
                .setInt32Value(1, 7)
                .setStringValue(2, "x")
                .setFloat64Value(3, 1.5);
        Derived bean = new Derived();
        bean.a = 7;
        bean.b = "x";
        bean.c = 1.5;
        // The members of the base type come first, as the class's fields do (§8.6.1.1).
        assertArrayEquals(ReflectionTypeSupport.of(Derived.class).encode(bean),
                new DynamicDataTypeSupport(ENV, derived).encode(data));
        DynamicData back = new DynamicDataTypeSupport(ENV, derived)
                .decode(ReflectionTypeSupport.of(Derived.class).encode(bean));
        assertEquals(7, back.getInt32Value(1));
        assertEquals("x", back.getStringValue(2));
        assertEquals(1.5, back.getFloat64Value(3));
    }
}
