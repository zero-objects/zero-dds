// SPDX-License-Identifier: Apache-2.0
package org.omg.dds;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.omg.dds.domain.DomainParticipant;
import org.omg.dds.pub.DataWriter;
import org.omg.dds.pub.Publisher;
import org.omg.dds.sub.DataReader;
import org.omg.dds.sub.InstanceState;
import org.omg.dds.sub.Sample;
import org.omg.dds.sub.Subscriber;
import org.omg.dds.topic.MultiTopic;
import org.omg.dds.topic.Topic;
import org.omg.dds.type.Key;
import org.zerodds.internal.InProcessBus;
import org.zerodds.topic.MultiTopicExpression;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * DDS-DCPS 1.4 §2.2.2.3.4 and Annex B: a MultiTopic that joins topics,
 * projects and renames members, and filters the joined row.
 */
class MultiTopicJoinTest {

    private static int dom = 400;

    private static int nextDom() {
        return dom++;
    }

    public static final class Position {
        @Key
        private final String flight;
        private final double x;
        private final double z;

        public Position(String flight, double x, double z) {
            this.flight = flight;
            this.x = x;
            this.z = z;
        }
    }

    public static final class Plan {
        @Key
        private final String flight;
        private final String destination;

        public Plan(String flight, String destination) {
            this.flight = flight;
            this.destination = destination;
        }
    }

    /** The resulting type: z is renamed to height. */
    public static final class Track {
        @Key
        private final String flight;
        private final double x;
        private final double height;
        private final String destination;

        public Track(String flight, double x, double height, String destination) {
            this.flight = flight;
            this.x = x;
            this.height = height;
            this.destination = destination;
        }

        String flight() {
            return flight;
        }

        double x() {
            return x;
        }

        double height() {
            return height;
        }

        String destination() {
            return destination;
        }
    }

    public static final class Pilot {
        @Key
        private final String flight;
        private final String name;

        public Pilot(String flight, String name) {
            this.flight = flight;
            this.name = name;
        }
    }

    public static final class Crewed {
        @Key
        private final String flight;
        private final String destination;
        private final String name;

        public Crewed(String flight, String destination, String name) {
            this.flight = flight;
            this.destination = destination;
            this.name = name;
        }

        String flight() {
            return flight;
        }

        String destination() {
            return destination;
        }

        String name() {
            return name;
        }
    }

    @BeforeEach
    void setUp() throws Exception {
        InProcessBus.instance().reset();
    }

    @AfterEach
    void tearDown() throws Exception {
        InProcessBus.instance().reset();
        System.clearProperty("zerodds.multitopic.join");
    }

    private static final String TRACKS =
            "SELECT flight, x, z AS height, destination FROM Position NATURAL JOIN Plan"
                    + " WHERE height < 1000 AND x < 23";

    private static final class Rig {
        final DomainParticipant participant = Qos.factory().createParticipant(nextDom());
        final Publisher publisher = participant.createPublisher();
        final Subscriber subscriber = participant.createSubscriber();
        final DataWriter<Position> positions;
        final DataWriter<Plan> plans;

        Rig() {
            Topic<Position> positionTopic = participant.createTopic("Position", Position.class);
            Topic<Plan> planTopic = participant.createTopic("Plan", Plan.class);
            positions = publisher.createDataWriter(positionTopic);
            // The tests that unregister expect NOT_ALIVE_NO_WRITERS; an unregister disposes
            // first when autodispose_unregistered_instances is true (DDS 1.4 §2.2.3.21).
            plans = publisher.createDataWriter(planTopic, Qos.writer().withPolicy(
                    Qos.pf().WriterDataLifecycle().withAutDisposeUnregisteredInstances(false)));
        }
    }

    @Test
    void javapsm_2_the_annex_b_example_joins_projects_renames_and_filters() throws Exception {
        Rig rig = new Rig();
        MultiTopic<Track> tracks = rig.participant.createMultiTopic("Tracks",
                Track.class, TRACKS, List.of());
        DataReader<Track> reader = rig.subscriber.createDataReader(tracks);

        rig.plans.write(new Plan("AF1", "PAR"));
        rig.positions.write(new Position("AF1", 10, 500));
        // No plan for AF2 yet: nothing to join.
        rig.positions.write(new Position("AF2", 11, 600));
        // The WHERE clause rejects these: x is not below 23, z is not below 1000.
        rig.positions.write(new Position("AF1", 30, 500));
        rig.positions.write(new Position("AF1", 12, 5000));

        List<Sample<Track>> got = Qos.take(reader);
        assertEquals(1, got.size());
        Track track = got.get(0).getData();
        assertEquals("AF1", track.flight());
        assertEquals(10.0, track.x());
        assertEquals(500.0, track.height());
        assertEquals("PAR", track.destination());
        assertEquals(InstanceState.ALIVE, got.get(0).getInstanceState());

        // The plan of AF2 arrives after its position: the stored position joins it.
        rig.plans.write(new Plan("AF2", "LON"));
        got = Qos.take(reader);
        assertEquals(1, got.size());
        assertEquals("AF2", got.get(0).getData().flight());
        assertEquals(600.0, got.get(0).getData().height());
        assertEquals("LON", got.get(0).getData().destination());
    }

    @Test
    void javapsm_2_an_update_of_a_constituent_writes_the_instance_again() throws Exception {
        Rig rig = new Rig();
        MultiTopic<Track> tracks = rig.participant.createMultiTopic("Tracks",
                Track.class, TRACKS, List.of());
        DataReader<Track> reader = rig.subscriber.createDataReader(tracks);

        rig.plans.write(new Plan("AF1", "PAR"));
        rig.positions.write(new Position("AF1", 10, 500));
        rig.positions.write(new Position("AF1", 11, 450));
        rig.plans.write(new Plan("AF1", "ROM"));

        List<Sample<Track>> got = Qos.take(reader);
        // One instance (flight AF1); the history keeps the last sample (depth 1).
        assertEquals(1, got.size());
        assertEquals("ROM", got.get(0).getData().destination());
        assertEquals(11.0, got.get(0).getData().x());
    }

    @Test
    void javapsm_7_7_2_disposing_a_constituent_disposes_the_result_instance() throws Exception {
        Rig rig = new Rig();
        MultiTopic<Track> tracks = rig.participant.createMultiTopic("Tracks",
                Track.class, TRACKS, List.of());
        DataReader<Track> reader = rig.subscriber.createDataReader(tracks);
        rig.plans.write(new Plan("AF1", "PAR"));
        rig.positions.write(new Position("AF1", 10, 500));
        assertEquals(1, Qos.take(reader).size());

        Qos.dispose(rig.positions, new Position("AF1", 10, 500));
        List<Sample<Track>> got = Qos.take(reader);
        assertEquals(1, got.size());
        assertNull(got.get(0).getData());
        assertEquals(InstanceState.NOT_ALIVE_DISPOSED, got.get(0).getInstanceState());
    }

    @Test
    void javapsm_2_the_where_clause_uses_the_expression_parameters() throws Exception {
        Rig rig = new Rig();
        MultiTopic<Track> tracks = rig.participant.createMultiTopic("Tracks",
                Track.class,
                "SELECT flight, x, z AS height, destination FROM Position"
                        + " NATURAL JOIN Plan WHERE height < %0",
                List.of("1000"));
        DataReader<Track> reader = rig.subscriber.createDataReader(tracks);
        rig.plans.write(new Plan("AF1", "PAR"));
        rig.positions.write(new Position("AF1", 10, 1500));
        assertEquals(0, Qos.take(reader).size());

        tracks.setExpressionParameters(List.of("2000"));
        rig.positions.write(new Position("AF1", 10, 1500));
        assertEquals(1, Qos.take(reader).size());
    }

    @Test
    void javapsm_2_the_join_of_three_topics_and_the_other_spellings_of_the_keyword() throws Exception {
        DomainParticipant participant = Qos.factory().createParticipant(nextDom());
        Publisher publisher = participant.createPublisher();
        Subscriber subscriber = participant.createSubscriber();
        DataWriter<Position> positions =
                publisher.createDataWriter(participant.createTopic("Position", Position.class));
        DataWriter<Plan> plans =
                publisher.createDataWriter(participant.createTopic("Plan", Plan.class));
        DataWriter<Pilot> pilots =
                publisher.createDataWriter(participant.createTopic("Pilot", Pilot.class));

        for (String join : new String[] {
                "SELECT flight, destination, name FROM Plan NATURAL JOIN Pilot;",
                "SELECT flight, destination, name FROM 'Plan' INNER NATURAL JOIN 'Pilot'",
                "SELECT flight, destination, name FROM Plan NATURAL INNER JOIN"
                        + " (Pilot NATURAL JOIN Position)"}) {
            MultiTopic<Crewed> crew = participant.createMultiTopic("Crew" + join.hashCode(),
                    Crewed.class, join, List.of());
            DataReader<Crewed> reader = subscriber.createDataReader(crew);
            positions.write(new Position("AF1", 1, 1));
            plans.write(new Plan("AF1", "PAR"));
            pilots.write(new Pilot("AF1", "Curie"));
            List<Sample<Crewed>> got = Qos.take(reader);
            assertEquals(1, got.size(), join);
            assertEquals("Curie", got.get(0).getData().name(), join);
            assertEquals("PAR", got.get(0).getData().destination(), join);
        }
    }

    @Test
    void javapsm_2_topics_that_share_no_member_join_as_a_cross_product() throws Exception {
        DomainParticipant participant = Qos.factory().createParticipant(nextDom());
        Publisher publisher = participant.createPublisher();
        Subscriber subscriber = participant.createSubscriber();
        DataWriter<Position> positions =
                publisher.createDataWriter(participant.createTopic("Position", Position.class));
        DataWriter<Marker> markers =
                publisher.createDataWriter(participant.createTopic("Marker", Marker.class));
        MultiTopic<Pair> pairs = participant.createMultiTopic("Pairs", Pair.class,
                "SELECT flight, label FROM Position NATURAL JOIN Marker", List.of());
        DataReader<Pair> reader = subscriber.createDataReader(pairs);

        positions.write(new Position("AF1", 1, 1));
        positions.write(new Position("AF2", 2, 2));
        markers.write(new Marker("m"));
        // Pair is keyed by flight: the history keeps one sample per instance.
        assertEquals(2, Qos.take(reader).size());
    }

    public static final class Marker {
        private final String label;

        public Marker(String label) {
            this.label = label;
        }
    }

    public static final class Pair {
        @Key
        private final String flight;
        private final String label;

        public Pair(String flight, String label) {
            this.flight = flight;
            this.label = label;
        }
    }

    // --- Mistakes are refused when the MultiTopic is created --------------

    @Test
    void javapsm_2_a_multi_topic_that_does_not_fit_is_refused() throws Exception {
        Rig rig = new Rig();
        // A field no topic has.
        assertThrows(IllegalArgumentException.class, () ->
                rig.participant.createMultiTopic("A", Track.class,
                        "SELECT flight, nothing FROM Position NATURAL JOIN Plan", List.of()));
        // A WHERE clause that reads a member no topic has.
        assertThrows(IllegalArgumentException.class, () ->
                rig.participant.createMultiTopic("B", Track.class,
                        TRACKS + " AND nothing = 1", List.of()));
        // A result type without a member for an output field.
        assertThrows(IllegalArgumentException.class, () ->
                rig.participant.createMultiTopic("C", Track.class,
                        "SELECT flight, z FROM Position NATURAL JOIN Plan", List.of()));
        // A parameter the list does not have.
        assertThrows(IllegalArgumentException.class, () ->
                rig.participant.createMultiTopic("D", Track.class,
                        "SELECT flight, x, z AS height, destination FROM Position"
                                + " NATURAL JOIN Plan WHERE height < %1", List.of("1")));
        // A field whose type does not fit the member of the result type.
        assertThrows(IllegalArgumentException.class, () ->
                rig.participant.createMultiTopic("E", Track.class,
                        "SELECT flight, destination AS x, z AS height, destination"
                                + " FROM Position NATURAL JOIN Plan", List.of()));
        // A topic of the FROM clause that does not exist.
        assertThrows(IllegalArgumentException.class, () ->
                rig.participant.createMultiTopic("F", Track.class, "SELECT * FROM Nothing", List.of()));
    }

    public static final class Clash {
        @Key
        private final String flight;
        private final int x;

        public Clash(String flight, int x) {
            this.flight = flight;
            this.x = x;
        }
    }

    @Test
    void javapsm_2_join_members_of_different_types_are_refused() throws Exception {
        DomainParticipant participant = Qos.factory().createParticipant(nextDom());
        participant.createTopic("Position", Position.class);
        participant.createTopic("Clash", Clash.class);
        // Position.x is a double, Clash.x an int (Annex B: same type).
        assertThrows(IllegalArgumentException.class, () ->
                participant.createMultiTopic("Bad", Track.class,
                        "SELECT flight, x FROM Position NATURAL JOIN Clash", List.of()));
    }

    // --- The setting that restricts the join to the key members ----------

    public static final class Left {
        @Key
        private final String id;
        private final String tag;

        public Left(String id, String tag) {
            this.id = id;
            this.tag = tag;
        }
    }

    public static final class Right {
        @Key
        private final String id;
        private final String tag;
        private final String extra;

        public Right(String id, String tag, String extra) {
            this.id = id;
            this.tag = tag;
            this.extra = extra;
        }
    }

    public static final class Both {
        @Key
        private final String id;
        private final String tag;
        private final String extra;

        public Both(String id, String tag, String extra) {
            this.id = id;
            this.tag = tag;
            this.extra = extra;
        }

        String tag() {
            return tag;
        }

        String extra() {
            return extra;
        }
    }

    private static List<Sample<Both>> joinLeftRight() throws Exception {
        DomainParticipant participant = Qos.factory().createParticipant(nextDom());
        Publisher publisher = participant.createPublisher();
        DataWriter<Left> lefts =
                publisher.createDataWriter(participant.createTopic("Left", Left.class));
        DataWriter<Right> rights =
                publisher.createDataWriter(participant.createTopic("Right", Right.class));
        MultiTopic<Both> both = participant.createMultiTopic("Both", Both.class,
                "SELECT * FROM Left NATURAL JOIN Right", List.of());
        DataReader<Both> reader = participant.createSubscriber().createDataReader(both);
        lefts.write(new Left("k", "a"));
        rights.write(new Right("k", "b", "e"));
        return Qos.take(reader);
    }

    @Test
    void javapsm_2_the_natural_join_compares_every_member_the_topics_share() throws Exception {
        // Left.tag and Right.tag differ, so the tuple is not in the natural join.
        assertEquals(0, joinLeftRight().size());
    }

    // --- The reader's own bookkeeping ---------------------------------------

    @Test
    void javapsm_2_closing_the_reader_ends_the_join() throws Exception {
        Rig rig = new Rig();
        MultiTopic<Track> tracks = rig.participant.createMultiTopic("Tracks",
                Track.class, TRACKS, List.of());
        DataReader<Track> reader = rig.subscriber.createDataReader(tracks);
        assertSame(tracks, reader.getTopicDescription());
        assertTrue(rig.subscriber.getDataReaders(new java.util.ArrayList<>()).contains(reader));
        // The join holds one reader on each source topic.
        assertEquals(1, rig.positions.getMatchedSubscriptions().size());
        assertEquals(1, rig.plans.getMatchedSubscriptions().size());
        reader.close();
        // The writers no longer have a reader to deliver to.
        assertEquals(0, rig.positions.getMatchedSubscriptions().size());
        assertEquals(0, rig.plans.getMatchedSubscriptions().size());
    }

    // --- The parser ---------------------------------------------------------

    @Test
    void javapsm_2_the_parser_reads_the_annex_b_grammar() throws Exception {
        MultiTopicExpression example = MultiTopicExpression.parse(
                "SELECT flight_name, x, y, z AS height FROM 'Location' NATURAL JOIN"
                        + " 'FlightPlan' WHERE height < 1000 AND x <23");
        assertEquals(List.of("Location", "FlightPlan"), example.getTopics());
        assertEquals("height < 1000 AND x <23", example.getWhere());
        assertEquals(4, example.getItems().size());
        assertEquals("height", example.getItems().get(3).getName());
        assertEquals(List.of("z"), example.getItems().get(3).getPath());

        MultiTopicExpression juxtaposed = MultiTopicExpression.parse(
                "SELECT a.b.c total FROM T;");
        assertEquals("total", juxtaposed.getItems().get(0).getName());
        assertEquals(List.of("a", "b", "c"), juxtaposed.getItems().get(0).getPath());

        MultiTopicExpression nested = MultiTopicExpression.parse(
                "SELECT * FROM A INNER NATURAL JOIN (B NATURAL INNER JOIN C)");
        assertTrue(nested.isStar());
        assertEquals(List.of("A", "B", "C"), nested.getTopics());
        assertNull(nested.getWhere());

        MultiTopicExpression unaliased = MultiTopicExpression.parse(
                "SELECT a.b.c FROM T");
        assertEquals("c", unaliased.getItems().get(0).getName());
        assertFalse(unaliased.getItems().get(0).isAliased());
    }

    @Test
    void javapsm_2_the_parser_refuses_what_the_grammar_does_not_have() throws Exception {
        for (String bad : new String[] {
                "", "x < 1", "SELECT FROM T", "SELECT * T", "SELECT * FROM",
                "SELECT * FROM 1T", "SELECT * FROM A NATURAL B",
                "SELECT * FROM A, B", "SELECT * FROM A NATURAL JOIN",
                "SELECT * FROM (A NATURAL JOIN B)", "SELECT a, a FROM T",
                "SELECT * FROM T ORDER BY x", "SELECT * FROM T WHERE",
                "SELECT * FROM T extra"}) {
            assertThrows(IllegalArgumentException.class,
                    () -> MultiTopicExpression.parse(bad), bad);
        }
    }
}
