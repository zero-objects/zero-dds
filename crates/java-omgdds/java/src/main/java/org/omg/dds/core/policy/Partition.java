// SPDX-License-Identifier: Apache-2.0
package org.omg.dds.core.policy;

import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import java.util.Objects;
import java.util.regex.Pattern;

/**
 * OMG DDS PartitionQosPolicy — DDS-DCPS 1.4 §2.2.3.13.
 *
 * <p>A set of partition name strings on a Publisher / Subscriber. A
 * DataWriter and a DataReader communicate only if their (Publisher /
 * Subscriber) partition sets <em>overlap</em> — i.e. at least one name on
 * one side matches a name on the other (§2.2.3.13: "establishes a logical
 * partition among the topics visible by the Publisher and Subscriber").
 *
 * <p>An <em>empty</em> partition set is treated as the single name
 * {@code ""} (the default partition), so two endpoints with no partition
 * configured do communicate. Names may contain the wildcards {@code *} and
 * {@code ?} (POSIX {@code fnmatch}); a match is symmetric — a wildcard on
 * either side matches a literal on the other.
 */
public final class Partition {
    /** Default partition = the single empty-string name. */
    public static final Partition DEFAULT = new Partition();

    private final List<String> names;

    public Partition(String... names) {
        this.names = (names == null || names.length == 0)
                ? Collections.emptyList()
                : new ArrayList<>(List.of(names));
    }

    public Partition(List<String> names) {
        this.names = (names == null || names.isEmpty())
                ? Collections.emptyList()
                : new ArrayList<>(names);
    }

    public List<String> names() {
        return Collections.unmodifiableList(names);
    }

    /** Effective name set: empty set is the single default partition {@code ""}. */
    private List<String> effective() {
        return names.isEmpty() ? List.of("") : names;
    }

    /**
     * DDS-DCPS 1.4 §2.2.3.13 partition overlap: the two name sets overlap if
     * any name on one side matches any name on the other (wildcards allowed
     * on either side). Mirrors the runtime {@code partitions_overlap} matcher.
     */
    public boolean overlaps(Partition other) {
        for (String a : this.effective()) {
            for (String b : other.effective()) {
                if (nameMatches(a, b)) {
                    return true;
                }
            }
        }
        return false;
    }

    private static boolean nameMatches(String a, String b) {
        if (a.equals(b)) {
            return true;
        }
        // Wildcard on either side matches a literal on the other (symmetric).
        if (hasWildcard(a) && globMatch(a, b)) {
            return true;
        }
        return hasWildcard(b) && globMatch(b, a);
    }

    private static boolean hasWildcard(String s) {
        return s.indexOf('*') >= 0 || s.indexOf('?') >= 0;
    }

    private static boolean globMatch(String pattern, String input) {
        StringBuilder re = new StringBuilder();
        for (int i = 0; i < pattern.length(); i++) {
            char c = pattern.charAt(i);
            switch (c) {
                case '*': re.append(".*"); break;
                case '?': re.append('.'); break;
                default: re.append(Pattern.quote(String.valueOf(c)));
            }
        }
        return Pattern.matches(re.toString(), input);
    }

    @Override
    public boolean equals(Object o) {
        if (this == o) return true;
        if (!(o instanceof Partition)) return false;
        return names.equals(((Partition) o).names);
    }

    @Override
    public int hashCode() {
        return Objects.hash(names);
    }

    @Override
    public String toString() {
        return "Partition" + (names.isEmpty() ? "[<default>]" : names);
    }
}
