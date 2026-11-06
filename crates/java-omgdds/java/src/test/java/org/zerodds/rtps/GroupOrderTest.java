// SPDX-License-Identifier: Apache-2.0
package org.zerodds.rtps;

import java.util.ArrayList;
import java.util.List;

import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * The ordering across the writers of a Publisher with access scope GROUP (DDSI-RTPS 2.5 8.7.5,
 * 8.7.6): samples are handed on in the order of the group sequence number, a group coherent set is
 * handed on together when its End Coherent Set DATA has come, a missing number is given up by the
 * HEARTBEATs of the counted writers (8.7.5), or after the timeout if that option is on, and a set
 * with a missing sample is not delivered.
 */
class GroupOrderTest {

    private final List<String> out = new ArrayList<String>();
    private final List<String> logged = new ArrayList<String>();
    private final List<Integer> atomicSizes = new ArrayList<Integer>();

    private final java.util.Set<String> counted = new java.util.HashSet<String>(java.util.Arrays.asList("A", "B"));

    private GroupOrder order() {
        return order(0L);
    }

    private GroupOrder order(long timeoutMillis) {
        return new GroupOrder(new java.util.function.Consumer<String>() {
            @Override
            public void accept(String line) {
                logged.add(line);
            }
        }, new java.util.function.Consumer<Runnable>() {
            @Override
            public void accept(Runnable r) {
                int before = out.size();
                r.run();
                atomicSizes.add(out.size() - before);
            }
        }, new java.util.function.Function<String, java.util.Set<String>>() {
            @Override
            public java.util.Set<String> apply(String key) {
                return counted;
            }
        }, timeoutMillis);
    }

    private Runnable sample(final String name) {
        return new Runnable() {
            @Override
            public void run() {
                out.add(name);
            }
        };
    }

    @Test
    void rtps_8_7_5_samples_of_several_writers_are_handed_on_in_the_order_of_the_group_sequence_number() {
        GroupOrder g = order();
        g.accept("p", "A", 11, 0, sample("A11"));
        g.accept("p", "A", 13, 0, sample("A13")); // 12 of the other writer is not here yet
        assertEquals(List.of("A11"), out);
        g.accept("p", "B", 12, 0, sample("B12"));
        assertEquals(List.of("A11", "B12", "A13"), out);
    }

    @Test
    void rtps_8_7_6_a_group_coherent_set_is_handed_on_together_when_its_end_has_come() {
        GroupOrder g = order();
        g.accept("p", "A", 11, 11, sample("A11"));
        g.accept("p", "B", 12, 11, sample("B12"));
        g.accept("p", "A", 13, 11, sample("A13"));
        assertTrue(out.isEmpty(), "held until the End Coherent Set DATA");
        g.ecs("p", "A", 14, 11);
        assertEquals(List.of("A11", "B12", "A13"), out);
        assertEquals(List.of(3), atomicSizes);
        // the second writer's End Coherent Set DATA has the same number and changes nothing
        g.ecs("p", "A", 14, 11);
        assertEquals(3, out.size());
        g.accept("p", "A", 15, 0, sample("A15"));
        assertEquals(4, out.size());
    }

    @Test
    void rtps_8_7_6_a_sample_of_another_group_coherent_set_ends_the_open_one() {
        GroupOrder g = order();
        g.accept("p", "A", 1, 1, sample("a"));
        g.accept("p", "A", 2, 1, sample("b"));
        g.accept("p", "A", 3, 0, sample("c"));
        assertEquals(List.of("a", "b", "c"), out);
    }

    @Test
    void rtps_8_7_5_the_groups_of_two_publishers_are_independent() {
        GroupOrder g = order();
        g.accept("p", "A", 5, 0, sample("p5"));
        g.accept("q", "A", 9, 0, sample("q9"));
        g.accept("p", "A", 7, 0, sample("p7")); // 6 missing in p only
        assertEquals(List.of("p5", "q9"), out);
    }

    @Test
    void rtps_8_7_5_a_group_sequence_number_is_given_up_when_every_writer_says_it_is_not_its_own() {
        GroupOrder g = order();
        g.accept("p", "A", 10, 0, sample("a10"));
        g.accept("p", "A", 12, 0, sample("a12")); // 11 is missing
        assertEquals(List.of("a10"), out);
        // A has delivered 12, but B has not been heard of: B may hold 11
        g.heartbeat("p", "A", 12, 10, 12);
        assertEquals(List.of("a10"), out);
        // B: currentGSN 12 >= 11, its range [13, 12] is empty
        g.heartbeat("p", "B", 12, 1, 0);
        assertEquals(List.of("a10", "a12"), out);
        assertTrue(logged.get(0).contains("11"), logged.toString());
    }

    @Test
    void rtps_8_7_5_a_heartbeat_that_holds_the_number_keeps_the_order_waiting() {
        GroupOrder g = order();
        g.accept("p", "A", 10, 0, sample("a10"));
        g.accept("p", "A", 12, 0, sample("a12"));
        g.heartbeat("p", "B", 12, 11, 11); // B has 11 and it is on the way
        assertEquals(List.of("a10"), out);
        g.accept("p", "B", 11, 0, sample("b11"));
        assertEquals(List.of("a10", "b11", "a12"), out);
    }

    @Test
    void rtps_8_7_5_a_heartbeat_sent_before_the_number_was_assigned_gives_nothing_up() {
        GroupOrder g = order();
        g.accept("p", "A", 10, 0, sample("a10"));
        g.accept("p", "A", 12, 0, sample("a12"));
        g.heartbeat("p", "B", 10, 1, 0); // currentGSN 10 < 11: B may write 11 later
        assertEquals(List.of("a10"), out);
    }

    @Test
    void rtps_8_7_6_a_group_coherent_set_with_a_number_given_up_is_not_delivered() {
        GroupOrder g = order();
        g.accept("p", "A", 1, 1, sample("a1"));
        g.accept("p", "A", 3, 1, sample("a3"));
        g.ecs("p", "A", 4, 1);
        g.heartbeat("p", "B", 4, 1, 0);
        assertTrue(out.isEmpty(), out.toString());
        assertTrue(logged.get(1).contains("incomplete"), logged.toString());
    }
}
