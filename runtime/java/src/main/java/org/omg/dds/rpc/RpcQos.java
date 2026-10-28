// SPDX-License-Identifier: Apache-2.0
package org.omg.dds.rpc;

import java.io.File;
import java.io.IOException;
import java.util.Objects;
import javax.xml.XMLConstants;
import javax.xml.parsers.DocumentBuilder;
import javax.xml.parsers.DocumentBuilderFactory;
import javax.xml.parsers.ParserConfigurationException;
import org.w3c.dom.Document;
import org.w3c.dom.Element;
import org.w3c.dom.Node;
import org.w3c.dom.NodeList;
import org.xml.sax.SAXException;

/**
 * The QoS of the DataWriters and DataReaders of a requester or a replier —
 * DDS-RPC 1.0 §7.10.
 *
 * <p>§7.10.2: "When no QoS is specified, default QoS are in effect",
 * RELIABLE reliability, KEEP_ALL history and VOLATILE durability, and "the
 * users may modify the mutable QoS policies at run-time"
 * ({@link Requester#setQos}, {@link Replier#setQos}). §7.10.1 names XML QoS
 * profiles as the other source: {@link #fromUrl}.
 *
 * <p>Immutable; the {@code with…} methods return a changed copy.
 */
public final class RpcQos {

    /** DDS reliability kind. */
    public enum Reliability {
        BEST_EFFORT,
        RELIABLE
    }

    /** DDS history kind. */
    public enum HistoryKind {
        KEEP_LAST,
        KEEP_ALL
    }

    /** DDS durability kind, in the order of the DDS 1.4 compatibility ladder. */
    public enum Durability {
        VOLATILE,
        TRANSIENT_LOCAL,
        TRANSIENT,
        PERSISTENT
    }

    private final Reliability reliability;
    private final HistoryKind historyKind;
    private final int historyDepth;
    private final Durability durability;
    private final long deadlineNanos;
    private final long lifespanNanos;
    private final WireEncoding dataRepresentation;

    private RpcQos(
            Reliability reliability,
            HistoryKind historyKind,
            int historyDepth,
            Durability durability,
            long deadlineNanos,
            long lifespanNanos,
            WireEncoding dataRepresentation) {
        this.reliability = reliability;
        this.historyKind = historyKind;
        this.historyDepth = historyDepth;
        this.durability = durability;
        this.deadlineNanos = deadlineNanos;
        this.lifespanNanos = lifespanNanos;
        this.dataRepresentation = dataRepresentation;
    }

    /** §7.10.2: RELIABLE, KEEP_ALL, VOLATILE; deadline and lifespan infinite. */
    public static RpcQos defaults() {
        return new RpcQos(Reliability.RELIABLE, HistoryKind.KEEP_ALL, 1, Durability.VOLATILE, 0L, 0L,
                WireEncoding.XCDR1);
    }

    public Reliability reliability() {
        return reliability;
    }

    public HistoryKind historyKind() {
        return historyKind;
    }

    /** The KEEP_LAST depth; meaningless for KEEP_ALL. */
    public int historyDepth() {
        return historyDepth;
    }

    public Durability durability() {
        return durability;
    }

    /** The deadline period in nanoseconds, {@code 0} for infinite. */
    public long deadlineNanos() {
        return deadlineNanos;
    }

    /** The lifespan in nanoseconds, {@code 0} for infinite. */
    public long lifespanNanos() {
        return lifespanNanos;
    }

    /**
     * The DATA_REPRESENTATION the endpoints offer or accept: classic CDR
     * unless the params ask for XCDR2 (see {@link WireEncoding}).
     */
    public WireEncoding dataRepresentation() {
        return dataRepresentation;
    }

    public RpcQos withDataRepresentation(WireEncoding value) {
        return new RpcQos(
                reliability, historyKind, historyDepth, durability, deadlineNanos, lifespanNanos,
                Objects.requireNonNull(value));
    }

    public RpcQos withReliability(Reliability value) {
        return new RpcQos(
                Objects.requireNonNull(value), historyKind, historyDepth, durability,
                deadlineNanos, lifespanNanos, dataRepresentation);
    }

    public RpcQos withHistory(HistoryKind kind, int depth) {
        if (depth < 1) {
            throw new IllegalArgumentException("a history depth is at least 1");
        }
        return new RpcQos(
                reliability, Objects.requireNonNull(kind), depth, durability,
                deadlineNanos, lifespanNanos, dataRepresentation);
    }

    public RpcQos withDurability(Durability value) {
        return new RpcQos(
                reliability, historyKind, historyDepth, Objects.requireNonNull(value),
                deadlineNanos, lifespanNanos, dataRepresentation);
    }

    public RpcQos withDeadlineNanos(long nanos) {
        if (nanos < 0) {
            throw new IllegalArgumentException("a deadline is not negative");
        }
        return new RpcQos(reliability, historyKind, historyDepth, durability, nanos, lifespanNanos, dataRepresentation);
    }

    public RpcQos withLifespanNanos(long nanos) {
        if (nanos < 0) {
            throw new IllegalArgumentException("a lifespan is not negative");
        }
        return new RpcQos(reliability, historyKind, historyDepth, durability, deadlineNanos, nanos, dataRepresentation);
    }

    /**
     * Whether {@code to} differs from this QoS only in policies DDS 1.4
     * lets change after the entity is enabled. RELIABILITY, DURABILITY and
     * HISTORY are immutable; DEADLINE is mutable. LIFESPAN is a
     * writer-only policy and is mutable.
     *
     * @param to the QoS an endpoint is to take
     * @return whether the change is allowed
     */
    public boolean mutableChangeTo(RpcQos to) {
        return reliability == to.reliability
                && historyKind == to.historyKind
                && (historyKind == HistoryKind.KEEP_ALL || historyDepth == to.historyDepth)
                && durability == to.durability
                && dataRepresentation == to.dataRepresentation;
    }

    @Override
    public boolean equals(Object other) {
        if (!(other instanceof RpcQos)) {
            return false;
        }
        RpcQos that = (RpcQos) other;
        return reliability == that.reliability
                && historyKind == that.historyKind
                && historyDepth == that.historyDepth
                && durability == that.durability
                && deadlineNanos == that.deadlineNanos
                && lifespanNanos == that.lifespanNanos
                && dataRepresentation == that.dataRepresentation;
    }

    @Override
    public int hashCode() {
        return Objects.hash(
                reliability, historyKind, historyDepth, durability, deadlineNanos, lifespanNanos, dataRepresentation);
    }

    @Override
    public String toString() {
        return "RpcQos(" + reliability + ", " + historyKind + "(" + historyDepth + "), "
                + durability + ", deadline " + deadlineNanos + " ns, lifespan " + lifespanNanos
                + " ns)";
    }

    // -- §7.10.1: the profile URLs ------------------------------------

    /**
     * Resolves a {@code @Qos} profile URL — DDS-RPC 1.0 §7.10.1.
     *
     * <p>"The profile attributes are string URLs of the form
     * {@code <protocol>://path/resource} where protocol, ':', '//' and
     * path are optional. Support for the file:// protocol is mandatory. If
     * the protocol:// is omitted, the string attributes are interpreted as
     * if the protocol is file://. The URL may end with an optional library
     * name. The profile filename and library_name are separated by a #."
     * The file holds XML QoS profiles of [DDS4CCM]; the named library (or
     * the first) supplies its first profile, and the profile's
     * {@code datawriter_qos} and {@code datareader_qos} are laid over the
     * defaults of §7.10.2.
     *
     * @param url the profile URL
     * @return the QoS the profile describes
     * @throws IllegalArgumentException for a protocol other than
     *     {@code file}, a file that cannot be read or parsed, or a missing
     *     library or profile — none of them is replaced by the defaults
     */
    public static RpcQos fromUrl(String url) {
        String rest;
        int protocolEnd = url.indexOf("://");
        if (protocolEnd >= 0) {
            String protocol = url.substring(0, protocolEnd);
            if (!"file".equals(protocol)) {
                throw new IllegalArgumentException(
                        "QoS profile protocol '" + protocol + "' is not supported, only file");
            }
            rest = url.substring(protocolEnd + 3);
        } else if (url.startsWith("file:")) {
            rest = url.substring("file:".length());
        } else {
            rest = url;
        }
        String library = null;
        int hash = rest.indexOf('#');
        if (hash >= 0) {
            library = rest.substring(hash + 1);
            rest = rest.substring(0, hash);
            if (library.isEmpty()) {
                library = null;
            }
        }
        if (rest.isEmpty()) {
            throw new IllegalArgumentException("QoS profile URL '" + url + "' has no path");
        }
        Document document = parse(new File(rest), url);
        Element profile = findProfile(document, library, url);
        RpcQos qos = defaults();
        Element writer = child(profile, "datawriter_qos");
        if (writer != null) {
            qos = qos.overlay(writer, true);
        }
        Element reader = child(profile, "datareader_qos");
        if (reader != null) {
            qos = qos.overlay(reader, false);
        }
        return qos;
    }

    private static Document parse(File file, String url) {
        try {
            DocumentBuilderFactory factory = DocumentBuilderFactory.newInstance();
            factory.setFeature(XMLConstants.FEATURE_SECURE_PROCESSING, true);
            factory.setFeature("http://apache.org/xml/features/disallow-doctype-decl", true);
            DocumentBuilder builder = factory.newDocumentBuilder();
            return builder.parse(file);
        } catch (ParserConfigurationException | SAXException | IOException failure) {
            throw new IllegalArgumentException(
                    "QoS profile '" + url + "' cannot be read: " + failure.getMessage(), failure);
        }
    }

    private static Element findProfile(Document document, String library, String url) {
        NodeList libraries = document.getElementsByTagName("qos_library");
        for (int at = 0; at < libraries.getLength(); ++at) {
            Element candidate = (Element) libraries.item(at);
            if (library != null && !library.equals(candidate.getAttribute("name"))) {
                continue;
            }
            Element profile = child(candidate, "qos_profile");
            if (profile == null) {
                throw new IllegalArgumentException(
                        "QoS library in '" + url + "' has no qos_profile");
            }
            return profile;
        }
        throw new IllegalArgumentException(
                "QoS profile '" + url + "' has no library " + (library == null ? "" : library));
    }

    private static Element child(Element parent, String name) {
        for (Node node = parent.getFirstChild(); node != null; node = node.getNextSibling()) {
            if (node instanceof Element && name.equals(node.getNodeName())) {
                return (Element) node;
            }
        }
        return null;
    }

    private static String text(Element parent, String name) {
        Element found = child(parent, name);
        return found == null ? null : found.getTextContent().trim();
    }

    private RpcQos overlay(Element entityQos, boolean writer) {
        RpcQos out = this;
        Element policy = child(entityQos, "reliability");
        if (policy != null) {
            String kind = text(policy, "kind");
            if (kind != null) {
                out = out.withReliability(parseReliability(kind));
            }
        }
        policy = child(entityQos, "durability");
        if (policy != null) {
            String kind = text(policy, "kind");
            if (kind != null) {
                out = out.withDurability(parseDurability(kind));
            }
        }
        policy = child(entityQos, "history");
        if (policy != null) {
            String kind = text(policy, "kind");
            HistoryKind parsed = kind == null ? out.historyKind : parseHistory(kind);
            String depth = text(policy, "depth");
            out = out.withHistory(parsed, depth == null ? out.historyDepth : Integer.parseInt(depth));
        }
        policy = child(entityQos, "deadline");
        if (policy != null) {
            out = out.withDeadlineNanos(period(policy, out.deadlineNanos));
        }
        policy = child(entityQos, "lifespan");
        if (writer && policy != null) {
            out = out.withLifespanNanos(period(policy, out.lifespanNanos));
        }
        return out;
    }

    private static long period(Element policy, long fallback) {
        Element duration = child(policy, "period");
        if (duration == null) {
            duration = child(policy, "duration");
        }
        if (duration == null) {
            return fallback;
        }
        String sec = text(duration, "sec");
        String nanosec = text(duration, "nanosec");
        if ("DURATION_INFINITY".equals(sec) || "DURATION_INFINITE_SEC".equals(sec)) {
            return 0L;
        }
        long seconds = sec == null ? 0L : Long.parseLong(sec);
        long nanos = nanosec == null ? 0L : Long.parseLong(nanosec);
        return seconds * 1_000_000_000L + nanos;
    }

    private static Reliability parseReliability(String kind) {
        switch (kind) {
            case "RELIABLE_RELIABILITY_QOS":
                return Reliability.RELIABLE;
            case "BEST_EFFORT_RELIABILITY_QOS":
                return Reliability.BEST_EFFORT;
            default:
                throw new IllegalArgumentException("unknown reliability kind " + kind);
        }
    }

    private static HistoryKind parseHistory(String kind) {
        switch (kind) {
            case "KEEP_ALL_HISTORY_QOS":
                return HistoryKind.KEEP_ALL;
            case "KEEP_LAST_HISTORY_QOS":
                return HistoryKind.KEEP_LAST;
            default:
                throw new IllegalArgumentException("unknown history kind " + kind);
        }
    }

    private static Durability parseDurability(String kind) {
        switch (kind) {
            case "VOLATILE_DURABILITY_QOS":
                return Durability.VOLATILE;
            case "TRANSIENT_LOCAL_DURABILITY_QOS":
                return Durability.TRANSIENT_LOCAL;
            case "TRANSIENT_DURABILITY_QOS":
                return Durability.TRANSIENT;
            case "PERSISTENT_DURABILITY_QOS":
                return Durability.PERSISTENT;
            default:
                throw new IllegalArgumentException("unknown durability kind " + kind);
        }
    }
}
