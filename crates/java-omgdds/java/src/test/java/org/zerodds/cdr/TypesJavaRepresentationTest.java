// SPDX-License-Identifier: Apache-2.0
package org.zerodds.cdr;

import com.acme.project.TypesTheClass;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.omg.dds.Qos;
import org.omg.dds.domain.DomainParticipant;
import org.omg.dds.topic.Topic;
import org.omg.dds.type.Extensibility;
import org.omg.dds.type.ID;
import org.omg.dds.type.Key;
import org.omg.dds.type.MustUnderstand;
import org.omg.dds.type.Optional;
import org.omg.dds.type.SerializeAs;
import org.omg.dds.type.TypeKind;
import org.omg.dds.type.dynamic.DynamicType;
import org.zerodds.internal.InProcessBus;

import java.io.Serializable;
import java.math.BigInteger;
import java.util.ArrayDeque;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collection;
import java.util.Deque;
import java.util.EnumMap;
import java.util.EnumSet;
import java.util.LinkedHashMap;
import java.util.LinkedHashSet;
import java.util.LinkedList;
import java.util.List;
import java.util.Map;
import java.util.NavigableMap;
import java.util.NavigableSet;
import java.util.Queue;
import java.util.Set;
import java.util.SortedMap;
import java.util.SortedSet;
import java.util.TreeMap;
import java.util.TreeSet;
import java.util.UUID;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.ConcurrentMap;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * The Java Type Representation of DDS-Java-PSM 1.0 Clause 8 as {@link ReflectionTypeSupport}
 * implements it: the collections and maps a field declares (§8.5.2, §8.5.3), UInt64 held in a
 * {@code BigInteger} (§8.4), wide strings with code points above U+FFFF (§8.5.1), static and
 * transient fields (§8.6), the extensibility kinds (§8.6.1.2), the annotations of the type
 * system and those of another package with the same names (§8.3, §8.9), the package of a class
 * as the modules of its type name (§8.8) and the Serializable types of the JDK that Table 8.1
 * does not list (§8.1). Expected octets are derived by hand from the clauses and from XTypes
 * 1.3 (XCDR2, little-endian).
 *
 * <p>Domain ids 1620 to 1629.
 */
class TypesJavaRepresentationTest {

    @BeforeEach
    void setUp() {
        InProcessBus.instance().reset();
    }

    @AfterEach
    void tearDown() {
        InProcessBus.instance().reset();
        System.clearProperty(NarrowStrings.PROPERTY);
    }

    private static byte[] bytes(int... values) {
        byte[] out = new byte[values.length];
        for (int i = 0; i < out.length; i++) {
            out[i] = (byte) values[i];
        }
        return out;
    }

    private static <T> byte[] encode(T value) {
        @SuppressWarnings("unchecked")
        Class<T> type = (Class<T>) value.getClass();
        return ReflectionTypeSupport.of(type).encode(value);
    }

    private static <T> T roundTrip(T value) {
        @SuppressWarnings("unchecked")
        Class<T> type = (Class<T>) value.getClass();
        ReflectionTypeSupport<T> ts = ReflectionTypeSupport.of(type);
        return ts.decode(ts.encode(value));
    }

    public enum Colour {
        RED, GREEN, BLUE
    }

    // --- §8.5.3 and Table 8.1: the collection a field declares ------------------

    public static final class Sequences {
        public TreeSet<String> tree = new TreeSet<>();
        public SortedSet<Integer> sorted = new TreeSet<>();
        public NavigableSet<Integer> navigable = new TreeSet<>();
        public EnumSet<Colour> enumSet = EnumSet.noneOf(Colour.class);
        public Queue<String> queue = new ArrayDeque<>();
        public Deque<String> deque = new ArrayDeque<>();
        public LinkedList<Integer> linked = new LinkedList<>();
        public Collection<Integer> collection = new ArrayList<>();
        public Set<Integer> set = new LinkedHashSet<>();
        public List<Integer> list = new ArrayList<>();
    }

    @Test
    void aSequenceIsReadIntoTheCollectionTheFieldDeclares() {
        Sequences value = new Sequences();
        value.tree.addAll(Arrays.asList("b", "a"));
        value.sorted.addAll(Arrays.asList(3, 1, 2));
        value.navigable.addAll(Arrays.asList(5, 4));
        value.enumSet.add(Colour.BLUE);
        value.enumSet.add(Colour.RED);
        value.queue.offer("x");
        value.queue.offer("y");
        value.deque.addFirst("p");
        value.deque.addLast("q");
        value.linked.addAll(Arrays.asList(7, 8));
        value.collection.add(1);
        value.set.addAll(Arrays.asList(9, 8));
        value.list.addAll(Arrays.asList(2, 1));

        Sequences back = roundTrip(value);

        assertEquals(TreeSet.class, back.tree.getClass());
        assertEquals(Arrays.asList("a", "b"), new ArrayList<>(back.tree));
        assertEquals(TreeSet.class, back.sorted.getClass());
        assertEquals(Arrays.asList(1, 2, 3), new ArrayList<>(back.sorted));
        assertEquals(TreeSet.class, back.navigable.getClass());
        assertEquals(Arrays.asList(4, 5), new ArrayList<>(back.navigable));
        assertTrue(back.enumSet instanceof EnumSet);
        assertEquals(Arrays.asList(Colour.RED, Colour.BLUE), new ArrayList<>(back.enumSet));
        assertEquals(ArrayDeque.class, back.queue.getClass());
        assertEquals(Arrays.asList("x", "y"), new ArrayList<>(back.queue));
        assertEquals(ArrayDeque.class, back.deque.getClass());
        assertEquals(Arrays.asList("p", "q"), new ArrayList<>(back.deque));
        assertEquals(LinkedList.class, back.linked.getClass());
        assertEquals(Arrays.asList(7, 8), back.linked);
        assertEquals(ArrayList.class, back.collection.getClass());
        assertEquals(Arrays.asList(1), new ArrayList<>(back.collection));
        assertEquals(LinkedHashSet.class, back.set.getClass());
        assertEquals(Arrays.asList(9, 8), new ArrayList<>(back.set));
        assertEquals(ArrayList.class, back.list.getClass());
        assertEquals(Arrays.asList(2, 1), back.list);
    }

    @Test
    void emptyCollectionsAreReadIntoTheirClassesToo() {
        Sequences back = roundTrip(new Sequences());
        assertTrue(back.tree.isEmpty() && back.sorted.isEmpty() && back.queue.isEmpty()
                && back.enumSet.isEmpty() && back.linked.isEmpty());
        assertEquals(TreeSet.class, back.sorted.getClass());
        assertTrue(back.enumSet instanceof EnumSet);
    }

    /** A field of a collection type that no standard class implements cannot receive a sequence. */
    public static final class UnfillableCollection {
        public java.util.AbstractSequentialList<Integer> value = new LinkedList<>();
    }

    @Test
    void aCollectionTypeNoStandardClassFillsIsRefusedWhenRead() {
        // AbstractSequentialList is an abstract class: a LinkedList is one, but the field's class is
        // not among the standard classes that are chosen for an interface.
        UnfillableCollection value = new UnfillableCollection();
        value.value = new LinkedList<>(Arrays.asList(1));
        ReflectionTypeSupport<UnfillableCollection> ts = ReflectionTypeSupport.of(UnfillableCollection.class);
        byte[] wire = ts.encode(value);
        XcdrException e = assertThrows(XcdrException.class, () -> ts.decode(wire));
        assertTrue(e.getMessage().contains("AbstractSequentialList"), e.getMessage());
    }

    // --- §8.5.3: the order of a collection that is not a list is that of its iterator ----

    public static final class OrderedSets {
        public Set<String> insertion = new LinkedHashSet<>(Arrays.asList("b", "a"));
        public Set<String> sorted = new TreeSet<>(Arrays.asList("b", "a"));
    }

    @Test
    void aSetIsWrittenInTheOrderOfItsIterator() {
        // Each set is a sequence of strings: the elements are not primitive, so a DHEADER
        // precedes the count. The first set iterates b, a; the second, a TreeSet, a, b.
        //   offset 0   DHEADER 18       offset 4  count 2
        //   offset 8   "b": 2 'b' NUL (8..13), padding (14, 15)
        //   offset 16  "a": 2 'a' NUL (16..21); the end is 22, the DHEADER counts 18
        //   offset 22  padding to 24 for the next DHEADER (18); count 2 at 28
        //   offset 32  "a" (32..37), padding (38, 39), "b" (40..45)
        assertArrayEquals(bytes(
                18, 0, 0, 0, 2, 0, 0, 0,
                2, 0, 0, 0, 'b', 0, 0, 0,
                2, 0, 0, 0, 'a', 0, 0, 0,
                18, 0, 0, 0, 2, 0, 0, 0,
                2, 0, 0, 0, 'a', 0, 0, 0,
                2, 0, 0, 0, 'b', 0), encode(new OrderedSets()));
        OrderedSets back = roundTrip(new OrderedSets());
        assertEquals(Arrays.asList("b", "a"), new ArrayList<>(back.insertion));
        assertEquals(Arrays.asList("a", "b"), new ArrayList<>(back.sorted));
    }

    // --- §8.5.2: the map a field declares ----------------------------------------

    public static final class Maps {
        public TreeMap<String, Integer> tree = new TreeMap<>();
        public SortedMap<String, Integer> sorted = new TreeMap<>();
        public NavigableMap<Integer, String> navigable = new TreeMap<>();
        public Map<String, Integer> plain = new LinkedHashMap<>();
        public EnumMap<Colour, Integer> enumMap = new EnumMap<>(Colour.class);
        public ConcurrentMap<String, Integer> concurrent = new ConcurrentHashMap<>();
        @SerializeAs(TypeKind.MAP_TYPE)
        public TreeMap<String, Integer> marked = new TreeMap<>();
    }

    @Test
    void aMapIsReadIntoTheMapClassTheFieldDeclares() {
        Maps value = new Maps();
        value.tree.put("b", 2);
        value.tree.put("a", 1);
        value.sorted.put("z", 26);
        value.navigable.put(2, "two");
        value.navigable.put(1, "one");
        value.plain.put("k", 5);
        value.plain.put("j", 4);
        value.enumMap.put(Colour.BLUE, 3);
        value.enumMap.put(Colour.RED, 1);
        value.concurrent.put("c", 9);
        value.marked.put("m", 7);

        Maps back = roundTrip(value);

        assertEquals(TreeMap.class, back.tree.getClass());
        assertEquals(value.tree, back.tree);
        assertEquals(Arrays.asList("a", "b"), new ArrayList<>(back.tree.keySet()));
        assertEquals(TreeMap.class, back.sorted.getClass());
        assertEquals(value.sorted, back.sorted);
        assertEquals(TreeMap.class, back.navigable.getClass());
        assertEquals(value.navigable, back.navigable);
        assertEquals(LinkedHashMap.class, back.plain.getClass());
        assertEquals(Arrays.asList("k", "j"), new ArrayList<>(back.plain.keySet()));
        assertEquals(EnumMap.class, back.enumMap.getClass());
        assertEquals(value.enumMap, back.enumMap);
        assertEquals(ConcurrentHashMap.class, back.concurrent.getClass());
        assertEquals(value.concurrent, back.concurrent);
        // @SerializeAs(MAP_TYPE) says what the default says, and the class is the declared one.
        assertEquals(TreeMap.class, back.marked.getClass());
        assertEquals(value.marked, back.marked);
    }

    @Test
    void aMapMarkedAsAMapKeepsItsBound() {
        BoundedMaps value = new BoundedMaps();
        value.entries.put("a", 1);
        value.entries.put("b", 2);
        assertEquals(value.entries, roundTrip(value).entries);
        value.entries.put("c", 3);
        assertThrows(XcdrException.class, () -> encode(value));
    }

    public static final class BoundedMaps {
        @SerializeAs(value = TypeKind.MAP_TYPE, bound = {2})
        public TreeMap<String, Integer> entries = new TreeMap<>();
    }

    // --- §8.4: UInt64 held in a BigInteger ---------------------------------------

    public static final class Big {
        public BigInteger value = BigInteger.ZERO;
    }

    private static Big big(BigInteger v) {
        Big b = new Big();
        b.value = v;
        return b;
    }

    @Test
    void aBigIntegerWithoutAnnotationIsUInt64AndIsRangeChecked() {
        BigInteger max = BigInteger.ONE.shiftLeft(64).subtract(BigInteger.ONE);
        assertArrayEquals(bytes(255, 255, 255, 255, 255, 255, 255, 255), encode(big(max)));
        assertEquals(max, roundTrip(big(max)).value);
        assertArrayEquals(bytes(0, 0, 0, 0, 0, 0, 0, 0), encode(big(BigInteger.ZERO)));
        // 2^63 is the sign bit of the Int64 pattern: still a UInt64.
        BigInteger half = BigInteger.ONE.shiftLeft(63);
        assertArrayEquals(bytes(0, 0, 0, 0, 0, 0, 0, 128), encode(big(half)));
        assertEquals(half, roundTrip(big(half)).value);

        // A value outside 0 to 2^64-1 is no UInt64: refused, and not cut to the low 64 bits.
        assertThrows(XcdrException.class, () -> encode(big(BigInteger.valueOf(-1))));
        assertThrows(XcdrException.class, () -> encode(big(BigInteger.ONE.shiftLeft(64))));
        assertThrows(XcdrException.class, () -> encode(big(BigInteger.ONE.shiftLeft(100))));
    }

    // --- §8.5.1: a wide string is a string of code points -------------------------

    public static final class WideText {
        @SerializeAs(value = TypeKind.STRING_TYPE, collectionElementKind = TypeKind.CHAR_32_TYPE)
        public String value;
    }

    @Test
    void aCodePointAboveU10000OfAWideStringIsOneCharacter() {
        // DDS-Java-PSM 1.0 8.5.1: "each Java code point shall become a single DDS wide
        // character". U+1D11E is one Java code point and two Java chars (a surrogate pair); on the
        // wire it is one 32-bit character. The length counts octets: 8 for 'a' and the code point.
        WideText t = new WideText();
        t.value = "a𝄞";
        assertEquals(2, t.value.codePointCount(0, t.value.length()));
        assertEquals(3, t.value.length());
        assertArrayEquals(bytes(8, 0, 0, 0, 0x61, 0, 0, 0, 0x1E, 0xD1, 0x01, 0), encode(t));

        WideText back = roundTrip(t);
        assertEquals(t.value, back.value);
        assertEquals(2, back.value.codePointCount(0, back.value.length()));
        assertEquals(0x1D11E, back.value.codePointAt(1));
    }

    @Test
    void theUtf16SettingWritesTheWstringOfXTypes13() {
        // XTypes 1.3 7.4.1.1.2: a wstring is a sequence of UTF-16 code units: the code point is
        // the pair D834 DD1E, and the length counts octets, 6 for 'a' and the pair.
        System.setProperty(WideStrings.PROPERTY, "utf16");
        try {
            WideText t = new WideText();
            t.value = "a𝄞";
            assertArrayEquals(bytes(6, 0, 0, 0, 0x61, 0x00, 0x34, 0xD8, 0x1E, 0xDD), encode(t));
            assertEquals(t.value, roundTrip(t).value);
        } finally {
            System.clearProperty(WideStrings.PROPERTY);
        }
    }

    @Test
    void aWideStringRefusesWhatIsNoCodePoint() {
        // 4 octets, the value 0x110000
        byte[] bad = bytes(4, 0, 0, 0, 0x00, 0x00, 0x11, 0x00);
        assertThrows(XcdrException.class, () -> ReflectionTypeSupport.of(WideText.class).decode(bad));
    }

    // --- §8.5.1: the forms of a narrow string ------------------------------------

    public static final class Text {
        public String value;
    }

    @Test
    void bothFormsOfANarrowStringAreWrittenByTheSameTypeSupport() {
        ReflectionTypeSupport<Text> ts = ReflectionTypeSupport.of(Text.class);
        Text t = new Text();
        t.value = "é";
        // The default is the form of DDS-Java-PSM 1.0 §8.5.1: each Java character truncated to its low octet.
        assertArrayEquals(bytes(2, 0, 0, 0, 0xE9, 0), ts.encode(t));
        assertEquals("é", ts.decode(bytes(2, 0, 0, 0, 0xE9, 0)).value);
        // The setting utf8: XTypes 1.3 §7.4.1.1.2, 'é' is C3 A9.
        System.setProperty(NarrowStrings.PROPERTY, "utf8");
        assertArrayEquals(bytes(3, 0, 0, 0, 0xC3, 0xA9, 0), ts.encode(t));
        System.clearProperty(NarrowStrings.PROPERTY);
        assertArrayEquals(bytes(2, 0, 0, 0, 0xE9, 0), ts.encode(t));
    }

    // --- §8.6: static and transient fields are omitted ----------------------------

    public static final class Skipped {
        public static int counter = 99;
        public transient int cache = 7;
        public int kept = 5;
    }

    @Test
    void staticAndTransientFieldsCarryNoMember() {
        Skipped value = new Skipped();
        value.cache = 100;
        // The one member, kept, as an Int32.
        assertArrayEquals(bytes(5, 0, 0, 0), encode(value));
        Skipped back = ReflectionTypeSupport.of(Skipped.class).decode(bytes(9, 0, 0, 0));
        assertEquals(9, back.kept);
        assertEquals(7, back.cache, "a transient field keeps what the constructor gave it");
        assertEquals(99, Skipped.counter);

        DynamicType type = DynamicTypeFactoryImpl.getInstance(Qos.env()).createType(Skipped.class);
        assertEquals(1, type.getAllMembers().size());
        assertEquals("kept", type.getMember(1).getName());
        assertNull(type.getMember("counter"));
        assertNull(type.getMember("cache"));
    }

    // --- §8.6.1.2: the extensibility kinds ---------------------------------------

    /** Not final, so EXTENSIBLE by default; the annotation says FINAL. */
    @Extensibility(Extensibility.Kind.FINAL_EXTENSIBILITY)
    public static class ExplicitlyFinal {
        public int a = 1;
        public int b = 2;
    }

    /** Not final, no annotation: EXTENSIBLE, which XTypes 1.3 calls APPENDABLE. */
    public static class DefaultExtensible {
        public int a = 1;
        public int b = 2;
    }

    @Test
    void finalCanBeSaidOnAClassThatIsNotFinal() {
        ReflectionTypeSupport<ExplicitlyFinal> ts = ReflectionTypeSupport.of(ExplicitlyFinal.class);
        assertEquals(ExtensibilityKind.FINAL, ts.getExtensibility());
        // Two Int32 members and no DHEADER.
        assertArrayEquals(bytes(1, 0, 0, 0, 2, 0, 0, 0), ts.encode(new ExplicitlyFinal()));
        assertEquals(2, ts.decode(ts.encode(new ExplicitlyFinal())).b);
        DynamicType type = DynamicTypeFactoryImpl.getInstance(Qos.env()).createType(ExplicitlyFinal.class);
        assertEquals("FINAL", type.getAnnotation("Extensibility").getValue("value"));
    }

    @Test
    void aClassThatIsNotFinalIsWrittenWithADheader() {
        ReflectionTypeSupport<DefaultExtensible> ts = ReflectionTypeSupport.of(DefaultExtensible.class);
        assertEquals(ExtensibilityKind.APPENDABLE, ts.getExtensibility());
        // The DHEADER 8 (two Int32 members), then the members.
        assertArrayEquals(bytes(8, 0, 0, 0, 1, 0, 0, 0, 2, 0, 0, 0), ts.encode(new DefaultExtensible()));
        assertEquals(1, ts.decode(ts.encode(new DefaultExtensible())).a);
    }

    // --- §8.3: ID, Optional and MustUnderstand on a MUTABLE class ------------------

    @Extensibility(Extensibility.Kind.MUTABLE_EXTENSIBILITY)
    public static final class Tagged {
        @ID(5)
        public int a;
        @Optional
        @ID(9)
        public Integer b;
        @MustUnderstand
        @ID(12)
        public short c;
    }

    private static Tagged tagged(int a, Integer b, int c) {
        Tagged t = new Tagged();
        t.a = a;
        t.b = b;
        t.c = (short) c;
        return t;
    }

    @Test
    void theIdOfAMemberIsItsIdOnTheWireAndAnAbsentOptionalMemberHasNone() {
        ReflectionTypeSupport<Tagged> ts = ReflectionTypeSupport.of(Tagged.class);
        // XCDR2 MUTABLE: a DHEADER, and for each member an EMHEADER word (M_FLAG << 31)
        // + (LC << 28) + id with LC 4, so a NEXTINT with the length of the member follows.
        //   a  (id 5):  0x40000005, NEXTINT 4, 1
        //   b  absent:  nothing
        //   c  (id 12): M_FLAG set: 0xC000000C, NEXTINT 2, the short 2
        // The members take offsets 4 to 25, 22 octets: DHEADER 22.
        byte[] absent = bytes(22, 0, 0, 0,
                0x05, 0x00, 0x00, 0x40, 4, 0, 0, 0, 1, 0, 0, 0,
                0x0C, 0x00, 0x00, 0xC0, 2, 0, 0, 0, 2, 0);
        assertArrayEquals(absent, ts.encode(tagged(1, null, 2)));
        Tagged back = ts.decode(absent);
        assertEquals(1, back.a);
        assertNull(back.b);
        assertEquals(2, back.c);

        // With b present: 0x40000009, NEXTINT 4, 7; 12 octets more, DHEADER 34.
        byte[] present = bytes(34, 0, 0, 0,
                0x05, 0x00, 0x00, 0x40, 4, 0, 0, 0, 1, 0, 0, 0,
                0x09, 0x00, 0x00, 0x40, 4, 0, 0, 0, 7, 0, 0, 0,
                0x0C, 0x00, 0x00, 0xC0, 2, 0, 0, 0, 2, 0);
        assertArrayEquals(present, ts.encode(tagged(1, 7, 2)));
        back = ts.decode(present);
        assertEquals(Integer.valueOf(7), back.b);
        assertEquals(2, back.c);
    }

    @Test
    void theAnnotationsAreMembersOfTheDynamicTypeToo() {
        DynamicType type = DynamicTypeFactoryImpl.getInstance(Qos.env()).createType(Tagged.class);
        assertEquals(Arrays.asList(5, 9, 12), Arrays.asList(type.getAllMembers().keySet().toArray()));
        assertEquals("Optional", type.getMember(9).getAnnotations().get(0).getType().getName());
        assertEquals("MustUnderstand", type.getMember(12).getAnnotations().get(0).getType().getName());
        assertTrue(type.getMember(5).getAnnotations().isEmpty());
    }

    // --- §8.9: only the annotations of the type system count -----------------------

    @org.zerodds.cdr.foreign.Extensibility("MUTABLE")
    public static final class ForeignInFinal {
        @org.zerodds.cdr.foreign.Key
        public int id = 1;
        @org.zerodds.cdr.foreign.ID(77)
        public int other = 2;
    }

    @Test
    void annotationsOfAnotherPackageWithTheNamesOfTheTypeSystemAreIgnored() {
        ReflectionTypeSupport<ForeignInFinal> ts = ReflectionTypeSupport.of(ForeignInFinal.class);
        assertFalse(ts.isKeyed(), "a Key of another package is no key");
        assertEquals(ExtensibilityKind.FINAL, ts.getExtensibility(),
                "an Extensibility of another package does not make the class MUTABLE");
        assertArrayEquals(new byte[16], ts.keyHash(new ForeignInFinal()));
        assertArrayEquals(bytes(1, 0, 0, 0, 2, 0, 0, 0), ts.encode(new ForeignInFinal()));
    }

    @Extensibility(Extensibility.Kind.MUTABLE_EXTENSIBILITY)
    public static final class ForeignInMutable {
        @org.zerodds.cdr.foreign.ID(77)
        @org.zerodds.cdr.foreign.MustUnderstand
        public int a = 3;
    }

    @Test
    void anIdAndAMustUnderstandOfAnotherPackageDoNotReachTheEmheader() {
        // The member id is 1 (the first), and the M_FLAG is not set: 0x40000001.
        assertArrayEquals(bytes(12, 0, 0, 0, 0x01, 0x00, 0x00, 0x40, 4, 0, 0, 0, 3, 0, 0, 0),
                encode(new ForeignInMutable()));
    }

    public static final class ForeignOptional {
        @org.zerodds.cdr.foreign.Optional
        public Integer value;
    }

    @Test
    void anOptionalOfAnotherPackageDoesNotMakeAMemberOptional() {
        // value is null and the member is not @Optional of the type system: a value is needed.
        assertThrows(XcdrException.class, () -> encode(new ForeignOptional()));
    }

    // --- §8.8: the package of a class is the modules of its type name ---------------

    @Test
    void thePackageOfAClassIsTheModulesOfTheTypeName() {
        assertEquals("com::acme::project::TypesTheClass",
                ReflectionTypeSupport.of(TypesTheClass.class).getTypeName());
        DomainParticipant participant = Qos.factory().createParticipant(1620);
        Topic<TypesTheClass> topic = participant.createTopic("TypesPackageTopic", TypesTheClass.class);
        assertEquals("com::acme::project::TypesTheClass", topic.getTypeName());
        // A nested class continues the name with its enclosing class.
        assertEquals("org::zerodds::cdr::TypesJavaRepresentationTest::Skipped",
                ReflectionTypeSupport.of(Skipped.class).getTypeName());
    }

    // --- §8.1: Serializable types ------------------------------------------------

    /** A Serializable class: serialVersionUID is static and is no member. */
    public static final class SerializableSample implements Serializable {
        private static final long serialVersionUID = 1L;
        private int x = 4;
        private String s = "ab";
    }

    public static final class WithDate {
        public java.util.Date when = new java.util.Date(0);
    }

    public static final class WithUuid {
        public UUID id = new UUID(1, 2);
    }

    public static final class WithInstant {
        public java.time.Instant at = java.time.Instant.EPOCH;
    }

    public static final class WithListOfDates {
        public List<java.util.Date> dates = new ArrayList<>();
    }

    public static final class WithArrayOfUuids {
        public UUID[] ids = new UUID[0];
    }

    @Test
    void aSerializableClassOfTheApplicationIsATopicType() {
        // x as an Int32, s as a string of 2 characters (length 3 with the NUL); a final
        // class that extends Object is FINAL, so no DHEADER.
        assertArrayEquals(bytes(4, 0, 0, 0, 3, 0, 0, 0, 'a', 'b', 0), encode(new SerializableSample()));
        SerializableSample back = roundTrip(new SerializableSample());
        assertEquals(4, back.x);
        assertEquals("ab", back.s);
    }

    public static final class WithJdkValues {
        public java.util.Date when = new java.util.Date(1_000L);
        public UUID id = new UUID(1, 2);
        public java.time.Instant at = java.time.Instant.ofEpochSecond(5, 6);
        public java.time.LocalDate day = java.time.LocalDate.ofEpochDay(7);
        public java.time.ZonedDateTime zoned = java.time.ZonedDateTime.of(
                java.time.LocalDateTime.of(2026, 10, 17, 12, 0), java.time.ZoneId.of("Europe/Berlin"));
        public java.net.URI uri = java.net.URI.create("dds://x/y");
        public java.util.Locale locale = java.util.Locale.GERMANY;
        public java.util.BitSet bits = java.util.BitSet.valueOf(new long[] {5L});
    }

    @Test
    void aSerializableClassOfTheJdkIsPublishedInTheFormOfItsKind() {
        WithJdkValues back = roundTrip(new WithJdkValues());
        assertEquals(new java.util.Date(1_000L), back.when);
        assertEquals(new UUID(1, 2), back.id);
        assertEquals(java.time.Instant.ofEpochSecond(5, 6), back.at);
        assertEquals(java.time.LocalDate.ofEpochDay(7), back.day);
        assertEquals(new WithJdkValues().zoned, back.zoned);
        assertEquals(java.net.URI.create("dds://x/y"), back.uri);
        assertEquals(java.util.Locale.GERMANY, back.locale);
        assertEquals(java.util.BitSet.valueOf(new long[] {5L}), back.bits);
    }

    @Test
    void theFormsOfDateAndUuidAreTheirTable81Parts() {
        // a Date is its milliseconds, an int64; a UUID the two halves, two int64 (natural alignment)
        WithDate d = new WithDate();
        d.when = new java.util.Date(0x0102030405060708L);
        assertArrayEquals(bytes(8, 7, 6, 5, 4, 3, 2, 1), encode(d));
        assertArrayEquals(bytes(1, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0), encode(new WithUuid()));
    }

    @Test
    void listsAndArraysOfJdkValuesAreSequences() {
        WithListOfDates l = new WithListOfDates();
        l.dates.add(new java.util.Date(1L));
        l.dates.add(new java.util.Date(2L));
        assertEquals(l.dates, roundTrip(l).dates);
        WithArrayOfUuids a = new WithArrayOfUuids();
        a.ids = new UUID[] {new UUID(1, 2), new UUID(3, 4)};
        assertEquals(java.util.Arrays.asList(a.ids), java.util.Arrays.asList(roundTrip(a).ids));
    }

    @Test
    void aJdkClassIsATopicTypeItself() {
        TopicTypeSupport<java.util.Date> ts = TypeSupportResolver.resolve(java.util.Date.class);
        assertEquals(new java.util.Date(99L), ts.decode(ts.encode(new java.util.Date(99L))));
        TopicTypeSupport<java.util.BitSet> bits = TypeSupportResolver.resolve(java.util.BitSet.class);
        assertEquals(java.util.BitSet.valueOf(new long[] {9L}),
                bits.decode(bits.encode(java.util.BitSet.valueOf(new long[] {9L}))));
    }

    @Test
    void theOctetsOfAJdkValueNeverMakeAnotherClass() {
        // the Java serialization of a HashMap, read as the BitSet the type says: refused
        TopicTypeSupport<java.util.BitSet> bits = TypeSupportResolver.resolve(java.util.BitSet.class);
        java.io.ByteArrayOutputStream out = new java.io.ByteArrayOutputStream();
        try (java.io.ObjectOutputStream o = new java.io.ObjectOutputStream(out)) {
            o.writeObject(new java.util.HashMap<String, String>());
        } catch (java.io.IOException e) {
            throw new AssertionError(e);
        }
        byte[] stream = out.toByteArray();
        byte[] framed = new byte[4 + stream.length];
        framed[0] = (byte) stream.length;
        framed[1] = (byte) (stream.length >> 8);
        System.arraycopy(stream, 0, framed, 4, stream.length);
        assertThrows(XcdrException.class, () -> bits.decode(framed));
    }

    @Test
    void aValueTheClassRefusesIsRefusedOnReading() {
        // month 13 of a YearMonth
        TopicTypeSupport<java.time.YearMonth> ts = TypeSupportResolver.resolve(java.time.YearMonth.class);
        assertThrows(XcdrException.class, () -> ts.decode(bytes(0xE9, 0x07, 0, 0, 13, 0, 0, 0)));
    }

    // --- §8.5.2 and §8.5.3: the octets of a map and of an array -------------------------

    public static final class OneEntry {
        public Map<String, Integer> m = new LinkedHashMap<>();
    }

    @Test
    void aMapIsADheaderACountAndTheEntries() {
        OneEntry value = new OneEntry();
        value.m.put("a", 1);
        // DHEADER at 0, count 1 at 4; the key "a" is the length 2 (8 to 11), 'a' and NUL
        // (12, 13); the value is 4-aligned (padding 14, 15) and 1 at 16 to 19: 16 octets
        // after the DHEADER.
        assertArrayEquals(bytes(16, 0, 0, 0, 1, 0, 0, 0, 2, 0, 0, 0, 'a', 0, 0, 0, 1, 0, 0, 0),
                encode(value));
        assertEquals(value.m, roundTrip(value).m);
    }

    public static final class IntArray {
        public int[] values = {1, 2};
    }

    @Test
    void anArrayOfPrimitivesIsASequenceWithoutAnElementDheader() {
        // The length 2 and two Int32 values; primitive elements have no DHEADER.
        assertArrayEquals(bytes(2, 0, 0, 0, 1, 0, 0, 0, 2, 0, 0, 0), encode(new IntArray()));
        assertArrayEquals(new int[] {1, 2}, roundTrip(new IntArray()).values);
    }

    public static final class ListAsArray {
        @SerializeAs(value = TypeKind.ARRAY_TYPE, bound = {3})
        public List<Integer> values = new ArrayList<>(Arrays.asList(1, 2, 3));
    }

    @Test
    void aCollectionMarkedAsAnArrayHasAFixedLengthAndNoLengthWord() {
        // An array of 3 Int32 values: no length word.
        assertArrayEquals(bytes(1, 0, 0, 0, 2, 0, 0, 0, 3, 0, 0, 0), encode(new ListAsArray()));
        ListAsArray back = roundTrip(new ListAsArray());
        assertEquals(Arrays.asList(1, 2, 3), back.values);
        assertEquals(ArrayList.class, back.values.getClass());

        // A list of another size is no value of the array type.
        ListAsArray shorter = new ListAsArray();
        shorter.values = new ArrayList<>(Arrays.asList(1, 2));
        assertThrows(XcdrException.class, () -> encode(shorter));
        ListAsArray longer = new ListAsArray();
        longer.values.add(4);
        assertThrows(XcdrException.class, () -> encode(longer));
    }

    // --- §8.6: fields are read and set whatever their visibility ----------------------

    public static final class Visibility {
        private int a = 1;
        protected int b = 2;
        int c = 3;
        public int d = 4;
    }

    private static final class PrivateConstructor {
        int v;

        private PrivateConstructor() {
        }
    }

    @Test
    void fieldsAndTheConstructorAreReachedWhateverTheirVisibility() {
        assertArrayEquals(bytes(1, 0, 0, 0, 2, 0, 0, 0, 3, 0, 0, 0, 4, 0, 0, 0),
                encode(new Visibility()));
        Visibility back = ReflectionTypeSupport.of(Visibility.class)
                .decode(bytes(9, 0, 0, 0, 8, 0, 0, 0, 7, 0, 0, 0, 6, 0, 0, 0));
        assertEquals(9, back.a);
        assertEquals(8, back.b);
        assertEquals(7, back.c);
        assertEquals(6, back.d);
        // A constructor without arguments need not be public (§8.6).
        assertEquals(5, ReflectionTypeSupport.of(PrivateConstructor.class)
                .decode(bytes(5, 0, 0, 0)).v);
    }

    // --- §8.2: SerializeAs on a class says a structure or a union ---------------------

    @SerializeAs(TypeKind.INT_32_TYPE)
    public static final class NotAStructure {
        public int x;
    }

    @SerializeAs(TypeKind.STRUCTURE_TYPE)
    public static final class AStructure {
        public int x = 7;
    }

    @Test
    void aKindOtherThanStructureOrUnionOnAClassIsRefused() {
        XcdrException e = assertThrows(XcdrException.class, () -> ReflectionTypeSupport.of(NotAStructure.class));
        assertTrue(e.getMessage().contains("STRUCTURE_TYPE"), e.getMessage());
        assertArrayEquals(bytes(7, 0, 0, 0), encode(new AStructure()));
    }

    /** Only JDK types that Table 8.1 and Table 8.2 list: they are no reason to refuse a class. */
    public static final class Table81 {
        public Object nothing = null;
        public Integer wrapper = 3;
        public String text = "s";
        public BigInteger counter = BigInteger.TEN;
        public java.math.BigDecimal measure = java.math.BigDecimal.ONE;
        public List<String> list = new ArrayList<>(Arrays.asList("x"));
        public Map<String, Integer> map = new LinkedHashMap<>();
        public int[] array = {1, 2};
    }

    @Test
    void theTypesOfTable81AreNotRefusedAsTypesOfTheJdk() {
        ReflectionTypeSupport<Table81> ts = ReflectionTypeSupport.of(Table81.class);
        assertEquals("org::zerodds::cdr::TypesJavaRepresentationTest::Table81", ts.getTypeName());
    }
}
