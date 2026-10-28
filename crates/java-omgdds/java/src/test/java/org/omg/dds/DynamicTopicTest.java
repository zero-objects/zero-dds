// SPDX-License-Identifier: Apache-2.0
package org.omg.dds;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.omg.dds.core.ServiceEnvironment;
import org.omg.dds.domain.DomainParticipant;
import org.omg.dds.pub.DataWriter;
import org.omg.dds.sub.DataReader;
import org.omg.dds.sub.Sample;
import org.omg.dds.topic.Topic;
import org.omg.dds.type.Key;
import org.omg.dds.type.TypeKind;
import org.omg.dds.type.dynamic.AnnotationDescriptor;
import org.omg.dds.type.dynamic.DynamicData;
import org.omg.dds.type.dynamic.DynamicDataFactory;
import org.omg.dds.type.dynamic.DynamicType;
import org.omg.dds.type.dynamic.DynamicTypeFactory;
import org.omg.dds.type.dynamic.DynamicTypeMember;
import org.zerodds.cdr.DynamicDataTypeSupport;
import org.zerodds.cdr.EndianMode;
import org.zerodds.cdr.ReflectionTypeSupport;
import org.zerodds.domain.DomainParticipantImpl;
import org.zerodds.domain.DynamicTopics;
import org.zerodds.internal.InProcessBus;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * DDS-Java-PSM 1.0 §7.5.1 and §7.8.1: {@code createTopic(String, DynamicType)}
 * works. The topic it returns, typed {@code Topic<DynamicType>} by the PSM,
 * is the topic of {@code DynamicData} samples, and a writer and a reader on
 * it move them.
 */
class DynamicTopicTest {

    private static int dom = 800;

    private static int nextDom() {
        return dom++;
    }

    private static final ServiceEnvironment ENV = Qos.env();
    private static final DynamicTypeFactory TYPES = DynamicTypeFactory.getInstance(ENV);

    @BeforeEach
    void setUp() {
        InProcessBus.instance().reset();
    }

    @AfterEach
    void tearDown() {
        InProcessBus.instance().reset();
    }

    private static DynamicType primitive(TypeKind kind) {
        return TYPES.getPrimitiveType(kind);
    }

    private static AnnotationDescriptor annotation(String name, String value) {
        DynamicType type = TYPES.createType(TYPES.newTypeDescriptor()
                .withKind(TypeKind.ANNOTATION_TYPE).withName(name));
        AnnotationDescriptor a = TYPES.newAnnotationDescriptor().withType(type);
        return value == null ? a : a.withValue("value", value);
    }

    private static DynamicTypeMember member(DynamicType struct, String name, int id,
            DynamicType type) {
        return struct.addMember(TYPES.newMemberDescriptor().withName(name).withId(id)
                .withType(type));
    }

    /** struct Reading { @key long id; string name; sequence<double> values; }. */
    private static DynamicType readingType(String extensibility) {
        DynamicType struct = TYPES.createType(TYPES.newTypeDescriptor()
                .withKind(TypeKind.STRUCTURE_TYPE).withName("DynReading"));
        if (extensibility != null) {
            struct.setAnnotation(annotation("Extensibility", extensibility));
        }
        member(struct, "id", 1, primitive(TypeKind.INT_32_TYPE))
                .setAnnotation(annotation("Key", null));
        member(struct, "name", 2, TYPES.createStringType(0));
        member(struct, "values", 3,
                TYPES.createSequenceType(primitive(TypeKind.FLOAT_64_TYPE)));
        return struct;
    }

    private static DynamicData reading(DynamicType type, int id, String name, double... values) {
        return DynamicDataFactory.getInstance(ENV).newDynamicData(type)
                .setInt32Value(1, id)
                .setStringValue(2, name)
                .setFloat64Values(3, values, 0, values.length);
    }

    @Test
    void aWriterAndAReaderOnTheTopicOfADynamicTypeMoveDynamicData() throws Exception {
        DynamicType type = readingType(null);
        DomainParticipant participant = Qos.factory().createParticipant(nextDom());

        // PSM 1.0 §7.5.1: the topic is typed Topic<DynamicType>.
        Topic<DynamicType> described = participant.createTopic("Readings", type);
        Topic<DynamicData> topic = DynamicTopics.dataTopic(described);
        assertSame(described, topic);
        assertEquals("DynReading", topic.getTypeName());

        DataWriter<DynamicData> writer = participant.createPublisher().createDataWriter(topic);
        DataReader<DynamicData> reader = participant.createSubscriber().createDataReader(topic);

        writer.write(reading(type, 7, "abc", 1.5, 2.5));
        writer.write(reading(type, 8, "def"));

        List<Sample<DynamicData>> got = Qos.take(reader);
        assertEquals(2, got.size(), "one instance per key value");
        DynamicData first = got.get(0).getData();
        assertEquals(7, first.getInt32Value(1));
        assertEquals("abc", first.getStringValue(2));
        double[] values = new double[2];
        assertEquals(2, first.getFloat64Values(values, 0, 2, 3));
        assertArrayEquals(new double[] {1.5, 2.5}, values);
        assertEquals(8, got.get(1).getData().getInt32Value(1));
        assertEquals("def", got.get(1).getData().getStringValue(2));
    }

    @Test
    void theKeyAnnotationMakesTheTopicKeyed() throws Exception {
        DynamicType type = readingType(null);
        DynamicDataTypeSupport codec = new DynamicDataTypeSupport(ENV, type);
        assertTrue(codec.isKeyed());
        assertArrayEquals(codec.keyHash(reading(type, 5, "a")), codec.keyHash(reading(type, 5, "b")));
        assertTrue(!java.util.Arrays.equals(codec.keyHash(reading(type, 5, "a")),
                codec.keyHash(reading(type, 6, "a"))));

        DomainParticipant participant = Qos.factory().createParticipant(nextDom());
        Topic<DynamicData> topic = ((DomainParticipantImpl) participant)
                .createDynamicTopic("Readings", type);
        DataWriter<DynamicData> writer = participant.createPublisher().createDataWriter(topic);
        DataReader<DynamicData> reader = participant.createSubscriber().createDataReader(topic);
        writer.write(reading(type, 5, "first"));
        writer.write(reading(type, 5, "second"));
        List<Sample<DynamicData>> got = Qos.take(reader);
        assertEquals(1, got.size(), "the same key is the same instance (history depth 1)");
        assertEquals("second", got.get(0).getData().getStringValue(2));
    }

    /** XTypes 1.3 §7.4: the bytes are those of the Java class with the same members. */
    public static final class ReadingBean {
        @Key
        private final int id;
        private final String name;
        private final double[] values;

        public ReadingBean(int id, String name, double[] values) {
            this.id = id;
            this.name = name;
            this.values = values;
        }
    }

    @Test
    void theBytesAreThoseOfTheJavaTypeRepresentation() {
        // the Java class is final and extends Object: FINAL (DDS-Java-PSM 1.0 §8.6.1.2)
        DynamicType type = readingType("FINAL");
        DynamicDataTypeSupport codec = new DynamicDataTypeSupport(ENV, type);
        byte[] dynamic = codec.encode(reading(type, 7, "abc", 1.5, 2.5));
        byte[] reflective = ReflectionTypeSupport.of(ReadingBean.class)
                .encode(new ReadingBean(7, "abc", new double[] {1.5, 2.5}), EndianMode.LITTLE_ENDIAN);
        assertArrayEquals(reflective, dynamic);
    }

    @Test
    void theExtensibilityAnnotationChoosesTheFraming() {
        DynamicType plain = readingType("FINAL");
        DynamicType appendable = readingType("APPENDABLE");
        DynamicType mutable = readingType("MUTABLE");
        byte[] fin = new DynamicDataTypeSupport(ENV, plain).encode(reading(plain, 1, "x"));
        byte[] app = new DynamicDataTypeSupport(ENV, appendable).encode(reading(appendable, 1, "x"));
        byte[] mut = new DynamicDataTypeSupport(ENV, mutable).encode(reading(mutable, 1, "x"));

        // APPENDABLE: a DHEADER (the length of the rest) before the members.
        assertEquals(fin.length + 4, app.length);
        assertEquals(fin.length, (app[0] & 0xff) | (app[1] & 0xff) << 8);
        // MUTABLE: a DHEADER, then an EMHEADER (and a NEXTINT) per member.
        assertEquals(mut.length - 4, (mut[0] & 0xff) | (mut[1] & 0xff) << 8);
        assertTrue(mut.length > app.length);

        DynamicData back = new DynamicDataTypeSupport(ENV, mutable).decode(mut);
        assertEquals(1, back.getInt32Value(1));
        assertEquals("x", back.getStringValue(2));
    }

    @Test
    void aUnionIsItsDiscriminatorAndTheSelectedMember() {
        DynamicType union = TYPES.createType(TYPES.newTypeDescriptor()
                .withKind(TypeKind.UNION_TYPE).withName("DynChoice")
                .withDiscriminatorType(primitive(TypeKind.INT_32_TYPE)));
        union.addMember(TYPES.newMemberDescriptor().withName("number").withId(1)
                .withType(primitive(TypeKind.INT_32_TYPE)).withLabel(1));
        union.addMember(TYPES.newMemberDescriptor().withName("text").withId(2)
                .withType(TYPES.createStringType(0)).withLabel(2));
        DynamicDataTypeSupport codec = new DynamicDataTypeSupport(ENV, union);

        DynamicData choice = DynamicDataFactory.getInstance(ENV).newDynamicData(union)
                .setStringValue(2, "hello");
        byte[] bytes = codec.encode(choice);
        // A type of unspecified extensibility is appendable (XTypes 1.3 §7.2.2.4.4.4): the
        // DHEADER 14, the discriminator 2, then the string "hello" (length 6 with its NUL).
        assertArrayEquals(new byte[] {14, 0, 0, 0, 2, 0, 0, 0, 6, 0, 0, 0, 'h', 'e', 'l', 'l', 'o', 0},
                bytes);
        DynamicData back = codec.decode(bytes);
        assertEquals("hello", back.getStringValue(2));
    }

    @Test
    void aStructureWithANestedStructureAndAnArray() {
        DynamicType point = TYPES.createType(TYPES.newTypeDescriptor()
                .withKind(TypeKind.STRUCTURE_TYPE).withName("DynPoint"));
        member(point, "x", 1, primitive(TypeKind.INT_32_TYPE));
        member(point, "y", 2, primitive(TypeKind.INT_32_TYPE));
        DynamicType shape = TYPES.createType(TYPES.newTypeDescriptor()
                .withKind(TypeKind.STRUCTURE_TYPE).withName("DynShape"));
        member(shape, "origin", 1, point);
        member(shape, "corners", 2, TYPES.createArrayType(point, 2));
        member(shape, "flags", 3, TYPES.createArrayType(primitive(TypeKind.BOOLEAN_TYPE), 3));
        DynamicDataFactory data = DynamicDataFactory.getInstance(ENV);

        DynamicData origin = data.newDynamicData(point).setInt32Value(1, 3).setInt32Value(2, 4);
        DynamicData c0 = data.newDynamicData(point).setInt32Value(1, 5).setInt32Value(2, 6);
        DynamicData c1 = data.newDynamicData(point).setInt32Value(1, 7).setInt32Value(2, 8);
        DynamicData sample = data.newDynamicData(shape).setComplexValue(1, origin);
        sample.setValue(2, java.util.Arrays.asList(c0, c1), List.class);
        sample.setBooleanValues(3, new boolean[] {true, false, true}, 0, 3);

        DynamicDataTypeSupport codec = new DynamicDataTypeSupport(ENV, shape);
        DynamicData back = codec.decode(codec.encode(sample));
        assertEquals(3, back.getComplexValue(1).getInt32Value(1));
        assertEquals(4, back.getComplexValue(1).getInt32Value(2));
        boolean[] flags = new boolean[3];
        back.getBooleanValues(flags, 0, 3, 3);
        assertArrayEquals(new boolean[] {true, false, true}, flags);
        @SuppressWarnings("unchecked")
        List<DynamicData> corners = back.getValue(2, List.class);
        assertEquals(2, corners.size());
        assertEquals(7, corners.get(1).getInt32Value(1));
    }

    @Test
    void aTopicNeedsAStructureOrAUnion() {
        DomainParticipant participant = Qos.factory().createParticipant(nextDom());
        assertThrows(IllegalArgumentException.class, () -> participant.createTopic("Bad",
                primitive(TypeKind.INT_32_TYPE)));
        assertThrows(IllegalArgumentException.class, () -> participant.createTopic("Bad",
                (DynamicType) null));
    }

    @Test
    void onlyTheTopicOfADynamicTypeConvertsToDynamicData() {
        DomainParticipant participant = Qos.factory().createParticipant(nextDom());
        @SuppressWarnings({"unchecked", "rawtypes"})
        Topic<DynamicType> wrong = (Topic) participant.createTopic("Plain", String.class);
        assertThrows(IllegalArgumentException.class, () -> DynamicTopics.dataTopic(wrong));
    }
}
