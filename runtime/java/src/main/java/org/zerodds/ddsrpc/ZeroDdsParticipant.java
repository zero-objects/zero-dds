package org.zerodds.ddsrpc;

import org.omg.dds.rpc.*;
import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.TimeUnit;
import java.util.function.Function;
import org.omg.dds.core.ServiceEnvironment;
import org.omg.dds.core.policy.PolicyFactory;
import org.omg.dds.pub.DataWriterQos;
import org.omg.dds.pub.Publisher;
import org.omg.dds.rpc.DataReader;
import org.omg.dds.rpc.DataWriter;
import org.omg.dds.rpc.DomainParticipant;
import org.omg.dds.rpc.RemoteUnsupportedError;
import org.omg.dds.rpc.RpcPeerInfo;
import org.omg.dds.rpc.RpcQos;
import org.omg.dds.rpc.SampleIdentity;
import org.omg.dds.rpc.TakenSample;
import org.omg.dds.core.event.DataAvailableEvent;
import org.omg.dds.sub.DataReaderAdapter;
import org.omg.dds.sub.DataReaderQos;
import org.omg.dds.sub.Sample;
import org.omg.dds.sub.Subscriber;
import org.omg.dds.topic.Topic;
import org.zerodds.cdr.TypeSupportImpl;
import org.zerodds.internal.SourceIdentity;
import org.zerodds.sub.SampleImpl;

/**
 * The seam of DDS-RPC 1.0 §7.11.1.4.23 and §7.11.2 over ZeroDDS's DDS Java
 * PSM ({@code org.omg.dds.domain.DomainParticipant} of
 * {@code crates/java-omgdds}): every writer and reader the RPC runtime asks
 * for is a PSM DataWriter or DataReader on a topic announced under the type
 * name the RPC layer chose.
 *
 * <h2>Typed by the topic type</h2>
 *
 * <p>When the type support of the endpoint names the class of its topic type
 * ({@link TopicSample#classOf}), the entity is a {@code DataWriter<T>} or
 * {@code DataReader<T>} of that type ({@link RpcTopicTypeSupport}), on a
 * {@code Topic<T>}: the one the application obtains from the requester or
 * replier (§7.11.2, {@code getRequestDataWriter()} and the other three).
 * The runtime keeps writing and reading the octets it frames through the same
 * entity ({@code DataWriterImpl.writeSerializedIdentified},
 * {@code DataReaderImpl.takeSerialized}). A type support that names no class,
 * or a topic the participant already has under another type, gives an entity
 * of octets ({@code DataWriter<byte[]>}), which the requester and replier do
 * not hand out. The Publisher, the Subscriber and the PSM QoS the params name
 * ({@link PsmEndpoint}) are the ones the entity is created in and with.
 *
 * <h2>Identities</h2>
 *
 * <p>A write returns the identity the sample was sent under, the GUID of the
 * writer and the sequence number it gave the sample ({@code DataWriterImpl.writeIdentified(data,
 * related)}), and a related identity given to a write travels as
 * {@code PID_RELATED_SAMPLE_IDENTITY} (§7.8.2) in the inline QoS of the DATA
 * (RTPS) or on the message (inside the process). A taken sample carries both
 * ({@code SampleImpl.identity()} and {@code relatedIdentity()}), so the Enhanced
 * mapping, which correlates by the related identity alone, runs on this binding.
 *
 * <h2>What this binding does not do</h2>
 *
 * <ul>
 *   <li>(none of the discovery extensions is missing: the service-side endpoints
 *       announce {@code service_instance_name} (PID 0x0080), and {@link
 *       org.omg.dds.rpc.DataWriter#announce} / {@link org.omg.dds.rpc.DataReader#announce} add
 *       {@code related_*_key} (0x0081) and {@code topic_aliases} (0x0082).)
 *   <li>The encapsulation of a sample is the PSM's (XCDR2): the DATA
 *       representation an endpoint offers ({@link RpcQos#dataRepresentation})
 *       is not passed on.
 *   <li>The mutable QoS policies (§7.10.2) are passed to the PSM entity
 *       ({@code DataWriter.setQos}, {@code DataReader.setQos}), which applies a
 *       change of a changeable policy (DEADLINE, LIFESPAN) and refuses one of an
 *       immutable policy with {@code ImmutablePolicyException}, reported as
 *       {@link RemoteUnsupportedError}.
 * </ul>
 */
public final class ZeroDdsParticipant implements DomainParticipant {

    private final org.omg.dds.domain.DomainParticipant participant;
    private Publisher publisher;
    private Subscriber subscriber;
    private final List<Writer> writers = new ArrayList<>();
    private final List<Reader> readers = new ArrayList<>();

    /**
     * @param participant the PSM participant the entities are created from
     */
    public ZeroDdsParticipant(org.omg.dds.domain.DomainParticipant participant) {
        this.participant = participant;
    }

    /** The DDS-Java-PSM participant the entities are created from. */
    public org.omg.dds.domain.DomainParticipant psm() {
        return participant;
    }

    @SuppressWarnings("unchecked")
    private Topic<byte[]> topicOf(String name, RawTypeSupport support) {
        Object existing = participant.lookupTopicDescription(name);
        if (existing != null) {
            return (Topic<byte[]>) existing;
        }
        ServiceEnvironment env = participant.getEnvironment();
        return participant.createTopic(name,
                new TypeSupportImpl<>(env, support.getTypeName(), byte[].class, support));
    }

    /**
     * The topic {@code name} typed by the topic type of {@code support}: the existing topic when
     * its type is that class, a new one when there is none; {@code null} when the participant
     * has the topic under another type (an endpoint whose type support names no class, or
     * another class), whose entities then carry octets only.
     */
    @SuppressWarnings("unchecked")
    private <T> Topic<T> typedTopicOf(String name, RpcTopicTypeSupport<T> support) {
        Object existing = participant.lookupTopicDescription(name);
        if (existing != null) {
            Topic<?> topic = (Topic<?>) existing;
            return topic.getTypeSupport().getType() == support.type() ? (Topic<T>) topic : null;
        }
        ServiceEnvironment env = participant.getEnvironment();
        return participant.createTopic(name,
                new TypeSupportImpl<>(env, support.getTypeName(), support.type(), support));
    }

    @Override
    public DataWriter createWriter(
            String topic, String typeName, RpcQos qos, String serviceInstanceName) {
        return createWriter(topic, typeName, qos, serviceInstanceName, null, null);
    }

    @Override
    public DataWriter createWriter(
            String topic,
            String typeName,
            RpcQos qos,
            String serviceInstanceName,
            org.zerodds.ddsrpc.TypeDescription description) {
        return createWriter(topic, typeName, qos, serviceInstanceName, description, null);
    }

    /**
     * §7.11.2: the writer is a DataWriter of the PSM typed by the topic type when {@code psm}
     * gives one, in the Publisher it names (one of this participant's), with the DataWriterQos
     * it names; without them in a Publisher of this binding with the QoS {@code qos} gives,
     * and a writer of octets.
     *
     * @throws IllegalArgumentException when the Publisher belongs to another participant
     */
    @Override
    public synchronized DataWriter createWriter(
            String topic,
            String typeName,
            RpcQos qos,
            String serviceInstanceName,
            org.zerodds.ddsrpc.TypeDescription description,
            PsmEndpoint<?> psm) {
        Publisher in;
        if (psm != null && psm.publisher() != null) {
            if (psm.publisher().getParent() != participant) {
                throw new IllegalArgumentException(
                        "the Publisher belongs to another DomainParticipant");
            }
            in = psm.publisher();
        } else {
            if (publisher == null) {
                publisher = participant.createPublisher();
            }
            in = publisher;
        }
        DataWriterQos psmQos = psm != null && psm.writerQos() != null
                ? psm.writerQos() : writerQos(qos);
        RawTypeSupport support = new RawTypeSupport(typeName, description);
        org.zerodds.pub.DataWriterImpl<?> entity =
                psm == null || psm.sample() == null ? null
                        : typedWriter(in, topic, support, psm.sample(), psmQos);
        boolean typed = entity != null;
        if (!typed) {
            entity = (org.zerodds.pub.DataWriterImpl<byte[]>) ((org.zerodds.pub.PublisherImpl) in)
                    .createDataWriter(topicOf(topic, support), support, psmQos);
        }
        // §7.6.2.1.1: the service-side endpoints announce the instance they serve
        // (PID 0x0080); a client-side endpoint is created with the empty name.
        if (serviceInstanceName != null && !serviceInstanceName.isEmpty()) {
            entity.setRpcDiscovery(new org.zerodds.rtps.RpcDiscovery(serviceInstanceName, null, null));
        }
        Writer writer = new Writer(entity, typed, qos, this::writerQos, serviceInstanceName);
        writers.add(writer);
        return writer;
    }

    private <T> org.zerodds.pub.DataWriterImpl<T> typedWriter(Publisher in, String topic,
            RawTypeSupport announced, TopicSample<T> sample, DataWriterQos qos) {
        RpcTopicTypeSupport<T> support = new RpcTopicTypeSupport<>(announced, sample);
        Topic<T> typedTopic = typedTopicOf(topic, support);
        if (typedTopic == null) {
            return null;
        }
        return (org.zerodds.pub.DataWriterImpl<T>) ((org.zerodds.pub.PublisherImpl) in)
                .createDataWriter(typedTopic, support, qos);
    }

    @Override
    public DataReader createReader(
            String topic, String typeName, RpcQos qos, String serviceInstanceName) {
        return createReader(topic, typeName, qos, serviceInstanceName, null, null);
    }

    @Override
    public DataReader createReader(
            String topic,
            String typeName,
            RpcQos qos,
            String serviceInstanceName,
            org.zerodds.ddsrpc.TypeDescription description) {
        return createReader(topic, typeName, qos, serviceInstanceName, description, null);
    }

    /** As {@link #createWriter(String, String, RpcQos, String, TypeDescription, PsmEndpoint)}, for a reader. */
    @Override
    public synchronized DataReader createReader(
            String topic,
            String typeName,
            RpcQos qos,
            String serviceInstanceName,
            org.zerodds.ddsrpc.TypeDescription description,
            PsmEndpoint<?> psm) {
        Subscriber in;
        if (psm != null && psm.subscriber() != null) {
            if (psm.subscriber().getParent() != participant) {
                throw new IllegalArgumentException(
                        "the Subscriber belongs to another DomainParticipant");
            }
            in = psm.subscriber();
        } else {
            if (subscriber == null) {
                subscriber = participant.createSubscriber();
            }
            in = subscriber;
        }
        RawTypeSupport support = new RawTypeSupport(typeName, description);
        Function<RpcQos, DataReaderQos> toPsm =
                support.matchesByTypeInformation() ? this::describedReaderQos : this::readerQos;
        DataReaderQos psmQos = psm != null && psm.readerQos() != null
                ? psm.readerQos() : toPsm.apply(qos);
        org.zerodds.sub.DataReaderImpl<?> entity =
                psm == null || psm.sample() == null ? null
                        : typedReader(in, topic, support, psm.sample(), psmQos);
        boolean typed = entity != null;
        if (!typed) {
            entity = (org.zerodds.sub.DataReaderImpl<byte[]>) ((org.zerodds.sub.SubscriberImpl) in)
                    .createDataReader(topicOf(topic, support), support, psmQos);
        }
        if (serviceInstanceName != null && !serviceInstanceName.isEmpty()) {
            entity.setRpcDiscovery(new org.zerodds.rtps.RpcDiscovery(serviceInstanceName, null, null));
        }
        Reader reader = new Reader(entity, typed, qos, toPsm, serviceInstanceName);
        readers.add(reader);
        return reader;
    }

    private <T> org.zerodds.sub.DataReaderImpl<T> typedReader(Subscriber in, String topic,
            RawTypeSupport announced, TopicSample<T> sample, DataReaderQos qos) {
        RpcTopicTypeSupport<T> support = new RpcTopicTypeSupport<>(announced, sample);
        Topic<T> typedTopic = typedTopicOf(topic, support);
        if (typedTopic == null) {
            return null;
        }
        return (org.zerodds.sub.DataReaderImpl<T>) ((org.zerodds.sub.SubscriberImpl) in)
                .createDataReader(typedTopic, support, qos);
    }

    @Override
    public synchronized void deleteWriter(DataWriter writer) {
        if (!(writer instanceof Writer) || !writers.remove(writer)) {
            throw new IllegalArgumentException("not a writer this participant created");
        }
        ((Writer) writer).psm.close();
    }

    @Override
    public synchronized void deleteReader(DataReader reader) {
        if (!(reader instanceof Reader) || !readers.remove(reader)) {
            throw new IllegalArgumentException("not a reader this participant created");
        }
        ((Reader) reader).psm.setListener(null);
        ((Reader) reader).psm.close();
    }

    private PolicyFactory policies() {
        return participant.getEnvironment().getSPI().getPolicyFactory();
    }

    /** The PSM writer QoS an {@link RpcQos} asks for. */
    private DataWriterQos writerQos(RpcQos qos) {
        PolicyFactory pf = policies();
        DataWriterQos out = org.zerodds.core.qos.DataWriterQosImpl.defaults(participant.getEnvironment())
                .withPolicy(qos.reliability() == RpcQos.Reliability.RELIABLE
                        ? pf.Reliability().withReliable() : pf.Reliability().withBestEffort())
                .withPolicy(pf.Durability().withKind(
                        org.omg.dds.core.policy.Durability.Kind.valueOf(qos.durability().name())))
                .withPolicy(qos.historyKind() == RpcQos.HistoryKind.KEEP_ALL
                        ? pf.History().withKeepAll() : pf.History().withKeepLast(qos.historyDepth()));
        if (qos.deadlineNanos() != 0L) {
            out = out.withPolicy(pf.Deadline().withPeriod(qos.deadlineNanos(), TimeUnit.NANOSECONDS));
        }
        if (qos.lifespanNanos() != 0L) {
            out = out.withPolicy(pf.Lifespan().withDuration(qos.lifespanNanos(), TimeUnit.NANOSECONDS));
        }
        return out;
    }

    /** The PSM reader QoS an {@link RpcQos} asks for. */
    private DataReaderQos readerQos(RpcQos qos) {
        PolicyFactory pf = policies();
        DataReaderQos out = org.zerodds.core.qos.DataReaderQosImpl.defaults(participant.getEnvironment())
                .withPolicy(qos.reliability() == RpcQos.Reliability.RELIABLE
                        ? pf.Reliability().withReliable() : pf.Reliability().withBestEffort())
                .withPolicy(pf.Durability().withKind(
                        org.omg.dds.core.policy.Durability.Kind.valueOf(qos.durability().name())))
                .withPolicy(qos.historyKind() == RpcQos.HistoryKind.KEEP_ALL
                        ? pf.History().withKeepAll() : pf.History().withKeepLast(qos.historyDepth()));
        if (qos.deadlineNanos() != 0L) {
            out = out.withPolicy(pf.Deadline().withPeriod(qos.deadlineNanos(), TimeUnit.NANOSECONDS));
        }
        return out;
    }

    /**
     * Takes every sample of {@code reader}: a typed reader without making values, each sample
     * with the octets it arrived as; a reader of octets with the octets as the value.
     */
    private static <T> List<Sample<T>> taken(org.zerodds.sub.DataReaderImpl<T> reader, boolean typed) {
        return typed ? reader.takeSerialized() : reader.take(new ArrayList<Sample<T>>());
    }

    /** Calls {@code listener} when {@code reader} has data, whatever the reader's type. */
    private static <T> void listen(org.zerodds.sub.DataReaderImpl<T> reader, Runnable listener) {
        reader.setListener(
                new DataReaderAdapter<T>() {
                    @Override
                    public void onDataAvailable(DataAvailableEvent<T> event) {
                        listener.run();
                    }
                });
    }

    /**
     * The PSM reader QoS of a topic type that pairs by its TypeInformation: as
     * {@link #readerQos}, with TYPE_CONSISTENCY_ENFORCEMENT {@code ASSIGNABLE_TYPE_CONSISTENCY},
     * announced as ALLOW_TYPE_COERCION (XTypes 1.3 §7.6.3.4). The pairing by assignability of
     * §7.5.1.2.6 presumes it; under the PSM's default, EXACT_TYPE_TYPE_CONSISTENCY, the reader
     * announces DISALLOW_TYPE_COERCION and a peer refuses a writer of another type (measured
     * against the Rust runtime: "the types are not equivalent").
     */
    private DataReaderQos describedReaderQos(RpcQos qos) {
        return readerQos(qos).withPolicy(policies().TypeConsistency().withKind(
                org.omg.dds.core.policy.TypeConsistencyEnforcement.Kind.ASSIGNABLE_TYPE_CONSISTENCY));
    }

    private static List<RpcPeerInfo> peersOf(List<org.zerodds.rtps.RpcPeer> peers) {
        List<RpcPeerInfo> out = new ArrayList<>();
        for (org.zerodds.rtps.RpcPeer peer : peers) {
            out.add(new RpcPeerInfo(peer.guid(), peer.rpc().serviceInstanceName(),
                    peer.rpc().relatedEntityGuid(), peer.rpc().topicAliases()));
        }
        return out;
    }

    private static SampleIdentity identityOf(SourceIdentity identity) {
        return identity == null ? SampleIdentity.UNKNOWN
                : new SampleIdentity(identity.guid(), identity.sequence());
    }

    private static SourceIdentity sourceOf(SampleIdentity identity) {
        return identity == null || identity.isUnknown() ? null
                : new SourceIdentity(identity.writerGuid(), identity.sequenceNumber());
    }

    private static final class Writer implements DataWriter {
        final org.zerodds.pub.DataWriterImpl<?> psm;
        /** Whether {@link #psm} is typed by the topic type rather than carrying octets. */
        private final boolean typed;
        private RpcQos qos;
        private final Function<RpcQos, DataWriterQos> toPsm;
        private final String instance;

        Writer(org.zerodds.pub.DataWriterImpl<?> psm, boolean typed, RpcQos qos,
                Function<RpcQos, DataWriterQos> toPsm, String instance) {
            this.psm = psm;
            this.typed = typed;
            this.qos = qos;
            this.toPsm = toPsm;
            this.instance = instance == null ? "" : instance;
        }

        @Override
        public byte[] guid() {
            return psm.nextIdentity().guid().clone();
        }

        @Override
        public boolean announce(List<String> topicAliases, byte[] relatedEntityGuid) {
            psm.setRpcDiscovery(
                    new org.zerodds.rtps.RpcDiscovery(instance, relatedEntityGuid, topicAliases));
            return true;
        }

        @Override
        public int matchedCount(String serviceInstanceName) {
            int count = 0;
            for (org.zerodds.rtps.RpcPeer peer : psm.getMatchedRpcPeers()) {
                if (peer.rpc().serviceInstanceName().equals(serviceInstanceName)) {
                    ++count;
                }
            }
            return count;
        }

        @Override
        public java.util.Collection<String> matchedInstances() {
            java.util.Set<String> names = new java.util.LinkedHashSet<>();
            for (org.zerodds.rtps.RpcPeer peer : psm.getMatchedRpcPeers()) {
                if (!peer.rpc().serviceInstanceName().isEmpty()) {
                    names.add(peer.rpc().serviceInstanceName());
                }
            }
            return names;
        }

        @Override
        public List<RpcPeerInfo> matchedPeers() {
            return peersOf(psm.getMatchedRpcPeers());
        }

        @Override
        public org.omg.dds.pub.DataWriter<?> psm() {
            return typed ? psm : null;
        }

        @Override
        public SampleIdentity writeTo(byte[] sample, String serviceInstanceName) {
            return identityOf(psm.writeSerializedIdentified(sample, null, serviceInstanceName));
        }

        @Override
        public SampleIdentity write(byte[] sample) {
            return identityOf(psm.writeSerializedIdentified(sample, null, null));
        }

        @Override
        public SampleIdentity write(byte[] sample, SampleIdentity related) {
            return identityOf(psm.writeSerializedIdentified(sample, sourceOf(related), null));
        }

        @Override
        public boolean carriesRelatedIdentity() {
            return true;
        }

        @Override
        public SampleIdentity nextIdentity() {
            return identityOf(psm.nextIdentity());
        }

        @Override
        public int matchedCount() {
            // A reader matched over the RTPS wire (another process) is a peer of the
            // discovery extension but is not counted in the status of the in-process bus.
            return Math.max(
                    psm.getPublicationMatchedStatus().getCurrentCount(),
                    psm.getMatchedRpcPeers().size());
        }

        @Override
        public synchronized void setQos(RpcQos requested) {
            if (!qos.mutableChangeTo(requested)) {
                throw new IllegalArgumentException(
                        "RELIABILITY, DURABILITY and HISTORY are not mutable (DDS 1.4 §2.2.3)");
            }
            if (qos.equals(requested)) {
                return;
            }
            try {
                psm.setQos(toPsm.apply(requested));
            } catch (org.omg.dds.core.ImmutablePolicyException refused) {
                throw new RemoteUnsupportedError(
                        "the DDS Java PSM writer refuses the change of DEADLINE or LIFESPAN: "
                                + refused.getMessage());
            }
            qos = requested;
        }
    }

    private static final class Reader implements DataReader {
        final org.zerodds.sub.DataReaderImpl<?> psm;
        /** Whether {@link #psm} is typed by the topic type and keeps the octets beside. */
        private final boolean typed;
        private RpcQos qos;
        private final Function<RpcQos, DataReaderQos> toPsm;
        private final String instance;

        Reader(org.zerodds.sub.DataReaderImpl<?> psm, boolean typed, RpcQos qos,
                Function<RpcQos, DataReaderQos> toPsm, String instance) {
            this.psm = psm;
            this.typed = typed;
            this.qos = qos;
            this.toPsm = toPsm;
            this.instance = instance == null ? "" : instance;
        }

        @Override
        public byte[] guid() {
            return psm.endpointGuid();
        }

        @Override
        public List<RpcPeerInfo> matchedPeers() {
            return peersOf(psm.getMatchedRpcPeers());
        }

        @Override
        public boolean announce(List<String> topicAliases, byte[] relatedEntityGuid) {
            psm.setRpcDiscovery(
                    new org.zerodds.rtps.RpcDiscovery(instance, relatedEntityGuid, topicAliases));
            return true;
        }

        @Override
        public org.omg.dds.sub.DataReader<?> psm() {
            return typed ? psm : null;
        }

        @Override
        public List<TakenSample> take() {
            List<TakenSample> out = new ArrayList<>();
            for (Sample<?> sample : taken(psm, typed)) {
                byte[] octets = typed
                        ? ((SampleImpl<?>) sample).serialized()
                        : (byte[]) sample.getData();
                // A state change without data (valid_data false, DDS 1.4 §2.2.2.5.1) carries
                // no request or reply; the requester and replier hand it over only when
                // receiveNondataSamples asked for it.
                if (octets == null) {
                    out.add(TakenSample.withoutData(
                            sample instanceof SampleImpl
                                    ? identityOf(((SampleImpl<?>) sample).identity())
                                    : SampleIdentity.UNKNOWN,
                            sample.getInstanceState()));
                    continue;
                }
                if (sample instanceof SampleImpl) {
                    SampleImpl<?> impl = (SampleImpl<?>) sample;
                    out.add(new TakenSample(octets, identityOf(impl.identity()),
                            identityOf(impl.relatedIdentity())));
                } else {
                    out.add(new TakenSample(octets, SampleIdentity.UNKNOWN));
                }
            }
            return out;
        }

        @Override
        public int matchedCount() {
            return Math.max(
                    psm.getSubscriptionMatchedStatus().getCurrentCount(),
                    psm.getMatchedRpcPeers().size());
        }

        @Override
        public void setDataAvailableListener(Runnable listener) {
            if (listener == null) {
                psm.setListener(null);
                return;
            }
            listen(psm, listener);
        }

        @Override
        public synchronized void setQos(RpcQos requested) {
            if (!qos.mutableChangeTo(requested)) {
                throw new IllegalArgumentException(
                        "RELIABILITY, DURABILITY and HISTORY are not mutable (DDS 1.4 §2.2.3)");
            }
            if (qos.equals(requested)) {
                return;
            }
            try {
                psm.setQos(toPsm.apply(requested));
            } catch (org.omg.dds.core.ImmutablePolicyException refused) {
                throw new RemoteUnsupportedError(
                        "the DDS Java PSM reader refuses the change of DEADLINE: "
                                + refused.getMessage());
            }
            qos = requested;
        }
    }
}
