// SPDX-License-Identifier: Apache-2.0
package org.zerodds.core.qos;

import org.omg.dds.core.Duration;
import org.omg.dds.core.EntityQos;
import org.omg.dds.core.QosProvider;
import org.omg.dds.core.ServiceEnvironment;
import org.omg.dds.core.policy.Deadline;
import org.omg.dds.core.policy.DestinationOrder;
import org.omg.dds.core.policy.Durability;
import org.omg.dds.core.policy.DurabilityService;
import org.omg.dds.core.policy.EntityFactory;
import org.omg.dds.core.policy.GroupData;
import org.omg.dds.core.policy.History;
import org.omg.dds.core.policy.LatencyBudget;
import org.omg.dds.core.policy.Lifespan;
import org.omg.dds.core.policy.Liveliness;
import org.omg.dds.core.policy.Ownership;
import org.omg.dds.core.policy.OwnershipStrength;
import org.omg.dds.core.policy.Partition;
import org.omg.dds.core.policy.Presentation;
import org.omg.dds.core.policy.QosPolicy;
import org.omg.dds.core.policy.ReaderDataLifecycle;
import org.omg.dds.core.policy.Reliability;
import org.omg.dds.core.policy.ResourceLimits;
import org.omg.dds.core.policy.TimeBasedFilter;
import org.omg.dds.core.policy.TopicData;
import org.omg.dds.core.policy.TransportPriority;
import org.omg.dds.core.policy.UserData;
import org.omg.dds.core.policy.WriterDataLifecycle;
import org.omg.dds.domain.DomainParticipantFactoryQos;
import org.omg.dds.domain.DomainParticipantQos;
import org.omg.dds.pub.DataWriterQos;
import org.omg.dds.pub.PublisherQos;
import org.omg.dds.sub.DataReaderQos;
import org.omg.dds.sub.SubscriberQos;
import org.omg.dds.topic.TopicQos;
import org.w3c.dom.Document;
import org.w3c.dom.Element;
import org.w3c.dom.Node;
import org.w3c.dom.NodeList;
import org.xml.sax.SAXException;

import javax.xml.XMLConstants;
import javax.xml.parsers.DocumentBuilder;
import javax.xml.parsers.DocumentBuilderFactory;
import javax.xml.parsers.ParserConfigurationException;
import java.io.File;
import java.io.IOException;
import java.net.URI;
import java.net.URISyntaxException;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Base64;
import java.util.EnumMap;
import java.util.IdentityHashMap;
import java.util.LinkedHashMap;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;
import java.util.concurrent.TimeUnit;

/**
 * ZeroDDS {@link QosProvider} (DDS-Java-PSM 1.0 §7.3.5.4): the entity QoS of the profiles of
 * an XML QoS library in the syntax of DDS-XML 1.0 §7.3.2 ({@code dds-xml_qos_definitions.xsd}).
 *
 * <p>The document is the one the DDS-XML schema describes:
 * <pre>
 * &lt;dds&gt;&lt;qos_library name="L"&gt;
 *   &lt;qos_profile name="P" base_name="Q"&gt;
 *     &lt;datawriter_qos name="w"&gt;
 *       &lt;reliability&gt;&lt;kind&gt;RELIABLE_RELIABILITY_QOS&lt;/kind&gt;&lt;/reliability&gt;
 * </pre>
 * with the enumeration literals of the schema ({@code KEEP_LAST_HISTORY_QOS}), durations as
 * {@code <sec>} and {@code <nanosec>} ({@code DURATION_INFINITY} for infinite), counts as numbers or
 * {@code LENGTH_UNLIMITED}, octet sequences as base64 and a partition as {@code <name><element>}s.
 * A policy or a field the document leaves out keeps the default of the entity (DDS 1.4 §2.2.3).
 *
 * <p>Inheritance follows DDS-XML 1.0 §7.3.2.4.2: a profile's {@code base_name} names a profile
 * defined before it (as {@code profile} or {@code library::profile}), the profile starts as that
 * profile and each QoS element of it replaces the field values of the element of the same kind
 * and {@code name}; the {@code base_name} of a QoS element names an element of the same kind in
 * the profile (as {@code name}) or in another one ({@code library::profile::name}) whose fields it
 * takes before its own. A cycle, a base defined later or a base that does not exist is an
 * {@link IllegalArgumentException}, as is any unknown element, literal or malformed value:
 * nothing in a document is ignored without a message.
 *
 * <p>The source is a URI. §7.3.5.4 requires {@code file://}, which this class implements; any
 * other scheme raises {@link UnsupportedOperationException}, the outcome §2 of this
 * specification allows an implementation without further XML profile support to choose.
 *
 * <p>The profile named in {@link #QosProviderImpl} is the one the operations without an {@code id}
 * answer from, with the first QoS element of the kind in it. {@code get...Qos(id)} finds the
 * element {@code name}d {@code id} in that profile, or the profile {@code library::profile} (its
 * first element of the kind), or the element {@code library::profile::name}, or the unique
 * profile of that name in the document.
 */
public final class QosProviderImpl extends QosProvider {

    /** The seven QoS elements of a profile. */
    private enum Kind {
        FACTORY, PARTICIPANT, TOPIC, SUBSCRIBER, PUBLISHER, READER, WRITER
    }

    /** A {@code <qos_profile>} of the document. */
    private static final class ProfileDef {
        final String library;
        final String name;
        final String baseName;
        final List<Element> elements;
        final int order;

        ProfileDef(String library, String name, String baseName, List<Element> elements, int order) {
            this.library = library;
            this.name = name;
            this.baseName = baseName;
            this.elements = elements;
            this.order = order;
        }

        String qualifiedName() {
            return library + "::" + name;
        }
    }

    /** The fields of one QoS element after inheritance: "policy/field" to the field's element. */
    private static final class Fields extends LinkedHashMap<String, Element> {
        private static final long serialVersionUID = 1L;
    }

    /** The QoS elements of a profile after inheritance, by kind and name. */
    private static final class Table extends EnumMap<Kind, Map<String, Fields>> {
        private static final long serialVersionUID = 1L;

        Table() {
            super(Kind.class);
            for (Kind kind : Kind.values()) {
                put(kind, new LinkedHashMap<String, Fields>());
            }
        }

        Table copy() {
            Table copy = new Table();
            for (Map.Entry<Kind, Map<String, Fields>> kind : entrySet()) {
                for (Map.Entry<String, Fields> element : kind.getValue().entrySet()) {
                    Fields fields = new Fields();
                    fields.putAll(element.getValue());
                    copy.get(kind.getKey()).put(element.getKey(), fields);
                }
            }
            return copy;
        }
    }

    private static final int MAX_INHERITANCE_DEPTH = 32;

    private final ServiceEnvironment environment;
    private final String uri;
    private final List<ProfileDef> profiles = new ArrayList<>();
    private final ProfileDef selected;
    private final Map<ProfileDef, Map<Kind, Map<String, Object>>> built = new IdentityHashMap<>();

    /**
     * Reads the document at {@code uri} and selects {@code profile}.
     *
     * @param profile  {@code library::profile}, or the name of a profile that is unique in the
     *                 document, or {@code null} for the first profile.
     * @throws UnsupportedOperationException  if the scheme of {@code uri} is not {@code file}.
     * @throws IllegalArgumentException       if the URI or the document is malformed, the file
     *         cannot be read, or the profile does not exist or is not unique.
     */
    public QosProviderImpl(ServiceEnvironment environment, String uri, String profile) {
        this.environment = environment;
        this.uri = uri;
        readLibraries(read(uri));
        if (profiles.isEmpty()) {
            throw new IllegalArgumentException("no QoS profile in " + uri);
        }
        if (profile == null) {
            this.selected = profiles.get(0);
        } else {
            ProfileDef found = findProfile(profile, null);
            if (found == null) {
                throw new IllegalArgumentException(
                        "no QoS profile named '" + profile + "' in " + uri);
            }
            this.selected = found;
        }
        // The selected profile is resolved now, so a document that cannot be resolved is
        // refused here and not at the first get.
        resolved(selected);
    }

    @Override
    public ServiceEnvironment getEnvironment() {
        return environment;
    }

    @Override
    public DomainParticipantFactoryQos getDomainParticipantFactoryQos() {
        return first(Kind.FACTORY);
    }

    @Override
    public DomainParticipantFactoryQos getDomainParticipantFactoryQos(String id) {
        return byId(Kind.FACTORY, id);
    }

    @Override
    public DomainParticipantQos getDomainParticipantQos() {
        return first(Kind.PARTICIPANT);
    }

    @Override
    public DomainParticipantQos getDomainParticipantQos(String id) {
        return byId(Kind.PARTICIPANT, id);
    }

    @Override
    public TopicQos getTopicQos() {
        return first(Kind.TOPIC);
    }

    @Override
    public TopicQos getTopicQos(String id) {
        return byId(Kind.TOPIC, id);
    }

    @Override
    public SubscriberQos getSubscriberQos() {
        return first(Kind.SUBSCRIBER);
    }

    @Override
    public SubscriberQos getSubscriberQos(String id) {
        return byId(Kind.SUBSCRIBER, id);
    }

    @Override
    public PublisherQos getPublisherQos() {
        return first(Kind.PUBLISHER);
    }

    @Override
    public PublisherQos getPublisherQos(String id) {
        return byId(Kind.PUBLISHER, id);
    }

    @Override
    public DataReaderQos getDataReaderQos() {
        return first(Kind.READER);
    }

    @Override
    public DataReaderQos getDataReaderQos(String id) {
        return byId(Kind.READER, id);
    }

    @Override
    public DataWriterQos getDataWriterQos() {
        return first(Kind.WRITER);
    }

    @Override
    public DataWriterQos getDataWriterQos(String id) {
        return byId(Kind.WRITER, id);
    }

    @SuppressWarnings("unchecked")
    private <Q> Q first(Kind kind) {
        Map<String, Object> elements = resolved(selected).get(kind);
        if (elements.isEmpty()) {
            throw new IllegalArgumentException("the QoS profile " + selected.qualifiedName()
                    + " has no " + tagOf(kind) + " element");
        }
        return (Q) elements.values().iterator().next();
    }

    @SuppressWarnings("unchecked")
    private <Q> Q byId(Kind kind, String id) {
        if (id == null) {
            throw new IllegalArgumentException("the id of a QoS must not be null");
        }
        Object direct = resolved(selected).get(kind).get(id);
        if (direct != null) {
            return (Q) direct;
        }
        String[] parts = id.split("::", -1);
        ProfileDef profile;
        String name = null;
        if (parts.length == 3) {
            profile = findProfile(parts[0] + "::" + parts[1], null);
            name = parts[2];
        } else {
            profile = findProfile(id, null);
        }
        if (profile != null) {
            Map<String, Object> elements = resolved(profile).get(kind);
            Object found = name == null
                    ? (elements.isEmpty() ? null : elements.values().iterator().next())
                    : elements.get(name);
            if (found != null) {
                return (Q) found;
            }
        }
        throw new IllegalArgumentException("no " + tagOf(kind) + " '" + id + "' in " + uri);
    }

    private static String tagOf(Kind kind) {
        switch (kind) {
            case FACTORY:
                return "domain_participant_factory_qos";
            case PARTICIPANT:
                return "domain_participant_qos";
            case TOPIC:
                return "topic_qos";
            case SUBSCRIBER:
                return "subscriber_qos";
            case PUBLISHER:
                return "publisher_qos";
            case READER:
                return "datareader_qos";
            default:
                return "datawriter_qos";
        }
    }

    /** The kind a profile child element is, or {@code null} for an element that is none. */
    private static Kind kindOf(String tag) {
        switch (tag) {
            case "domain_participant_factory_qos":
            case "domainparticipantfactory_qos":
                return Kind.FACTORY;
            case "domain_participant_qos":
            case "domainparticipant_qos":
            case "participant_qos":
                return Kind.PARTICIPANT;
            case "topic_qos":
                return Kind.TOPIC;
            case "subscriber_qos":
                return Kind.SUBSCRIBER;
            case "publisher_qos":
                return Kind.PUBLISHER;
            case "datareader_qos":
                return Kind.READER;
            case "datawriter_qos":
                return Kind.WRITER;
            default:
                return null;
        }
    }

    // --- Reading the document ------------------------------------------------

    private void readLibraries(Document document) {
        List<Element> libraries = new ArrayList<>();
        Element root = document.getDocumentElement();
        if ("qos_library".equals(root.getTagName())) {
            libraries.add(root);
        } else {
            collect(root, "qos_library", libraries);
        }
        for (Element library : libraries) {
            String libraryName = library.getAttribute("name");
            for (Element profile : children(library)) {
                if (!"qos_profile".equals(profile.getTagName())) {
                    throw new IllegalArgumentException("unexpected element <" + profile.getTagName()
                            + "> in the qos_library '" + libraryName + "'");
                }
                String name = profile.getAttribute("name");
                if (name.isEmpty()) {
                    throw new IllegalArgumentException(
                            "a qos_profile of '" + libraryName + "' has no name");
                }
                ProfileDef def = new ProfileDef(libraryName, name, profile.getAttribute("base_name"),
                        children(profile), profiles.size());
                for (ProfileDef other : profiles) {
                    if (other.library.equals(libraryName) && other.name.equals(name)) {
                        throw new IllegalArgumentException("the qos_profile '"
                                + def.qualifiedName() + "' is defined twice in " + uri);
                    }
                }
                profiles.add(def);
            }
        }
    }

    private static void collect(Element element, String tag, List<Element> into) {
        for (Element child : children(element)) {
            if (tag.equals(child.getTagName())) {
                into.add(child);
            } else {
                collect(child, tag, into);
            }
        }
    }

    private static List<Element> children(Element element) {
        List<Element> elements = new ArrayList<>();
        NodeList nodes = element.getChildNodes();
        for (int i = 0; i < nodes.getLength(); i++) {
            Node node = nodes.item(i);
            if (node instanceof Element) {
                elements.add((Element) node);
            }
        }
        return elements;
    }

    private static Document read(String uri) {
        if (uri == null) {
            throw new IllegalArgumentException("a QoS library URI must not be null");
        }
        URI parsed;
        try {
            parsed = new URI(uri);
        } catch (URISyntaxException e) {
            throw new IllegalArgumentException("not a URI: " + uri, e);
        }
        String scheme = parsed.getScheme();
        if (!"file".equals(scheme)) {
            throw new UnsupportedOperationException(
                    "this QoS library URI scheme '" + scheme + "' is not supported; "
                            + "only file:// QoS libraries are");
        }
        Path path;
        try {
            path = java.nio.file.Paths.get(parsed);
        } catch (RuntimeException e) {
            throw new IllegalArgumentException("not a file URI: " + uri, e);
        }
        try {
            DocumentBuilderFactory factory = DocumentBuilderFactory.newInstance();
            // A QoS library has no document type; refusing one closes external entities.
            factory.setFeature("http://apache.org/xml/features/disallow-doctype-decl", true);
            factory.setAttribute(XMLConstants.ACCESS_EXTERNAL_DTD, "");
            factory.setAttribute(XMLConstants.ACCESS_EXTERNAL_SCHEMA, "");
            factory.setNamespaceAware(false);
            DocumentBuilder builder = factory.newDocumentBuilder();
            File file = path.toFile();
            if (!file.isFile()) {
                throw new IOException("no such file: " + path);
            }
            return builder.parse(file);
        } catch (ParserConfigurationException | SAXException | IOException e) {
            throw new IllegalArgumentException("cannot read the QoS library " + uri
                    + ": " + e.getMessage(), e);
        }
    }

    // --- Profiles -------------------------------------------------------------

    /**
     * The profile {@code ref} names: {@code library::profile}, or a profile name that is unique
     * in the document (or in {@code inLibrary} if given and unique there).
     *
     * @return the profile, or {@code null} when there is none.
     * @throws IllegalArgumentException  if the name matches more than one profile.
     */
    private ProfileDef findProfile(String ref, String inLibrary) {
        int separator = ref.indexOf("::");
        if (separator >= 0) {
            String library = ref.substring(0, separator);
            String name = ref.substring(separator + 2);
            for (ProfileDef candidate : profiles) {
                if (candidate.library.equals(library) && candidate.name.equals(name)) {
                    return candidate;
                }
            }
            return null;
        }
        List<ProfileDef> named = new ArrayList<>();
        for (ProfileDef candidate : profiles) {
            if (candidate.name.equals(ref) && (inLibrary == null || candidate.library.equals(inLibrary))) {
                named.add(candidate);
            }
        }
        if (named.isEmpty() && inLibrary != null) {
            return findProfile(ref, null);
        }
        if (named.size() > 1) {
            throw new IllegalArgumentException("the QoS profile name '" + ref
                    + "' is not unique in " + uri + "; name it library::profile");
        }
        return named.isEmpty() ? null : named.get(0);
    }

    private Map<Kind, Map<String, Object>> resolved(ProfileDef profile) {
        Map<Kind, Map<String, Object>> typed = built.get(profile);
        if (typed == null) {
            Table table = tableOf(profile, new ArrayList<ProfileDef>());
            typed = new EnumMap<>(Kind.class);
            for (Kind kind : Kind.values()) {
                Map<String, Object> elements = new LinkedHashMap<>();
                for (Map.Entry<String, Fields> element : table.get(kind).entrySet()) {
                    elements.put(element.getKey(), build(kind, element.getValue()));
                }
                typed.put(kind, elements);
            }
            built.put(profile, typed);
        }
        return typed;
    }

    /** The QoS elements of {@code profile} with those of its base profiles applied first. */
    private Table tableOf(ProfileDef profile, List<ProfileDef> path) {
        if (path.contains(profile)) {
            throw new IllegalArgumentException("the QoS profile " + profile.qualifiedName()
                    + " inherits from itself");
        }
        if (path.size() >= MAX_INHERITANCE_DEPTH) {
            throw new IllegalArgumentException("the QoS profile inheritance is deeper than "
                    + MAX_INHERITANCE_DEPTH + " at " + profile.qualifiedName());
        }
        path.add(profile);
        Table table;
        if (profile.baseName.isEmpty()) {
            table = new Table();
        } else {
            ProfileDef base = findProfile(profile.baseName, profile.library);
            if (base == null) {
                throw new IllegalArgumentException("the base profile '" + profile.baseName
                        + "' of " + profile.qualifiedName() + " does not exist");
            }
            if (base.order >= profile.order) {
                throw new IllegalArgumentException("the profile " + profile.qualifiedName()
                        + " inherits from " + base.qualifiedName()
                        + ", which is not defined before it");
            }
            table = tableOf(base, path).copy();
        }
        for (Element element : profile.elements) {
            Kind kind = kindOf(element.getTagName());
            if (kind == null) {
                if ("topic_filter".equals(element.getTagName())) {
                    continue; // a vendor element the Java API has no place for: not a QoS
                }
                throw new IllegalArgumentException("unknown element <" + element.getTagName()
                        + "> in the profile " + profile.qualifiedName());
            }
            String name = element.getAttribute("name");
            Fields fields = new Fields();
            String baseName = element.getAttribute("base_name");
            if (!baseName.isEmpty()) {
                fields.putAll(elementBase(kind, baseName, table, profile));
            } else if (table.get(kind).containsKey(name)) {
                fields.putAll(table.get(kind).get(name));
            }
            for (Element policy : children(element)) {
                List<Element> values = children(policy);
                if (values.isEmpty()) {
                    fields.put(policy.getTagName() + "/", policy);
                }
                for (Element value : values) {
                    fields.put(policy.getTagName() + "/" + value.getTagName(), value);
                }
            }
            table.get(kind).put(name, fields);
        }
        path.remove(path.size() - 1);
        return table;
    }

    /** The fields of the QoS element a {@code base_name} refers to. */
    private Fields elementBase(Kind kind, String baseName, Table table, ProfileDef profile) {
        String[] parts = baseName.split("::", -1);
        Fields found;
        if (parts.length == 1) {
            found = table.get(kind).get(baseName);
        } else if (parts.length == 3) {
            ProfileDef other = findProfile(parts[0] + "::" + parts[1], null);
            if (other == null || other.order > profile.order) {
                throw new IllegalArgumentException("the base_name '" + baseName
                        + "' names no profile defined before " + profile.qualifiedName());
            }
            found = other == profile ? table.get(kind).get(parts[2])
                    : tableOf(other, new ArrayList<ProfileDef>()).get(kind).get(parts[2]);
        } else {
            throw new IllegalArgumentException("the base_name '" + baseName
                    + "' is neither a name nor library::profile::name");
        }
        if (found == null) {
            throw new IllegalArgumentException("the base_name '" + baseName + "' of a "
                    + tagOf(kind) + " in " + profile.qualifiedName() + " names no such element");
        }
        return found;
    }

    // --- From fields to QoS ----------------------------------------------------

    private Object build(Kind kind, Fields fields) {
        Object qos;
        switch (kind) {
            case FACTORY:
                qos = DomainParticipantFactoryQosImpl.defaults(environment);
                break;
            case PARTICIPANT:
                qos = DomainParticipantQosImpl.defaults(environment);
                break;
            case TOPIC:
                qos = TopicQosImpl.defaults(environment);
                break;
            case SUBSCRIBER:
                qos = SubscriberQosImpl.defaults(environment);
                break;
            case PUBLISHER:
                qos = PublisherQosImpl.defaults(environment);
                break;
            case READER:
                qos = DataReaderQosImpl.defaults(environment);
                break;
            default:
                qos = DataWriterQosImpl.defaults(environment);
                break;
        }
        Map<String, Map<String, Element>> byPolicy = new LinkedHashMap<>();
        for (Map.Entry<String, Element> field : fields.entrySet()) {
            String key = field.getKey();
            String policy = key.substring(0, key.indexOf('/'));
            String name = key.substring(key.indexOf('/') + 1);
            Map<String, Element> values = byPolicy.get(policy);
            if (values == null) {
                values = new LinkedHashMap<>();
                byPolicy.put(policy, values);
            }
            if (!name.isEmpty()) {
                values.put(name, field.getValue());
            }
        }
        for (Map.Entry<String, Map<String, Element>> policy : byPolicy.entrySet()) {
            Class<? extends QosPolicy> type = policyType(policy.getKey());
            Object current = ((Map<?, ?>) qos).get(type);
            if (current == null) {
                throw new IllegalArgumentException("the policy <" + policy.getKey()
                        + "> does not apply to a " + tagOf(kind));
            }
            QosPolicy updated = apply(policy.getKey(), policy.getValue(), (QosPolicy) current);
            qos = withPolicy(qos, updated);
        }
        return qos;
    }

    @SuppressWarnings({"unchecked", "rawtypes"})
    private static Object withPolicy(Object qos, QosPolicy policy) {
        return ((EntityQos) qos).withPolicy(policy);
    }

    private static Class<? extends QosPolicy> policyType(String tag) {
        switch (tag) {
            case "user_data":
                return UserData.class;
            case "topic_data":
                return TopicData.class;
            case "group_data":
                return GroupData.class;
            case "durability":
                return Durability.class;
            case "durability_service":
                return DurabilityService.class;
            case "deadline":
                return Deadline.class;
            case "latency_budget":
                return LatencyBudget.class;
            case "lifespan":
                return Lifespan.class;
            case "liveliness":
                return Liveliness.class;
            case "reliability":
                return Reliability.class;
            case "destination_order":
                return DestinationOrder.class;
            case "history":
                return History.class;
            case "resource_limits":
                return ResourceLimits.class;
            case "ownership":
                return Ownership.class;
            case "ownership_strength":
                return OwnershipStrength.class;
            case "transport_priority":
                return TransportPriority.class;
            case "time_based_filter":
                return TimeBasedFilter.class;
            case "reader_data_lifecycle":
                return ReaderDataLifecycle.class;
            case "writer_data_lifecycle":
                return WriterDataLifecycle.class;
            case "entity_factory":
                return EntityFactory.class;
            case "partition":
                return Partition.class;
            case "presentation":
                return Presentation.class;
            default:
                throw new IllegalArgumentException("unknown QoS policy element <" + tag + ">");
        }
    }

    /** {@code current} with the fields of the policy element {@code tag} applied. */
    private QosPolicy apply(String tag, Map<String, Element> v, QosPolicy current) {
        for (String field : v.keySet()) {
            if (!knownField(tag, field)) {
                throw new IllegalArgumentException(
                        "unknown field <" + field + "> in the policy <" + tag + ">");
            }
        }
        switch (tag) {
            case "user_data": {
                UserData p = (UserData) current;
                return v.containsKey("value") ? sequence(p, v.get("value")) : p;
            }
            case "topic_data": {
                TopicData p = (TopicData) current;
                if (v.containsKey("value")) {
                    byte[] value = base64(v.get("value"));
                    return p.withValue(value, 0, value.length);
                }
                return p;
            }
            case "group_data": {
                GroupData p = (GroupData) current;
                if (v.containsKey("value")) {
                    byte[] value = base64(v.get("value"));
                    return p.withValue(value, 0, value.length);
                }
                return p;
            }
            case "durability": {
                Durability p = (Durability) current;
                return v.containsKey("kind") ? p.withKind(durabilityKind(v.get("kind"))) : p;
            }
            case "durability_service": {
                DurabilityService p = (DurabilityService) current;
                if (v.containsKey("service_cleanup_delay")) {
                    p = p.withServiceCleanupDelay(duration(v.get("service_cleanup_delay")));
                }
                if (v.containsKey("history_kind")) {
                    p = p.withHistoryKind(historyKind(v.get("history_kind")));
                }
                if (v.containsKey("history_depth")) {
                    p = p.withHistoryDepth(count(v.get("history_depth")));
                }
                if (v.containsKey("max_samples")) {
                    p = p.withMaxSamples(count(v.get("max_samples")));
                }
                if (v.containsKey("max_instances")) {
                    p = p.withMaxInstances(count(v.get("max_instances")));
                }
                if (v.containsKey("max_samples_per_instance")) {
                    p = p.withMaxSamplesPerInstance(count(v.get("max_samples_per_instance")));
                }
                return p;
            }
            case "deadline": {
                Deadline p = (Deadline) current;
                return v.containsKey("period") ? p.withPeriod(duration(v.get("period"))) : p;
            }
            case "latency_budget": {
                LatencyBudget p = (LatencyBudget) current;
                return v.containsKey("duration") ? p.withDuration(duration(v.get("duration"))) : p;
            }
            case "lifespan": {
                Lifespan p = (Lifespan) current;
                return v.containsKey("duration") ? p.withDuration(duration(v.get("duration"))) : p;
            }
            case "liveliness": {
                Liveliness p = (Liveliness) current;
                if (v.containsKey("kind")) {
                    p = p.withKind(livelinessKind(v.get("kind")));
                }
                if (v.containsKey("lease_duration")) {
                    p = p.withLeaseDuration(duration(v.get("lease_duration")));
                }
                return p;
            }
            case "reliability": {
                Reliability p = (Reliability) current;
                if (v.containsKey("kind")) {
                    p = p.withKind(reliabilityKind(v.get("kind")));
                }
                if (v.containsKey("max_blocking_time")) {
                    p = p.withMaxBlockingTime(duration(v.get("max_blocking_time")));
                }
                return p;
            }
            case "destination_order": {
                DestinationOrder p = (DestinationOrder) current;
                return v.containsKey("kind") ? p.withKind(destinationOrderKind(v.get("kind"))) : p;
            }
            case "history": {
                History p = (History) current;
                if (v.containsKey("kind")) {
                    p = p.withKind(historyKind(v.get("kind")));
                }
                if (v.containsKey("depth")) {
                    p = p.withDepth(count(v.get("depth")));
                }
                return p;
            }
            case "resource_limits": {
                ResourceLimits p = (ResourceLimits) current;
                if (v.containsKey("max_samples")) {
                    p = p.withMaxSamples(count(v.get("max_samples")));
                }
                if (v.containsKey("max_instances")) {
                    p = p.withMaxInstances(count(v.get("max_instances")));
                }
                if (v.containsKey("max_samples_per_instance")) {
                    p = p.withMaxSamplesPerInstance(count(v.get("max_samples_per_instance")));
                }
                return p;
            }
            case "ownership": {
                Ownership p = (Ownership) current;
                return v.containsKey("kind") ? p.withKind(ownershipKind(v.get("kind"))) : p;
            }
            case "ownership_strength": {
                OwnershipStrength p = (OwnershipStrength) current;
                return v.containsKey("value") ? p.withValue(count(v.get("value"))) : p;
            }
            case "transport_priority": {
                TransportPriority p = (TransportPriority) current;
                return v.containsKey("value") ? p.withValue(count(v.get("value"))) : p;
            }
            case "time_based_filter": {
                TimeBasedFilter p = (TimeBasedFilter) current;
                return v.containsKey("minimum_separation")
                        ? p.withMinimumSeparation(duration(v.get("minimum_separation"))) : p;
            }
            case "reader_data_lifecycle": {
                ReaderDataLifecycle p = (ReaderDataLifecycle) current;
                if (v.containsKey("autopurge_nowriter_samples_delay")) {
                    p = p.withAutoPurgeNoWriterSamplesDelay(
                            duration(v.get("autopurge_nowriter_samples_delay")));
                }
                if (v.containsKey("autopurge_disposed_samples_delay")) {
                    p = p.withAutoPurgeDisposedSamplesDelay(
                            duration(v.get("autopurge_disposed_samples_delay")));
                }
                return p;
            }
            case "writer_data_lifecycle": {
                WriterDataLifecycle p = (WriterDataLifecycle) current;
                return v.containsKey("autodispose_unregistered_instances")
                        ? p.withAutDisposeUnregisteredInstances(
                                bool(v.get("autodispose_unregistered_instances"))) : p;
            }
            case "entity_factory": {
                EntityFactory p = (EntityFactory) current;
                return v.containsKey("autoenable_created_entities")
                        ? p.withAutoEnableCreatedEntities(
                                bool(v.get("autoenable_created_entities"))) : p;
            }
            case "partition": {
                Partition p = (Partition) current;
                return v.containsKey("name") ? p.withName(names(v.get("name"))) : p;
            }
            case "presentation": {
                Presentation p = (Presentation) current;
                if (v.containsKey("access_scope")) {
                    p = p.withAccessScope(accessScope(v.get("access_scope")));
                }
                if (v.containsKey("coherent_access")) {
                    p = p.withCoherentAccess(bool(v.get("coherent_access")));
                }
                if (v.containsKey("ordered_access")) {
                    p = p.withOrderedAccess(bool(v.get("ordered_access")));
                }
                return p;
            }
            default:
                throw new IllegalArgumentException("unknown QoS policy element <" + tag + ">");
        }
    }

    private static boolean knownField(String policy, String field) {
        switch (policy) {
            case "user_data":
            case "topic_data":
            case "group_data":
            case "ownership_strength":
            case "transport_priority":
                return "value".equals(field);
            case "durability":
            case "destination_order":
            case "ownership":
                return "kind".equals(field);
            case "durability_service":
                return "service_cleanup_delay".equals(field) || "history_kind".equals(field)
                        || "history_depth".equals(field) || "max_samples".equals(field)
                        || "max_instances".equals(field) || "max_samples_per_instance".equals(field);
            case "deadline":
                return "period".equals(field);
            case "latency_budget":
            case "lifespan":
                return "duration".equals(field);
            case "liveliness":
                return "kind".equals(field) || "lease_duration".equals(field);
            case "reliability":
                return "kind".equals(field) || "max_blocking_time".equals(field);
            case "history":
                return "kind".equals(field) || "depth".equals(field);
            case "resource_limits":
                return "max_samples".equals(field) || "max_instances".equals(field)
                        || "max_samples_per_instance".equals(field)
                        || "initial_samples".equals(field) || "initial_instances".equals(field);
            case "time_based_filter":
                return "minimum_separation".equals(field);
            case "reader_data_lifecycle":
                return "autopurge_nowriter_samples_delay".equals(field)
                        || "autopurge_disposed_samples_delay".equals(field);
            case "writer_data_lifecycle":
                return "autodispose_unregistered_instances".equals(field);
            case "entity_factory":
                return "autoenable_created_entities".equals(field);
            case "partition":
                return "name".equals(field);
            case "presentation":
                return "access_scope".equals(field) || "coherent_access".equals(field)
                        || "ordered_access".equals(field);
            default:
                return false;
        }
    }

    private static UserData sequence(UserData policy, Element value) {
        byte[] bytes = base64(value);
        return policy.withValue(bytes, 0, bytes.length);
    }

    // --- Values of the DDS-XML schema -----------------------------------------

    private static String text(Element element) {
        return element.getTextContent().trim();
    }

    /** {@code xs:base64Binary}; whitespace between the characters is allowed. */
    private static byte[] base64(Element element) {
        StringBuilder compact = new StringBuilder();
        for (char c : element.getTextContent().toCharArray()) {
            if (!Character.isWhitespace(c)) {
                compact.append(c);
            }
        }
        try {
            return Base64.getDecoder().decode(compact.toString());
        } catch (IllegalArgumentException e) {
            throw new IllegalArgumentException("<" + element.getTagName()
                    + "> is not base64: " + compact, e);
        }
    }

    /** {@code xs:boolean}: true, false, 1 or 0. */
    private static boolean bool(Element element) {
        String value = text(element);
        if ("true".equals(value) || "1".equals(value)) {
            return true;
        }
        if ("false".equals(value) || "0".equals(value)) {
            return false;
        }
        throw new IllegalArgumentException("<" + element.getTagName() + "> is not a boolean: " + value);
    }

    /** A number, or {@code LENGTH_UNLIMITED}. */
    private static int count(Element element) {
        String value = text(element);
        if ("LENGTH_UNLIMITED".equals(value)) {
            return ResourceLimits.LENGTH_UNLIMITED;
        }
        try {
            return Integer.parseInt(value);
        } catch (NumberFormatException e) {
            throw new IllegalArgumentException("<" + element.getTagName()
                    + "> is not a number: " + value, e);
        }
    }

    /** A duration: {@code <sec>} and {@code <nanosec>}, either of them {@code DURATION_INFINITY}. */
    private Duration duration(Element element) {
        Element sec = null;
        Element nanosec = null;
        for (Element child : children(element)) {
            if ("sec".equals(child.getTagName())) {
                sec = child;
            } else if ("nanosec".equals(child.getTagName())) {
                nanosec = child;
            } else {
                throw new IllegalArgumentException("unexpected <" + child.getTagName()
                        + "> in the duration <" + element.getTagName() + ">");
            }
        }
        if (sec == null && nanosec == null) {
            String bare = text(element);
            if (bare.isEmpty()) {
                return org.zerodds.core.DurationImpl.zero(environment);
            }
            if (isInfinite(bare)) {
                return org.zerodds.core.DurationImpl.infinite(environment);
            }
            throw new IllegalArgumentException("<" + element.getTagName()
                    + "> is not a duration: " + bare);
        }
        if ((sec != null && isInfinite(text(sec))) || (nanosec != null && isInfinite(text(nanosec)))) {
            return org.zerodds.core.DurationImpl.infinite(environment);
        }
        long nanos = 0;
        try {
            if (sec != null && !text(sec).isEmpty()) {
                nanos += TimeUnit.SECONDS.toNanos(Long.parseLong(text(sec)));
            }
            if (nanosec != null && !text(nanosec).isEmpty()) {
                nanos += Long.parseLong(text(nanosec));
            }
        } catch (NumberFormatException e) {
            throw new IllegalArgumentException("<" + element.getTagName()
                    + "> holds no number of seconds and nanoseconds", e);
        }
        return org.zerodds.core.DurationImpl.of(environment, nanos, TimeUnit.NANOSECONDS);
    }

    private static boolean isInfinite(String value) {
        return "DURATION_INFINITY".equals(value) || "DURATION_INFINITE_SEC".equals(value)
                || "DURATION_INFINITE_NSEC".equals(value);
    }

    /** The {@code <element>}s of a string sequence. */
    private static java.util.Collection<String> names(Element element) {
        java.util.Set<String> names = new LinkedHashSet<>();
        List<Element> items = children(element);
        if (items.isEmpty()) {
            String bare = text(element);
            if (!bare.isEmpty()) {
                names.add(bare);
            }
            return names;
        }
        for (Element item : items) {
            if (!"element".equals(item.getTagName())) {
                throw new IllegalArgumentException("unexpected <" + item.getTagName()
                        + "> in <" + element.getTagName() + ">");
            }
            names.add(text(item));
        }
        return names;
    }

    private static Durability.Kind durabilityKind(Element element) {
        switch (text(element)) {
            case "VOLATILE_DURABILITY_QOS":
                return Durability.Kind.VOLATILE;
            case "TRANSIENT_LOCAL_DURABILITY_QOS":
                return Durability.Kind.TRANSIENT_LOCAL;
            case "TRANSIENT_DURABILITY_QOS":
                return Durability.Kind.TRANSIENT;
            case "PERSISTENT_DURABILITY_QOS":
                return Durability.Kind.PERSISTENT;
            default:
                throw unknownLiteral(element);
        }
    }

    private static History.Kind historyKind(Element element) {
        switch (text(element)) {
            case "KEEP_LAST_HISTORY_QOS":
                return History.Kind.KEEP_LAST;
            case "KEEP_ALL_HISTORY_QOS":
                return History.Kind.KEEP_ALL;
            default:
                throw unknownLiteral(element);
        }
    }

    private static Liveliness.Kind livelinessKind(Element element) {
        switch (text(element)) {
            case "AUTOMATIC_LIVELINESS_QOS":
                return Liveliness.Kind.AUTOMATIC;
            case "MANUAL_BY_PARTICIPANT_LIVELINESS_QOS":
                return Liveliness.Kind.MANUAL_BY_PARTICIPANT;
            case "MANUAL_BY_TOPIC_LIVELINESS_QOS":
                return Liveliness.Kind.MANUAL_BY_TOPIC;
            default:
                throw unknownLiteral(element);
        }
    }

    private static Reliability.Kind reliabilityKind(Element element) {
        switch (text(element)) {
            case "BEST_EFFORT_RELIABILITY_QOS":
                return Reliability.Kind.BEST_EFFORT;
            case "RELIABLE_RELIABILITY_QOS":
                return Reliability.Kind.RELIABLE;
            default:
                throw unknownLiteral(element);
        }
    }

    private static DestinationOrder.Kind destinationOrderKind(Element element) {
        switch (text(element)) {
            case "BY_RECEPTION_TIMESTAMP_DESTINATIONORDER_QOS":
                return DestinationOrder.Kind.BY_RECEPTION_TIMESTAMP;
            case "BY_SOURCE_TIMESTAMP_DESTINATIONORDER_QOS":
                return DestinationOrder.Kind.BY_SOURCE_TIMESTAMP;
            default:
                throw unknownLiteral(element);
        }
    }

    private static Ownership.Kind ownershipKind(Element element) {
        switch (text(element)) {
            case "SHARED_OWNERSHIP_QOS":
                return Ownership.Kind.SHARED;
            case "EXCLUSIVE_OWNERSHIP_QOS":
                return Ownership.Kind.EXCLUSIVE;
            default:
                throw unknownLiteral(element);
        }
    }

    private static Presentation.AccessScopeKind accessScope(Element element) {
        switch (text(element)) {
            case "INSTANCE_PRESENTATION_QOS":
                return Presentation.AccessScopeKind.INSTANCE;
            case "TOPIC_PRESENTATION_QOS":
                return Presentation.AccessScopeKind.TOPIC;
            case "GROUP_PRESENTATION_QOS":
                return Presentation.AccessScopeKind.GROUP;
            default:
                throw unknownLiteral(element);
        }
    }

    private static IllegalArgumentException unknownLiteral(Element element) {
        return new IllegalArgumentException("<" + element.getTagName()
                + "> holds no value of the DDS-XML schema: " + text(element));
    }
}
