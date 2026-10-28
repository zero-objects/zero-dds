// SPDX-License-Identifier: Apache-2.0
package org.zerodds.cdr;

import org.junit.jupiter.api.Test;
import org.omg.dds.type.Extensibility;
import org.omg.dds.type.ID;
import org.omg.dds.type.Key;

import java.math.BigDecimal;
import java.math.BigInteger;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Set;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * The Java Type Representation of DDS-Java-PSM 1.0 §8 as
 * {@link ReflectionTypeSupport} implements it: the extensibility default of
 * §8.6.1.2, the inheritance of §8.6.1.1, the kinds of Table 8.1 and §8.4, the
 * sequences of §8.5.3 and the annotations of §8.3/§8.9.
 */
class JavaTypeRepresentationTest {

    // --- §8.6.1.2 Extensibility -------------------------------------------

    /** A final class directly under Object: FINAL. */
    public static final class FinalType {
        public int a;
    }

    /** A non-final class: EXTENSIBLE of §8.6.1.2, which XTypes 1.3 calls APPENDABLE. */
    public static class OpenType {
        public int a;
    }

    /** A final class with a base class: EXTENSIBLE (APPENDABLE), not FINAL. */
    public static final class DerivedType extends OpenType {
        public int b;
    }

    @Test
    void extensibilityFollowsTheClassShape() {
        assertEquals(ExtensibilityKind.FINAL,
                ReflectionTypeSupport.of(FinalType.class).getExtensibility());
        assertEquals(ExtensibilityKind.APPENDABLE,
                ReflectionTypeSupport.of(OpenType.class).getExtensibility());
        assertEquals(ExtensibilityKind.APPENDABLE,
                ReflectionTypeSupport.of(DerivedType.class).getExtensibility());
    }

    /** §8.6.1.2: the class may still say so itself. */
    @Extensibility(Extensibility.Kind.MUTABLE_EXTENSIBILITY)
    public static final class MutableType {
        public int a;
    }

    @Test
    void anAnnotationOverridesTheDefault() {
        assertEquals(ExtensibilityKind.MUTABLE,
                ReflectionTypeSupport.of(MutableType.class).getExtensibility());
    }

    // --- §8.6.1.1 Inheritance ---------------------------------------------

    @Test
    void theMembersOfTheBaseClassComeFirst() {
        ReflectionTypeSupport<DerivedType> ts =
                ReflectionTypeSupport.of(DerivedType.class);
        DerivedType value = new DerivedType();
        value.a = 7;
        value.b = 9;
        DerivedType back = ts.decode(ts.encode(value));
        assertEquals(7, back.a);
        assertEquals(9, back.b);
        // Two INT32 members, the DHEADER of an EXTENSIBLE (APPENDABLE) type before them.
        assertEquals(12, ts.encode(value).length);
    }

    // --- Table 8.1, §8.4 and §8.5.3 ---------------------------------------

    public enum Colour {
        RED, GREEN, BLUE
    }

    public static final class Kinds {
        public BigInteger counter = BigInteger.ZERO;
        public BigDecimal measure = BigDecimal.ZERO;
        public Colour colour = Colour.RED;
        public Set<String> tags = new LinkedHashSet<>();
        public List<Integer> numbers = List.of();
    }

    @Test
    void theKindsOfTable81RoundTrip() {
        Kinds value = new Kinds();
        // A value above 2^63 is what only BigInteger can carry (Table 8.2).
        value.counter = new BigInteger("18446744073709551615");
        value.measure = new BigDecimal("1.25");
        value.colour = Colour.BLUE;
        value.tags = new LinkedHashSet<>(List.of("a", "b"));
        value.numbers = List.of(1, 2, 3);

        ReflectionTypeSupport<Kinds> ts = ReflectionTypeSupport.of(Kinds.class);
        Kinds back = ts.decode(ts.encode(value));
        assertEquals(value.counter, back.counter);
        // Float128 holds the value with more digits than 1.25 states; BigDecimal.equals
        // compares the scale too, so the values are compared as numbers.
        assertEquals(0, value.measure.compareTo(back.measure));
        assertEquals(Colour.BLUE, back.colour);
        assertEquals(value.tags, back.tags);
        assertEquals(value.numbers, back.numbers);
    }

    @Test
    void aSequenceIsWrittenForEveryCollection() {
        Kinds value = new Kinds();
        value.tags = new LinkedHashSet<>(List.of("only"));
        ReflectionTypeSupport<Kinds> ts = ReflectionTypeSupport.of(Kinds.class);
        Kinds back = ts.decode(ts.encode(value));
        assertEquals(1, back.tags.size());
        assertTrue(back.tags.contains("only"));
    }

    // --- §8.3 and §8.9 Annotations ----------------------------------------

    /** A `@Key` of another framework is not metadata of the type system. */
    @interface ForeignKey {
    }

    @interface ForeignId {
        int value();
    }

    public static final class ForeignAnnotations {
        @ForeignKey
        public int secret = 5;
        @ForeignId(42)
        public int other = 6;
    }

    @Test
    void onlyTheTypeSystemAnnotationsCount() {
        ReflectionTypeSupport<ForeignAnnotations> ts =
                ReflectionTypeSupport.of(ForeignAnnotations.class);
        assertFalse(ts.isKeyed(), "a @Key of another package is not a key");
        ForeignAnnotations back = ts.decode(ts.encode(new ForeignAnnotations()));
        assertEquals(5, back.secret);
        assertEquals(6, back.other);
    }

    public static final class KeyedType {
        @Key
        @ID(7)
        public int id = 1;
        public int value = 2;
    }

    @Test
    void theTypeSystemAnnotationsAreRead() {
        ReflectionTypeSupport<KeyedType> ts =
                ReflectionTypeSupport.of(KeyedType.class);
        assertTrue(ts.isKeyed());
        // The key member is the only key; the key holder (XTypes 1.3 §7.6.8, big-endian)
        // is the value of the key member, 1, and not its member id 7.
        byte[] hash = ts.keyHash(new KeyedType());
        assertArrayEquals(new byte[] {0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0}, hash);
    }

    // --- §8.6.1.1 the restrictions of java.io.Serializable ---------------------

    /** A base class that is not Serializable: its fields are not serialized (initialized by its constructor). */
    public static class PlainBase {
        public int hidden = 99;
    }

    public static class SerializableDerived extends PlainBase implements java.io.Serializable {
        private static final long serialVersionUID = 1L;
        public int shown;
    }

    @Test
    void theFieldsOfANonSerializableBaseOfASerializableClassAreNotPartOfTheType() {
        ReflectionTypeSupport<SerializableDerived> ts =
                ReflectionTypeSupport.of(SerializableDerived.class);
        SerializableDerived value = new SerializableDerived();
        value.shown = 5;
        value.hidden = 7;
        byte[] bytes = ts.encode(value);
        // DHEADER (EXTENSIBLE) and the one int32 of the class itself
        assertEquals(8, bytes.length);
        SerializableDerived back = ts.decode(bytes);
        assertEquals(5, back.shown);
        assertEquals(99, back.hidden, "the base is initialized by its own constructor");
    }

    @Test
    void theDynamicTypeOfThatClassHasNoBaseType() {
        org.omg.dds.type.dynamic.DynamicType type = DynamicTypeFactoryImpl
                .getInstance(org.omg.dds.Qos.env()).createType(SerializableDerived.class);
        org.junit.jupiter.api.Assertions.assertNull(type.getDescriptor().getBaseType());
        assertEquals(1, type.getAllMembers().size());
    }
}
