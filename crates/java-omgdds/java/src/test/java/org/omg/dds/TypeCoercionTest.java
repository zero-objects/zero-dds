// SPDX-License-Identifier: Apache-2.0
package org.omg.dds;

import java.util.List;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.omg.dds.core.policy.TypeConsistencyEnforcement;
import org.omg.dds.domain.DomainParticipant;
import org.omg.dds.pub.DataWriter;
import org.omg.dds.sub.DataReader;
import org.omg.dds.sub.Sample;
import org.omg.dds.topic.Topic;
import org.omg.dds.type.Extensibility;
import org.omg.dds.type.dynamic.DynamicType;
import org.zerodds.cdr.DynamicTypeFactoryImpl;
import org.zerodds.cdr.ReflectionTypeSupport;
import org.zerodds.cdr.TypeAssignability;
import org.zerodds.internal.InProcessBus;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * TypeConsistencyEnforcement (DDS-Java-PSM 1.0 7.3.5.1): DECLARED and ASSIGNABLE readers take
 * writers of another type name when the types relate as XTypes 1.3 7.2.4.4 (assignable) or as
 * a declared base type (declared); EXACT readers do not.
 */
class TypeCoercionTest {

    private static int dom = 760;

    /** The reader's view: the first two members. */
    public static class Reading {
        public int id;
        public String name;
    }

    /** The writer's view: the same members and one more, appended (XTypes 1.3 7.2.4.2). */
    public static class Writing {
        public int id;
        public String name;
        public double extra;
    }

    /** A different type with a member of the same name and another type. */
    public static class Clashing {
        public String id;
        public String name;
    }

    @Extensibility(Extensibility.Kind.FINAL_EXTENSIBILITY)
    public static class FinalReading {
        public int id;
        public String name;
    }

    public static class Derived extends Reading {
        public long more;
    }

    @BeforeEach
    void setUp() {
        InProcessBus.instance().reset();
    }

    @AfterEach
    void tearDown() {
        InProcessBus.instance().reset();
    }

    private static DynamicType type(Class<?> c) {
        return DynamicTypeFactoryImpl.getInstance(Qos.env()).createType(c);
    }

    @Test
    void javapsm_2_an_appended_member_is_assignable_in_both_directions() {
        assertTrue(TypeAssignability.assignableFrom(type(Reading.class), type(Writing.class)));
        assertTrue(TypeAssignability.assignableFrom(type(Writing.class), type(Reading.class)));
    }

    @Test
    void javapsm_2_a_member_of_the_same_id_and_another_type_is_not_assignable() {
        assertFalse(TypeAssignability.assignableFrom(type(Reading.class), type(Clashing.class)));
    }

    @Test
    void javapsm_2_different_extensibility_is_not_assignable() {
        assertFalse(TypeAssignability.assignableFrom(type(FinalReading.class), type(Reading.class)));
        assertFalse(TypeAssignability.assignableFrom(type(Reading.class), type(FinalReading.class)));
    }

    @Test
    void javapsm_2_a_final_sender_is_not_strongly_assignable() {
        // a final type is not delimited (7.2.4.2), so the type of a member must not be one
        assertFalse(TypeAssignability.strongly(type(Reading.class), type(FinalReading.class)));
        assertTrue(TypeAssignability.strongly(type(Reading.class), type(Writing.class)));
    }

    @Test
    void javapsm_2_a_derived_type_is_declared_as_its_base() {
        assertTrue(TypeAssignability.declared(type(Reading.class), type(Derived.class)));
        assertFalse(TypeAssignability.declared(type(Derived.class), type(Reading.class)));
        assertFalse(TypeAssignability.declared(type(Reading.class), type(Writing.class)));
    }

    private static <R, W> List<Sample<R>> run(TypeConsistencyEnforcement.Kind kind, R unusedR,
            Class<R> reading, Class<W> writing, W sample, int domain) throws Exception {
        DomainParticipant participant = Qos.factory().createParticipant(domain);
        Topic<R> readerTopic = participant.createTopic("coerce" + domain, reading);
        Topic<W> writerTopic = participant.createTopic("coerce" + domain, writing);
        DataReader<R> reader = Qos.reader(participant.createSubscriber(), readerTopic,
                ReflectionTypeSupport.of(reading),
                Qos.reader().withPolicy(Qos.policies().TypeConsistency().withKind(kind)));
        DataWriter<W> writer = Qos.writer(participant.createPublisher(), writerTopic,
                ReflectionTypeSupport.of(writing));
        writer.write(sample);
        List<Sample<R>> taken = Qos.take(reader);
        participant.close();
        return taken;
    }

    @Test
    void javapsm_2_an_assignable_reader_takes_the_samples_of_another_type_name() throws Exception {
        Writing w = new Writing();
        w.id = 7;
        w.name = "seven";
        w.extra = 1.5;
        List<Sample<Reading>> taken = run(
                TypeConsistencyEnforcement.Kind.ASSIGNABLE_TYPE_CONSISTENCY, null,
                Reading.class, Writing.class, w, dom++);
        assertEquals(1, taken.size());
        assertEquals(7, taken.get(0).getData().id);
        assertEquals("seven", taken.get(0).getData().name);
    }

    @Test
    void javapsm_2_an_exact_reader_does_not_take_them() throws Exception {
        Writing w = new Writing();
        w.id = 7;
        w.name = "seven";
        List<Sample<Reading>> taken = run(
                TypeConsistencyEnforcement.Kind.EXACT_TYPE_TYPE_CONSISTENCY, null,
                Reading.class, Writing.class, w, dom++);
        assertEquals(0, taken.size());
    }

    @Test
    void javapsm_2_an_assignable_reader_does_not_take_a_clashing_type() throws Exception {
        Clashing c = new Clashing();
        c.id = "x";
        c.name = "y";
        List<Sample<Reading>> taken = run(
                TypeConsistencyEnforcement.Kind.ASSIGNABLE_TYPE_CONSISTENCY, null,
                Reading.class, Clashing.class, c, dom++);
        assertEquals(0, taken.size());
    }

    @Test
    void javapsm_2_a_declared_reader_takes_a_derived_type() throws Exception {
        Derived d = new Derived();
        d.id = 3;
        d.name = "three";
        d.more = 9;
        List<Sample<Reading>> taken = run(
                TypeConsistencyEnforcement.Kind.DECLARED_TYPE_CONSISTENCY, null,
                Reading.class, Derived.class, d, dom++);
        assertEquals(1, taken.size());
        assertEquals(3, taken.get(0).getData().id);
    }
}
