// SPDX-License-Identifier: Apache-2.0
package org.zerodds.rtps;

import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.List;

/**
 * The QoS policies of the built-in topic data on the wire: the ParameterIds
 * and value layouts DDSI-RTPS 2.5 §9.6.2 and Table 9.13 give to each policy
 * of the DDS 1.4 §2.2.5 built-in topic types, in the little-endian form
 * (PL_CDR_LE, §9.4.2.11) the SPDP and SEDP announcements use.
 *
 * <p>A field that is {@code null} is a policy that is not on the wire; a
 * reader of an announcement takes the default of the entity kind for it
 * (§9.6.3.1: a policy absent from an announcement is that policy at its
 * default). A policy that is present and does not decode is an error
 * ({@link IllegalArgumentException}), not its default.
 *
 * <p>The layouts are those of {@code crates/qos} and
 * {@code crates/rtps/src/publication_data.rs} in this repository, so an
 * announcement of this class is read by the Rust discovery decoders and
 * reads theirs.
 */
public final class QosParameters {

    /** The entity kinds that have built-in topic data (DDS 1.4 §2.2.5). */
    public enum Entity { PARTICIPANT, TOPIC, PUBLICATION, SUBSCRIPTION }

    // ParameterIds of Table 9.13.
    public static final int PID_TIME_BASED_FILTER = 0x0004;
    public static final int PID_OWNERSHIP_STRENGTH = 0x0006;
    public static final int PID_RELIABILITY = 0x001A;
    public static final int PID_LIVELINESS = 0x001B;
    public static final int PID_DURABILITY = 0x001D;
    public static final int PID_DURABILITY_SERVICE = 0x001E;
    public static final int PID_OWNERSHIP = 0x001F;
    public static final int PID_PRESENTATION = 0x0021;
    public static final int PID_DEADLINE = 0x0023;
    public static final int PID_DESTINATION_ORDER = 0x0025;
    public static final int PID_LATENCY_BUDGET = 0x0027;
    public static final int PID_PARTITION = 0x0029;
    public static final int PID_LIFESPAN = 0x002B;
    public static final int PID_USER_DATA = 0x002C;
    public static final int PID_GROUP_DATA = 0x002D;
    public static final int PID_TOPIC_DATA = 0x002E;
    public static final int PID_HISTORY = 0x0040;
    public static final int PID_RESOURCE_LIMITS = 0x0041;
    public static final int PID_TRANSPORT_PRIORITY = 0x0049;
    public static final int PID_DATA_REPRESENTATION = 0x0073;
    public static final int PID_TYPE_CONSISTENCY_ENFORCEMENT = 0x0074;
    /** XTypes 1.3 §7.6.3.2.2: {@code @id(0x0072) type}, the XTypes 1.1 TypeObject. */
    public static final int PID_TYPE_OBJECT = 0x0072;
    /** XTypes 1.3 §7.6.3.2.2: {@code @id(0x0075) type_information}. */
    public static final int PID_TYPE_INFORMATION = 0x0075;
    /**
     * {@code equivalentTypeName} of the Java built-in topic data. DDS-Java-PSM 1.0 gives
     * it {@code @ID(0x0075)}, which XTypes 1.3 §7.6.3.2.2 gave to {@code type_information},
     * so it is a vendor-specific parameter (RTPS 2.5 Table 9.6, bit 0x8000): a
     * sequence of strings, ignored by other vendors.
     */
    public static final int PID_EQUIVALENT_TYPE_NAME = 0x8003;
    /** {@code baseTypeName}; {@code @ID(0x0076)} in the PSM, here as above. */
    public static final int PID_BASE_TYPE_NAME = 0x8004;

    /** A DDS Duration_t as RTPS carries it: seconds and a fraction of 2^-32 s. */
    public static final class Dur {
        public static final Dur ZERO = new Dur(0, 0);
        public static final Dur INFINITE = new Dur(0x7fffffff, 0xffffffffL);

        public final int seconds;
        public final long fraction;

        public Dur(int seconds, long fraction) {
            this.seconds = seconds;
            this.fraction = fraction & 0xffffffffL;
        }

        /** From seconds and nanoseconds (rounded to the nearest fraction). */
        public static Dur ofNanos(long seconds, long nanos) {
            if (seconds >= 0x7fffffffL) {
                return INFINITE;
            }
            long fraction = Math.round(nanos * 4294967296.0 / 1e9);
            if (fraction > 0xffffffffL) {
                fraction = 0xffffffffL;
            }
            return new Dur((int) seconds, fraction);
        }

        public boolean isInfinite() {
            return seconds == 0x7fffffff && fraction == 0xffffffffL;
        }

        /** The nanoseconds of the fraction, rounded. */
        public long nanos() {
            return Math.round(fraction * 1e9 / 4294967296.0);
        }

        @Override
        public boolean equals(Object o) {
            return o instanceof Dur && ((Dur) o).seconds == seconds
                    && ((Dur) o).fraction == fraction;
        }

        @Override
        public int hashCode() {
            return 31 * seconds + Long.hashCode(fraction);
        }

        /** Orders durations; infinite is the largest. */
        public int compareTo(Dur other) {
            if (seconds != other.seconds) {
                return Integer.compare(seconds, other.seconds);
            }
            return Long.compare(fraction, other.fraction);
        }

        @Override
        public String toString() {
            return isInfinite() ? "infinite" : seconds + "s+" + fraction + "/2^32";
        }
    }

    public static final class DurabilityService {
        public Dur cleanupDelay = Dur.ZERO;
        public int historyKind;
        public int historyDepth = 1;
        public int maxSamples = -1;
        public int maxInstances = -1;
        public int maxSamplesPerInstance = -1;
    }

    public static final class Liveliness {
        public int kind;
        public Dur lease = Dur.INFINITE;
    }

    public static final class Reliability {
        public int kind;
        /** DDS 1.4 2.2.3.14: the default max_blocking_time is 100 ms. */
        public Dur maxBlockingTime = Dur.ofNanos(0, 100_000_000L);
    }

    public static final class History {
        public int kind;
        public int depth = 1;
    }

    public static final class ResourceLimits {
        public int maxSamples = -1;
        public int maxInstances = -1;
        public int maxSamplesPerInstance = -1;
    }

    public static final class Presentation {
        public int accessScope;
        public boolean coherentAccess;
        public boolean orderedAccess;
    }

    public static final class TypeConsistency {
        /** XTypes 1.3 §7.6.3.4: 0 DISALLOW_TYPE_COERCION, 1 ALLOW_TYPE_COERCION. */
        public int kind = 1;
        public boolean ignoreSequenceBounds = true;
        public boolean ignoreStringBounds = true;
        public boolean ignoreMemberNames;
        public boolean preventTypeWidening;
        public boolean forceTypeValidation;
    }

    public Integer durability;
    public DurabilityService durabilityService;
    public Dur deadline;
    public Dur latencyBudget;
    public Liveliness liveliness;
    public Reliability reliability;
    public Dur lifespan;
    public byte[] userData;
    public Integer ownership;
    public Integer ownershipStrength;
    public Integer destinationOrder;
    public Presentation presentation;
    public List<String> partition;
    public byte[] topicData;
    public byte[] groupData;
    public Dur timeBasedFilter;
    public Integer transportPriority;
    public History history;
    public ResourceLimits resourceLimits;
    public int[] dataRepresentation;
    public TypeConsistency typeConsistency;
    /** The type of the entity as XTypes 1.3 §7.6.3.2.2 announces it; {@code null} for none. */
    public byte[] typeInformation;
    public byte[] typeObject;
    public List<String> equivalentTypeName;
    public List<String> baseTypeName;

    /** A copy that shares no mutable state of the policy objects it refers to. */
    public QosParameters shallowCopy() {
        QosParameters c = new QosParameters();
        c.durability = durability;
        c.durabilityService = durabilityService;
        c.deadline = deadline;
        c.latencyBudget = latencyBudget;
        c.liveliness = liveliness;
        c.reliability = reliability;
        c.lifespan = lifespan;
        c.userData = userData;
        c.ownership = ownership;
        c.ownershipStrength = ownershipStrength;
        c.destinationOrder = destinationOrder;
        c.presentation = presentation;
        c.partition = partition;
        c.topicData = topicData;
        c.groupData = groupData;
        c.timeBasedFilter = timeBasedFilter;
        c.transportPriority = transportPriority;
        c.history = history;
        c.resourceLimits = resourceLimits;
        c.dataRepresentation = dataRepresentation;
        c.typeConsistency = typeConsistency;
        c.typeInformation = typeInformation;
        c.typeObject = typeObject;
        c.equivalentTypeName = equivalentTypeName;
        c.baseTypeName = baseTypeName;
        return c;
    }

    // ======================================================================
    // Writing
    // ======================================================================

    /**
     * Adds the parameters of the policies the built-in topic data of
     * {@code entity} has (DDS 1.4 §2.2.5) and that this object holds.
     */
    public void writeTo(ParameterList pl, Entity entity) {
        switch (entity) {
            case PARTICIPANT:
                putUserData(pl);
                break;
            case TOPIC:
                putDurability(pl);
                putDurabilityService(pl);
                putDeadline(pl);
                putLatencyBudget(pl);
                putLiveliness(pl);
                putReliability(pl);
                putTransportPriority(pl);
                putLifespan(pl);
                putDestinationOrder(pl);
                putHistory(pl);
                putResourceLimits(pl);
                putOwnership(pl);
                putTopicData(pl);
                putExtensions(pl);
                break;
            case PUBLICATION:
                putDurability(pl);
                putDurabilityService(pl);
                putDeadline(pl);
                putLatencyBudget(pl);
                putLiveliness(pl);
                putReliability(pl);
                putLifespan(pl);
                putUserData(pl);
                putOwnership(pl);
                putOwnershipStrength(pl);
                putDestinationOrder(pl);
                putPresentation(pl);
                putPartition(pl);
                putTopicData(pl);
                putGroupData(pl);
                putExtensions(pl);
                break;
            default:
                putDurability(pl);
                putDeadline(pl);
                putLatencyBudget(pl);
                putLiveliness(pl);
                putReliability(pl);
                putOwnership(pl);
                putDestinationOrder(pl);
                putUserData(pl);
                putTimeBasedFilter(pl);
                putPresentation(pl);
                putPartition(pl);
                putTopicData(pl);
                putGroupData(pl);
                putExtensions(pl);
                break;
        }
    }

    private void putDurability(ParameterList pl) {
        if (durability != null) {
            pl.add(PID_DURABILITY, ParameterList.u32(durability));
        }
    }

    private void putDurabilityService(ParameterList pl) {
        if (durabilityService != null) {
            DurabilityService s = durabilityService;
            Wire w = new Wire(28);
            duration(w, s.cleanupDelay);
            w.u32le(s.historyKind & 0xffffffffL);
            w.u32le(s.historyDepth & 0xffffffffL);
            w.u32le(s.maxSamples & 0xffffffffL);
            w.u32le(s.maxInstances & 0xffffffffL);
            w.u32le(s.maxSamplesPerInstance & 0xffffffffL);
            pl.add(PID_DURABILITY_SERVICE, w.toBytes());
        }
    }

    private void putDeadline(ParameterList pl) {
        if (deadline != null) {
            pl.add(PID_DEADLINE, duration(deadline));
        }
    }

    private void putLatencyBudget(ParameterList pl) {
        if (latencyBudget != null) {
            pl.add(PID_LATENCY_BUDGET, duration(latencyBudget));
        }
    }

    private void putLiveliness(ParameterList pl) {
        if (liveliness != null) {
            Wire w = new Wire(12);
            w.u32le(liveliness.kind & 0xffffffffL);
            duration(w, liveliness.lease);
            pl.add(PID_LIVELINESS, w.toBytes());
        }
    }

    private void putReliability(ParameterList pl) {
        if (reliability != null) {
            Wire w = new Wire(12);
            w.u32le(reliability.kind & 0xffffffffL);
            duration(w, reliability.maxBlockingTime);
            pl.add(PID_RELIABILITY, w.toBytes());
        }
    }

    private void putLifespan(ParameterList pl) {
        if (lifespan != null) {
            pl.add(PID_LIFESPAN, duration(lifespan));
        }
    }

    private void putUserData(ParameterList pl) {
        if (userData != null && userData.length > 0) {
            pl.add(PID_USER_DATA, octets(userData));
        }
    }

    private void putTopicData(ParameterList pl) {
        if (topicData != null && topicData.length > 0) {
            pl.add(PID_TOPIC_DATA, octets(topicData));
        }
    }

    private void putGroupData(ParameterList pl) {
        if (groupData != null && groupData.length > 0) {
            pl.add(PID_GROUP_DATA, octets(groupData));
        }
    }

    private void putOwnership(ParameterList pl) {
        if (ownership != null) {
            pl.add(PID_OWNERSHIP, ParameterList.u32(ownership));
        }
    }

    private void putOwnershipStrength(ParameterList pl) {
        if (ownershipStrength != null) {
            pl.add(PID_OWNERSHIP_STRENGTH, ParameterList.u32(ownershipStrength & 0xffffffffL));
        }
    }

    private void putDestinationOrder(ParameterList pl) {
        if (destinationOrder != null) {
            pl.add(PID_DESTINATION_ORDER, ParameterList.u32(destinationOrder));
        }
    }

    private void putPresentation(ParameterList pl) {
        if (presentation != null) {
            Wire w = new Wire(8);
            w.u32le(presentation.accessScope & 0xffffffffL);
            w.u8(presentation.coherentAccess ? 1 : 0);
            w.u8(presentation.orderedAccess ? 1 : 0);
            w.u8(0);
            w.u8(0);
            pl.add(PID_PRESENTATION, w.toBytes());
        }
    }

    private void putPartition(ParameterList pl) {
        // An empty list is the default partition, which is not on the wire.
        if (partition != null && !partition.isEmpty()) {
            pl.add(PID_PARTITION, ParameterList.stringSeq(partition));
        }
    }

    private void putTimeBasedFilter(ParameterList pl) {
        if (timeBasedFilter != null) {
            pl.add(PID_TIME_BASED_FILTER, duration(timeBasedFilter));
        }
    }

    private void putTransportPriority(ParameterList pl) {
        if (transportPriority != null) {
            pl.add(PID_TRANSPORT_PRIORITY, ParameterList.u32(transportPriority & 0xffffffffL));
        }
    }

    private void putHistory(ParameterList pl) {
        if (history != null) {
            Wire w = new Wire(8);
            w.u32le(history.kind & 0xffffffffL);
            w.u32le(history.depth & 0xffffffffL);
            pl.add(PID_HISTORY, w.toBytes());
        }
    }

    private void putResourceLimits(ParameterList pl) {
        if (resourceLimits != null) {
            Wire w = new Wire(12);
            w.u32le(resourceLimits.maxSamples & 0xffffffffL);
            w.u32le(resourceLimits.maxInstances & 0xffffffffL);
            w.u32le(resourceLimits.maxSamplesPerInstance & 0xffffffffL);
            pl.add(PID_RESOURCE_LIMITS, w.toBytes());
        }
    }

    /** DATA_REPRESENTATION and TYPE_CONSISTENCY_ENFORCEMENT (XTypes 1.3 §7.6.3). */
    private void putExtensions(ParameterList pl) {
        if (typeConsistency != null) {
            // XTypes 1.3 §7.6.3.4.3: kind as a 16-bit value, five booleans.
            TypeConsistency t = typeConsistency;
            Wire w = new Wire(8);
            w.u16le(t.kind);
            w.u8(t.ignoreSequenceBounds ? 1 : 0);
            w.u8(t.ignoreStringBounds ? 1 : 0);
            w.u8(t.ignoreMemberNames ? 1 : 0);
            w.u8(t.preventTypeWidening ? 1 : 0);
            w.u8(t.forceTypeValidation ? 1 : 0);
            w.u8(0);
            pl.add(PID_TYPE_CONSISTENCY_ENFORCEMENT, w.toBytes());
        }
        if (dataRepresentation != null && dataRepresentation.length > 0
                && pl.find(PID_DATA_REPRESENTATION) == null) {
            pl.add(PID_DATA_REPRESENTATION, ParameterList.dataRepresentation(dataRepresentation));
        }
        putOpaque(pl, PID_TYPE_OBJECT, typeObject);
        putOpaque(pl, PID_TYPE_INFORMATION, typeInformation);
        if (equivalentTypeName != null && !equivalentTypeName.isEmpty()) {
            pl.add(PID_EQUIVALENT_TYPE_NAME, ParameterList.stringSeq(equivalentTypeName));
        }
        if (baseTypeName != null && !baseTypeName.isEmpty()) {
            pl.add(PID_BASE_TYPE_NAME, ParameterList.stringSeq(baseTypeName));
        }
    }

    /** A parameter whose value is kept as the octets it has; the length field is 16 bits. */
    private static void putOpaque(ParameterList pl, int pid, byte[] value) {
        if (value == null || value.length == 0 || pl.find(pid) != null) {
            return;
        }
        if (value.length > 65532) {
            throw new IllegalArgumentException("parameter " + Integer.toHexString(pid) + " of "
                    + value.length + " octets exceeds the 16-bit length of a parameter");
        }
        pl.add(pid, value.clone());
    }

    private static byte[] duration(Dur d) {
        Wire w = new Wire(8);
        duration(w, d);
        return w.toBytes();
    }

    private static void duration(Wire w, Dur d) {
        w.u32le(d.seconds & 0xffffffffL);
        w.u32le(d.fraction);
    }

    private static byte[] octets(byte[] data) {
        Wire w = new Wire(data.length + 4);
        w.u32le(data.length);
        w.bytes(data);
        return w.toBytes();
    }

    // ======================================================================
    // Reading
    // ======================================================================

    /**
     * Reads the policies of the built-in topic data of {@code entity} out of
     * a parameter list. A policy with no parameter is left {@code null}.
     *
     * @throws IllegalArgumentException if a parameter that is present does
     *         not hold a value of its layout.
     */
    public static QosParameters readFrom(ParameterList pl, Entity entity) {
        QosParameters q = new QosParameters();
        switch (entity) {
            case PARTICIPANT:
                q.userData = octetSeq(pl, PID_USER_DATA);
                return q;
            case TOPIC:
                q.durability = u32(pl, PID_DURABILITY);
                q.durabilityService = durabilityService(pl);
                q.deadline = dur(pl, PID_DEADLINE);
                q.latencyBudget = dur(pl, PID_LATENCY_BUDGET);
                q.liveliness = liveliness(pl);
                q.reliability = reliability(pl);
                q.transportPriority = u32(pl, PID_TRANSPORT_PRIORITY);
                q.lifespan = dur(pl, PID_LIFESPAN);
                q.destinationOrder = u32(pl, PID_DESTINATION_ORDER);
                q.history = history(pl);
                q.resourceLimits = resourceLimits(pl);
                q.ownership = u32(pl, PID_OWNERSHIP);
                q.topicData = octetSeq(pl, PID_TOPIC_DATA);
                break;
            case PUBLICATION:
                q.durability = u32(pl, PID_DURABILITY);
                q.durabilityService = durabilityService(pl);
                q.deadline = dur(pl, PID_DEADLINE);
                q.latencyBudget = dur(pl, PID_LATENCY_BUDGET);
                q.liveliness = liveliness(pl);
                q.reliability = reliability(pl);
                q.lifespan = dur(pl, PID_LIFESPAN);
                q.userData = octetSeq(pl, PID_USER_DATA);
                q.ownership = u32(pl, PID_OWNERSHIP);
                q.ownershipStrength = u32(pl, PID_OWNERSHIP_STRENGTH);
                q.destinationOrder = u32(pl, PID_DESTINATION_ORDER);
                q.presentation = presentation(pl);
                q.partition = partition(pl);
                q.topicData = octetSeq(pl, PID_TOPIC_DATA);
                q.groupData = octetSeq(pl, PID_GROUP_DATA);
                break;
            default:
                q.durability = u32(pl, PID_DURABILITY);
                q.deadline = dur(pl, PID_DEADLINE);
                q.latencyBudget = dur(pl, PID_LATENCY_BUDGET);
                q.liveliness = liveliness(pl);
                q.reliability = reliability(pl);
                q.ownership = u32(pl, PID_OWNERSHIP);
                q.destinationOrder = u32(pl, PID_DESTINATION_ORDER);
                q.userData = octetSeq(pl, PID_USER_DATA);
                q.timeBasedFilter = dur(pl, PID_TIME_BASED_FILTER);
                q.presentation = presentation(pl);
                q.partition = partition(pl);
                q.topicData = octetSeq(pl, PID_TOPIC_DATA);
                q.groupData = octetSeq(pl, PID_GROUP_DATA);
                break;
        }
        q.typeConsistency = typeConsistency(pl);
        q.dataRepresentation = dataRepresentation(pl);
        q.typeObject = pl.find(PID_TYPE_OBJECT);
        q.typeInformation = pl.find(PID_TYPE_INFORMATION);
        q.equivalentTypeName = stringSeq(pl, PID_EQUIVALENT_TYPE_NAME);
        q.baseTypeName = stringSeq(pl, PID_BASE_TYPE_NAME);
        return q;
    }

    private static void require(byte[] v, int length, String what) {
        if (v.length < length) {
            throw new IllegalArgumentException(what + " has " + v.length
                    + " octets, " + length + " are needed");
        }
    }

    private static Integer u32(ParameterList pl, int pid) {
        byte[] v = pl.find(pid);
        if (v == null) {
            return null;
        }
        require(v, 4, "parameter " + Integer.toHexString(pid));
        return (int) Wire.u32le(v, 0);
    }

    private static Dur dur(ParameterList pl, int pid) {
        byte[] v = pl.find(pid);
        return v == null ? null : duration(v, 0, "parameter " + Integer.toHexString(pid));
    }

    private static Dur duration(byte[] v, int at, String what) {
        require(v, at + 8, what);
        return new Dur((int) Wire.u32le(v, at), Wire.u32le(v, at + 4));
    }

    private static DurabilityService durabilityService(ParameterList pl) {
        byte[] v = pl.find(PID_DURABILITY_SERVICE);
        if (v == null) {
            return null;
        }
        require(v, 28, "PID_DURABILITY_SERVICE");
        DurabilityService s = new DurabilityService();
        s.cleanupDelay = duration(v, 0, "PID_DURABILITY_SERVICE");
        s.historyKind = (int) Wire.u32le(v, 8);
        s.historyDepth = (int) Wire.u32le(v, 12);
        s.maxSamples = (int) Wire.u32le(v, 16);
        s.maxInstances = (int) Wire.u32le(v, 20);
        s.maxSamplesPerInstance = (int) Wire.u32le(v, 24);
        return s;
    }

    private static Liveliness liveliness(ParameterList pl) {
        byte[] v = pl.find(PID_LIVELINESS);
        if (v == null) {
            return null;
        }
        require(v, 12, "PID_LIVELINESS");
        Liveliness l = new Liveliness();
        l.kind = (int) Wire.u32le(v, 0);
        l.lease = duration(v, 4, "PID_LIVELINESS");
        return l;
    }

    private static Reliability reliability(ParameterList pl) {
        byte[] v = pl.find(PID_RELIABILITY);
        if (v == null) {
            return null;
        }
        require(v, 12, "PID_RELIABILITY");
        Reliability r = new Reliability();
        r.kind = (int) Wire.u32le(v, 0);
        r.maxBlockingTime = duration(v, 4, "PID_RELIABILITY");
        return r;
    }

    private static History history(ParameterList pl) {
        byte[] v = pl.find(PID_HISTORY);
        if (v == null) {
            return null;
        }
        require(v, 8, "PID_HISTORY");
        History h = new History();
        h.kind = (int) Wire.u32le(v, 0);
        h.depth = (int) Wire.u32le(v, 4);
        return h;
    }

    private static ResourceLimits resourceLimits(ParameterList pl) {
        byte[] v = pl.find(PID_RESOURCE_LIMITS);
        if (v == null) {
            return null;
        }
        require(v, 12, "PID_RESOURCE_LIMITS");
        ResourceLimits r = new ResourceLimits();
        r.maxSamples = (int) Wire.u32le(v, 0);
        r.maxInstances = (int) Wire.u32le(v, 4);
        r.maxSamplesPerInstance = (int) Wire.u32le(v, 8);
        return r;
    }

    private static Presentation presentation(ParameterList pl) {
        byte[] v = pl.find(PID_PRESENTATION);
        if (v == null) {
            return null;
        }
        require(v, 6, "PID_PRESENTATION");
        Presentation p = new Presentation();
        p.accessScope = (int) Wire.u32le(v, 0);
        p.coherentAccess = v[4] != 0;
        p.orderedAccess = v[5] != 0;
        return p;
    }

    private static List<String> partition(ParameterList pl) {
        return stringSeq(pl, PID_PARTITION);
    }

    private static List<String> stringSeq(ParameterList pl, int pid) {
        byte[] v = pl.find(pid);
        if (v == null) {
            return null;
        }
        String what = "parameter " + Integer.toHexString(pid);
        require(v, 4, what);
        long count = Wire.u32le(v, 0);
        List<String> out = new ArrayList<>();
        int at = 4;
        for (long i = 0; i < count; i++) {
            require(v, at + 4, what);
            long length = Wire.u32le(v, at);
            at += 4;
            if (length < 1 || at + length > v.length) {
                throw new IllegalArgumentException(
                        what + " string of " + length + " octets does not fit");
            }
            out.add(new String(v, at, (int) length - 1, StandardCharsets.UTF_8));
            at += (int) length;
            at = (at + 3) & ~3;
        }
        return out;
    }

    private static byte[] octetSeq(ParameterList pl, int pid) {
        byte[] v = pl.find(pid);
        if (v == null) {
            return null;
        }
        require(v, 4, "parameter " + Integer.toHexString(pid));
        long length = Wire.u32le(v, 0);
        if (length > v.length - 4L) {
            throw new IllegalArgumentException("parameter " + Integer.toHexString(pid)
                    + " holds " + length + " octets in " + (v.length - 4) + " octets");
        }
        return Arrays.copyOfRange(v, 4, 4 + (int) length);
    }

    private static TypeConsistency typeConsistency(ParameterList pl) {
        byte[] v = pl.find(PID_TYPE_CONSISTENCY_ENFORCEMENT);
        if (v == null) {
            return null;
        }
        require(v, 2, "PID_TYPE_CONSISTENCY_ENFORCEMENT");
        TypeConsistency t = new TypeConsistency();
        t.kind = Wire.u16le(v, 0);
        // A flag the parameter is too short to carry keeps its default.
        if (v.length > 2) {
            t.ignoreSequenceBounds = v[2] != 0;
        }
        if (v.length > 3) {
            t.ignoreStringBounds = v[3] != 0;
        }
        if (v.length > 4) {
            t.ignoreMemberNames = v[4] != 0;
        }
        if (v.length > 5) {
            t.preventTypeWidening = v[5] != 0;
        }
        if (v.length > 6) {
            t.forceTypeValidation = v[6] != 0;
        }
        return t;
    }

    private static int[] dataRepresentation(ParameterList pl) {
        byte[] v = pl.find(PID_DATA_REPRESENTATION);
        if (v == null) {
            return null;
        }
        require(v, 4, "PID_DATA_REPRESENTATION");
        long count = Wire.u32le(v, 0);
        if (count > (v.length - 4L) / 2) {
            throw new IllegalArgumentException("PID_DATA_REPRESENTATION lists "
                    + count + " ids in " + (v.length - 4) + " octets");
        }
        int[] ids = new int[(int) count];
        for (int i = 0; i < count; i++) {
            ids[i] = (short) Wire.u16le(v, 4 + 2 * i);
        }
        return ids;
    }

    // ======================================================================
    // Request/offered compatibility (DDS 1.4 §2.2.3, "RxO")
    // ======================================================================

    /**
     * The name of the first policy for which {@code offered} (a writer's
     * announcement) does not satisfy {@code requested} (a reader's), or
     * {@code null} if the two are compatible: the request-offered rules of
     * DDS 1.4 §2.2.3 for DURABILITY, DEADLINE, LATENCY_BUDGET, LIVELINESS,
     * RELIABILITY, OWNERSHIP, DESTINATION_ORDER and PRESENTATION, and
     * PARTITION (§2.2.3.13: the names must overlap, or both be default).
     * A policy either side does not state is its default and compatible
     * with the other's.
     */
    public static String incompatibility(QosParameters offered, QosParameters requested) {
        if (offered == null || requested == null) {
            return null;
        }
        if (offered.reliability != null && requested.reliability != null
                && offered.reliability.kind < requested.reliability.kind) {
            return "RELIABILITY";
        }
        if (offered.durability != null && requested.durability != null
                && offered.durability < requested.durability) {
            return "DURABILITY";
        }
        if (offered.deadline != null && requested.deadline != null
                && offered.deadline.compareTo(requested.deadline) > 0) {
            return "DEADLINE";
        }
        if (offered.latencyBudget != null && requested.latencyBudget != null
                && offered.latencyBudget.compareTo(requested.latencyBudget) > 0) {
            return "LATENCY_BUDGET";
        }
        if (offered.liveliness != null && requested.liveliness != null
                && (offered.liveliness.kind < requested.liveliness.kind
                    || offered.liveliness.lease.compareTo(requested.liveliness.lease) > 0)) {
            return "LIVELINESS";
        }
        if (offered.ownership != null && requested.ownership != null
                && !offered.ownership.equals(requested.ownership)) {
            return "OWNERSHIP";
        }
        if (offered.destinationOrder != null && requested.destinationOrder != null
                && offered.destinationOrder < requested.destinationOrder) {
            return "DESTINATION_ORDER";
        }
        if (offered.presentation != null && requested.presentation != null) {
            Presentation o = offered.presentation;
            Presentation r = requested.presentation;
            if (o.accessScope < r.accessScope
                    || (r.coherentAccess && !o.coherentAccess)
                    || (r.orderedAccess && !o.orderedAccess)) {
                return "PRESENTATION";
            }
        }
        if (!partitionsOverlap(offered.partition, requested.partition)) {
            return "PARTITION";
        }
        return null;
    }

    /**
     * §2.2.3.13: the default partition is the single name ""; a name may be a
     * pattern with {@code *} and {@code ?} (fnmatch), and two sets overlap when
     * a name of one matches a name of the other.
     */
    public static boolean partitionsOverlap(List<String> a, List<String> b) {
        List<String> left = a == null || a.isEmpty() ? Collections.singletonList("") : a;
        List<String> right = b == null || b.isEmpty() ? Collections.singletonList("") : b;
        for (String x : left) {
            for (String y : right) {
                if (glob(x, y) || glob(y, x)) {
                    return true;
                }
            }
        }
        return false;
    }

    /** Whether {@code text} matches the fnmatch {@code pattern}. */
    private static boolean glob(String pattern, String text) {
        int p = 0;
        int t = 0;
        int star = -1;
        int mark = 0;
        while (t < text.length()) {
            if (p < pattern.length()
                    && (pattern.charAt(p) == '?' || pattern.charAt(p) == text.charAt(t))) {
                p++;
                t++;
            } else if (p < pattern.length() && pattern.charAt(p) == '*') {
                star = p++;
                mark = t;
            } else if (star >= 0) {
                p = star + 1;
                t = ++mark;
            } else {
                return false;
            }
        }
        while (p < pattern.length() && pattern.charAt(p) == '*') {
            p++;
        }
        return p == pattern.length();
    }
}
