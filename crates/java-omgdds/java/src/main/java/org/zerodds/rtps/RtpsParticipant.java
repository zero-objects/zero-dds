// SPDX-License-Identifier: Apache-2.0
package org.zerodds.rtps;

import java.io.IOException;
import java.net.InetAddress;
import java.net.InetSocketAddress;
import java.net.NetworkInterface;
import java.net.StandardProtocolFamily;
import java.net.StandardSocketOptions;
import java.nio.ByteBuffer;
import java.nio.channels.DatagramChannel;
import java.util.ArrayList;
import java.util.Enumeration;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.Random;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.function.Consumer;
import org.zerodds.typelookup.TypeLookupService;
import org.zerodds.typelookup.generated.ZdTypes.DDS;

/**
 * Pure-Java RTPS participant: the native wire stack behind the DDS Java PSM.
 * No FFI, no Rust library — {@link DatagramChannel} UDP sockets carry SPDP
 * (multicast participant discovery), SEDP (unicast endpoint discovery, with the
 * cross-binding TypeIdentifier), and best-effort user DATA. Interoperates on
 * the wire with the Rust zerodds stack (DDSI-RTPS 2.5).
 *
 * <p>One participant per (domain, JVM) by default via {@link #get(int)}; the
 * sockets and discovery threads start on first use.
 */
public final class RtpsParticipant {

    // ---- global registry (one participant per domain) ----
    private static final Map<Integer, RtpsParticipant> BY_DOMAIN = new ConcurrentHashMap<>();

    public static RtpsParticipant get(int domainId) {
        return BY_DOMAIN.computeIfAbsent(domainId, RtpsParticipant::new);
    }

    /**
     * Closes the participant {@link #get(int)} made for {@code domainId}, if there is one: its
     * sockets and threads end, and the next {@code get} starts a fresh one. For a process that
     * leaves a domain, and for a test that gives a leased domain back, so that no participant of
     * it stays in the domain for the next holder.
     */
    public static void closeDomain(int domainId) {
        RtpsParticipant participant = BY_DOMAIN.remove(domainId);
        if (participant != null) {
            participant.close();
        }
    }

    /**
     * Create a fresh, non-cached participant on {@code domainId} (its own GUID
     * prefix and unicast port). Used for multi-participant loopback tests and
     * apps that want more than one participant per domain in a JVM.
     */
    public static RtpsParticipant newParticipant(int domainId) {
        return new RtpsParticipant(domainId);
    }

    /** Optional match/event log sink (System.err by default). */
    public static volatile Consumer<String> LOG = msg -> System.err.println("[rtps] " + msg);

    private static void log(String m) {
        Consumer<String> l = LOG;
        if (l != null) {
            l.accept(m);
        }
    }

    // ---- endpoint registrations ----

    public final class WireWriter {
        final byte[] entityId; // 4
        final byte[] guid; // 16
        final String topic;
        final String typeName;
        final byte[] encap; // 4-byte encapsulation header
        final byte[] typeId;
        final byte[] typeInformation;
        final boolean keyed;
        final boolean reliable;
        /** The QoS the writer announces (SEDP, DDS 1.4 §2.2.5); null: kinds only. */
        volatile QosParameters qos;
        /**
         * Decides on a remote reader of another type name (type coercion); {@code null}: only
         * the same type name matches.
         */
        final java.util.function.Predicate<DiscoveryData.Endpoint> typeMatcher;
        /**
         * The types pair by the assignability of the announced TypeInformation (XTypes 1.3
         * 7.6.3.4.2) and not by the type name; {@code typeMatcher} is then not asked.
         */
        final boolean byTypeInformation;
        long sn = 0;
        /** The group info of the Publisher for the HEARTBEAT (GroupInfoFlag); {@code null} result: none. */
        private volatile java.util.function.Supplier<GroupStamp> groupInfo;

        /** Gives this writer the group info of its Publisher (access scope GROUP, DDSI-RTPS 2.5 8.3.7.5). */
        public void setGroupInfo(java.util.function.Supplier<GroupStamp> info) {
            this.groupInfo = info;
        }

        /** The DDS-RPC 1.0 §7.6.2.1 extensions this writer announces. */
        volatile RpcDiscovery rpc = RpcDiscovery.NONE;
        // matched remote readers: guidHex -> target
        final Map<String, MatchedReader> matched = new ConcurrentHashMap<>();

        WireWriter(byte[] entityId, String topic, String typeName, byte[] encap,
                   byte[] typeId, byte[] typeInformation, boolean keyed, boolean reliable,
                   QosParameters qos, java.util.function.Predicate<DiscoveryData.Endpoint> typeMatcher,
                   boolean byTypeInformation) {
            this.typeMatcher = typeMatcher;
            this.byTypeInformation = byTypeInformation;
            this.entityId = entityId;
            this.guid = new Guid(guidPrefix, entityId).bytes();
            this.topic = topic;
            this.typeName = typeName;
            if (encap == null || encap.length != 4 || encap[2] != 0 || encap[3] != 0) {
                throw new IllegalArgumentException("Writer requires a four-byte encapsulation without padding options");
            }
            SerializedSample.parse(encap);
            this.encap = encap.clone();
            this.typeId = typeId == null ? new byte[0] : typeId.clone();
            this.typeInformation = requireTypeInformation(typeInformation);
            this.keyed = keyed;
            this.qos = qos;
            // A writer that announces its QoS is reliable exactly when that
            // QoS says RELIABLE; one that does not keeps what the caller asked.
            this.reliable = qos != null && qos.reliability != null
                    ? qos.reliability.kind == Rtps.RELIABILITY_RELIABLE : reliable;
        }

        /**
         * The QoS of the DDS writer changed (DDS 1.4 2.2.2.1.1.1 set_qos): the endpoint is
         * announced again on SEDP with the new QoS, as a new sample of the same instance, and the
         * remote readers match again.
         */
        public void updateQos(QosParameters changed) {
            this.qos = changed;
            announceAllToPeers();
        }

        /**
         * Announces the DDS-RPC 1.0 §7.6.2.1 discovery extensions of this writer
         * ({@code service_instance_name}, {@code related_entity_guid}, {@code topic_aliases})
         * and matches again with what is known: a writer with aliases may now correspond to
         * readers it did not.
         */
        public void setRpcDiscovery(RpcDiscovery value) {
            rpc = value == null ? RpcDiscovery.NONE : value;
            synchronized (RtpsParticipant.this) {
                for (DiscoveryData.Endpoint e : new ArrayList<>(remoteReaders.values())) {
                    tryMatchWriter(this, e);
                }
            }
            announceAllToPeers();
        }

        /** The extensions this writer announces. */
        public RpcDiscovery rpcDiscovery() {
            return rpc;
        }

        /** The remote readers this writer is matched with (SEDP). */
        public List<DiscoveryData.Endpoint> matchedEndpoints() {
            List<DiscoveryData.Endpoint> out = new ArrayList<>();
            for (String guid : matched.keySet()) {
                DiscoveryData.Endpoint e = remoteReaders.get(guid);
                if (e != null) {
                    out.add(e);
                }
            }
            return out;
        }

        /** The sequence number the next sample will be sent under. */
        public synchronized long nextSequence() {
            return sn + 1;
        }

        /** The GUID of this writer. */
        public byte[] guid() {
            return guid.clone();
        }

        /** Send one sample body (encapsulation is prepended) to all matched readers. */
        public synchronized void write(byte[] body) {
            write(body, null);
        }

        /**
         * As {@link #write(byte[])}, with the identity of the sample this one relates to
         * as {@code PID_RELATED_SAMPLE_IDENTITY} in the inline QoS (DDS-RPC 1.0 §7.8.2).
         *
         * @return the sequence number the sample was sent under.
         */
        public synchronized long write(byte[] body, org.zerodds.internal.SourceIdentity related) {
            return write(body, related, 0L, null, null);
        }

        /** The coherent set the last sample belonged to (named by the caller), and its first sequence number. */
        private long coherentName;
        private long coherentFirst;

        /**
         * As {@link #write(byte[], org.zerodds.internal.SourceIdentity)}, for a sample of the
         * coherent set {@code coherentSetName} of the writer's Publisher (0: in no set); it travels as
         * {@code PID_COHERENT_SET} in the inline QoS, valued with the sequence number of the first
         * sample of the set (DDSI-RTPS 2.5 8.7.6, 9.6.4.2).
         */
        public synchronized long write(byte[] body, org.zerodds.internal.SourceIdentity related,
                long coherentSetName) {
            return write(body, related, coherentSetName, null, null);
        }

        /**
         * As above, for a Publisher with access scope GROUP: {@code group} gives the group sequence
         * number of the sample, the group sequence number of the first sample of its group coherent set
         * (0: none) and the digest of the writers of the Publisher (DDSI-RTPS 2.5 8.7.5, 9.6.4.3 to
         * 9.6.4.5).
         */
        public synchronized long write(byte[] body, org.zerodds.internal.SourceIdentity related,
                long coherentSetName, GroupStamp group) {
            return write(body, related, coherentSetName, group, null);
        }

        /**
         * As {@link #write(byte[], org.zerodds.internal.SourceIdentity)}, to the matched
         * readers that announced the service instance {@code instance} only (DDS-RPC 1.0
         * 7.11.1.4.3: "requests sent through a bound requester reference shall be sent to
         * the bound service instance only"); {@code null} is every matched reader. The
         * sequence number is consumed whoever is addressed.
         */
        public synchronized long write(byte[] body, org.zerodds.internal.SourceIdentity related,
                String instance) {
            return write(body, related, 0L, null, instance);
        }

        /** The sample of a coherent set and a group, to the readers of the service instance (or all: null). */
        public synchronized long write(byte[] body, org.zerodds.internal.SourceIdentity related,
                long coherentSetName, GroupStamp group, String instance) {
            ParameterList inline = null;
            if (related != null) {
                inline = new ParameterList().add(Rtps.PID_RELATED_SAMPLE_IDENTITY, related.toBytesLe());
            }
            if (coherentSetName != 0) {
                if (coherentName != coherentSetName) {
                    coherentName = coherentSetName;
                    coherentFirst = sn + 1;
                }
                if (inline == null) {
                    inline = new ParameterList();
                }
                inline.add(Rtps.PID_COHERENT_SET, CoherentSetParameter.encode(coherentFirst));
            }
            if (group != null) {
                if (inline == null) {
                    inline = new ParameterList();
                }
                group.addTo(inline);
            }
            byte[] payload = new byte[encap.length + body.length];
            System.arraycopy(encap, 0, payload, 0, encap.length);
            System.arraycopy(body, 0, payload, encap.length, body.length);
            return send(payload, inline, instance);
        }

        /**
         * The End Coherent Set DATA of a group coherent set (DDSI-RTPS 2.5 8.7.6): no serialized data,
         * the group sequence number one past the last sample of the set, the set, and the digest.
         */
        public synchronized void endGroupCoherentSet(GroupStamp group) {
            coherentName = 0;
            ParameterList inline = new ParameterList();
            group.addTo(inline);
            send(new byte[0], inline, null);
        }

        /**
         * Ends the coherent set {@code coherentSetName} for the readers if this writer sent samples
         * of it: a DATA without serialized data and without inline QoS (DDSI-RTPS 2.5 8.7.6,
         * Table 9.22, Example 3). The marker takes a sequence number.
         *
         * @return whether a marker was sent.
         */
        public synchronized boolean endCoherentSet(long coherentSetName) {
            if (coherentName != coherentSetName) {
                return false;
            }
            coherentName = 0;
            send(new byte[0], null, null);
            return true;
        }

        private long send(byte[] payload, ParameterList inline, String instance) {
            sn++;
            long now = System.currentTimeMillis();
            int secs = (int) (now / 1000L);
            long frac = ((now % 1000L) << 32) / 1000L;
            if (reliable) {
                ReliableWriterState state = reliableWriters.computeIfAbsent(hex(entityId), key -> new ReliableWriterState(entityId));
                List<ReliableWriterState.Target> targets = new ArrayList<>();
                ReliableWriterState.Sample sample = null;
                for (Map.Entry<String, MatchedReader> entry : matched.entrySet()) {
                    if (!addressed(entry.getKey(), instance)) {
                        continue;
                    }
                    MatchedReader reader = entry.getValue();
                    ReliableWriterState.Target target = new ReliableWriterState.Target(reader.participantPrefix, reader.readerEntityId, address(reader.locator));
                    sample = state.add(sn, payload, target, inline, inline == null ? 0L
                            : CoherentSetParameter.decode(inline.find(Rtps.PID_GROUP_SEQ_NUM)));
                    targets.add(target);
                }
                if (sample != null) for (ReliableWriterState.Target target : targets) sendHistory(state, sample, target);
                return sn;
            }
            for (Map.Entry<String, MatchedReader> entry : matched.entrySet()) {
                if (!addressed(entry.getKey(), instance)) {
                    continue;
                }
                MatchedReader mr = entry.getValue();
                RtpsMessage.Builder b = new RtpsMessage.Builder(guidPrefix)
                        .infoDestination(mr.participantPrefix)
                        .infoTimestamp(secs, frac)
                        .data(mr.readerEntityId, entityId, sn, inline, payload, false);
                sendTo(b.toBytes(), mr.locator);
            }
            return sn;
        }

        private int livelinessCount;

        /**
         * The DDS DataWriter asserted its liveliness manually: a HEARTBEAT with the L flag goes to
         * every matched reader, which renews the manual liveliness lease of this writer
         * (DDSI-RTPS 2.5 8.3.7.5). The range is that of the history, or empty.
         */
        public synchronized void assertLiveliness() {
            ReliableWriterState state = reliableWriters.get(hex(entityId));
            for (MatchedReader mr : matched.values()) {
                long first = sn + 1;
                long last = sn;
                int count;
                if (state != null) {
                    ReliableWriterState.Target target = new ReliableWriterState.Target(
                            mr.participantPrefix, mr.readerEntityId, address(mr.locator));
                    long[] range = state.range(target);
                    if (range[0] != 0) {
                        first = range[0];
                        last = range[1];
                    }
                    count = state.nextHeartbeatCount();
                } else {
                    count = ++livelinessCount;
                }
                sendTo(new RtpsMessage.Builder(guidPrefix).infoDestination(mr.participantPrefix)
                        .heartbeat(mr.readerEntityId, entityId, first, last, count, true, true)
                        .toBytes(), mr.locator);
            }
        }

        /** Whether the remote reader {@code key} is addressed by a write to {@code instance}. */
        private boolean addressed(String key, String instance) {
            if (instance == null) {
                return true;
            }
            DiscoveryData.Endpoint e = remoteReaders.get(key);
            return e != null && e.rpc.serviceInstanceName().equals(instance);
        }

        public int matchedCount() {
            return matched.size();
        }

        /**
         * Blocks until fewer than {@code maxSamples} samples of this writer are unacknowledged by
         * the readers it is matched with, or {@code timeoutNanos} have passed.
         *
         * @return whether there is room.
         */
        public boolean awaitHistorySpace(int maxSamples, long timeoutNanos) throws InterruptedException {
            ReliableWriterState state = reliableWriters.get(hex(entityId));
            return state == null || state.awaitSpace(maxSamples, timeoutNanos);
        }

        /**
         * How many matched remote readers announced the service instance {@code instance}
         * (DDS-RPC 1.0 §7.6.2.1.2).
         */
        public int matchedCount(String instance) {
            int count = 0;
            for (String key : matched.keySet()) {
                if (addressed(key, instance)) {
                    count++;
                }
            }
            return count;
        }

        /**
         * Blocks until every sample this writer sent has been acknowledged by the readers it
         * is matched with (ACKNACK, DDSI-RTPS 2.5 8.4.7.1), or {@code timeoutNanos} have passed.
         * A writer that is not reliable, or has sent nothing, has nothing outstanding.
         *
         * @return whether everything was acknowledged.
         */
        public boolean waitForAcknowledgments(long timeoutNanos) throws InterruptedException {
            ReliableWriterState state = reliableWriters.get(hex(entityId));
            return state == null || state.awaitAcknowledged(timeoutNanos);
        }
    }

    /** The group stamp of a sample or of an End Coherent Set DATA (DDSI-RTPS 2.5 9.6.4.3 to 9.6.4.5). */
    public static final class GroupStamp {
        final long groupSeq;
        final long setFirst;
        final byte[] digest;

        public GroupStamp(long groupSeq, long setFirst, byte[] digest) {
            this.groupSeq = groupSeq;
            this.setFirst = setFirst;
            this.digest = digest.clone();
        }

        void addTo(ParameterList inline) {
            inline.add(Rtps.PID_GROUP_SEQ_NUM, CoherentSetParameter.encode(groupSeq));
            if (setFirst > 0) {
                inline.add(Rtps.PID_GROUP_COHERENT_SET, CoherentSetParameter.encode(setFirst));
            }
            inline.add(Rtps.PID_WRITER_GROUP_INFO, digest);
        }
    }

    /** The liveliness of one remote writer as the reader sees it. */
    public static final class RemoteLiveliness {
        public final byte[] guid;
        public final boolean alive;

        RemoteLiveliness(byte[] guid, boolean alive) {
            this.guid = guid.clone();
            this.alive = alive;
        }
    }

    private static final int LIVELINESS_AUTOMATIC = 0;

    /** The last DATA or HEARTBEAT with the L flag of each remote writer, by GUID (ms). */
    private final Map<String, Long> writerActivity = new ConcurrentHashMap<>();

    private boolean aliveAt(String writerKey, DiscoveryData.Endpoint e, Long matchedAt, long now) {
        QosParameters.Liveliness liveliness = e.qos == null ? null : e.qos.liveliness;
        if (liveliness == null || liveliness.lease == null || liveliness.lease.isInfinite()) {
            return true;
        }
        long leaseMillis = liveliness.lease.seconds * 1000L + liveliness.lease.nanos() / 1_000_000L;
        long last = matchedAt == null ? 0L : matchedAt;
        Long activity = writerActivity.get(writerKey);
        if (activity != null) {
            last = Math.max(last, activity);
        }
        if (liveliness.kind == LIVELINESS_AUTOMATIC) {
            DiscoveryData.Participant peer = peers.get(writerKey.substring(0, 24));
            if (peer != null) {
                last = Math.max(last, peer.lastSeenMillis);
            }
        }
        return now - last <= leaseMillis;
    }

    /**
     * The remote writer {@code writerKey} showed it is alive (DATA, or HEARTBEAT with the L
     * flag). A reader that saw it not alive is told.
     */
    private void noteWriterAlive(String writerKey) {
        long now = System.currentTimeMillis();
        DiscoveryData.Endpoint e = remoteWriters.get(writerKey);
        boolean wasAlive = e == null;
        if (e != null) {
            for (WireReader r : readers) {
                if (r.matchedWriters.containsKey(writerKey)) {
                    wasAlive = aliveAt(writerKey, e, r.matchedAt.get(writerKey), now);
                    break;
                }
            }
        }
        writerActivity.put(writerKey, now);
        if (!wasAlive) {
            for (WireReader r : readers) {
                Runnable listener = r.livelinessListener;
                if (listener != null && r.matchedWriters.containsKey(writerKey)) {
                    listener.run();
                }
            }
        }
    }

    public final class WireReader {
        final byte[] entityId; // 4
        final byte[] guid; // 16
        final String topic;
        final String typeName;
        final boolean keyed;
        final Consumer<SerializedSample> onSample;
        final int[] dataRepresentations;
        final byte[] typeInformation;
        /** The QoS the reader announces (SEDP, DDS 1.4 §2.2.5); null: kinds only. */
        volatile QosParameters qos;
        /**
         * Decides on a remote writer of another type name (type coercion, TypeConsistencyEnforcement
         * DECLARED or ASSIGNABLE); {@code null}: only the same type name matches.
         */
        final java.util.function.Predicate<DiscoveryData.Endpoint> typeMatcher;
        /** As {@link WireWriter#byTypeInformation}. */
        final boolean byTypeInformation;
        // matched remote writer GUIDs (hex) that we accept DATA from
        final Map<String, Boolean> matchedWriters = new ConcurrentHashMap<>();
        /** The DDS-RPC 1.0 §7.6.2.1 extensions this reader announces. */
        volatile RpcDiscovery rpc = RpcDiscovery.NONE;

        /**
         * Announces the DDS-RPC 1.0 §7.6.2.1 discovery extensions of this reader and matches
         * again with what is known.
         */
        public void setRpcDiscovery(RpcDiscovery value) {
            rpc = value == null ? RpcDiscovery.NONE : value;
            synchronized (RtpsParticipant.this) {
                for (DiscoveryData.Endpoint e : new ArrayList<>(remoteWriters.values())) {
                    tryMatchReader(this, e);
                }
            }
            announceAllToPeers();
        }

        /** The extensions this reader announces. */
        public RpcDiscovery rpcDiscovery() {
            return rpc;
        }

        WireReader(byte[] entityId, String topic, String typeName, boolean keyed,
                   int[] dataRepresentations, byte[] typeInformation, Consumer<SerializedSample> onSample,
                   QosParameters qos, java.util.function.Predicate<DiscoveryData.Endpoint> typeMatcher,
                   boolean byTypeInformation) {
            this.typeMatcher = typeMatcher;
            this.byTypeInformation = byTypeInformation;
            this.qos = qos;
            this.entityId = entityId;
            this.guid = new Guid(guidPrefix, entityId).bytes();
            this.topic = topic;
            this.typeName = typeName;
            this.keyed = keyed;
            this.onSample = onSample;
            this.dataRepresentations = DdsWireBridge.validateRepresentations(dataRepresentations);
            this.typeInformation = requireTypeInformation(typeInformation);
        }

        /** As {@link WireWriter#updateQos}, for the DDS reader. */
        public void updateQos(QosParameters changed) {
            this.qos = changed;
            announceAllToPeers();
        }

        /** When each remote writer was matched (ms): the first evidence of its liveliness. */
        final Map<String, Long> matchedAt = new ConcurrentHashMap<>();
        /** Called when a remote writer that had lost its liveliness shows it again; may be {@code null}. */
        private volatile Runnable livelinessListener;

        /** The reader wants to hear when a remote writer is alive again after it was not. */
        public void setLivelinessListener(Runnable listener) {
            this.livelinessListener = listener;
        }

        /**
         * The liveliness of the matched remote writers (DDS 1.4 2.2.3.11): a writer is alive while
         * the time since the last evidence is within the lease it announced. Evidence is a DATA or a
         * HEARTBEAT with the L flag of the writer and, for AUTOMATIC liveliness, a message of its
         * participant (SPDP). A writer that announced no liveliness is AUTOMATIC with an infinite lease.
         */
        public List<RemoteLiveliness> remoteLiveliness() {
            List<RemoteLiveliness> out = new ArrayList<>();
            long now = System.currentTimeMillis();
            for (String key : matchedWriters.keySet()) {
                DiscoveryData.Endpoint e = remoteWriters.get(key);
                if (e == null) {
                    continue;
                }
                out.add(new RemoteLiveliness(e.endpointGuid, aliveAt(key, e, matchedAt.get(key), now)));
            }
            return out;
        }

        /** The samples of coherent sets of remote writers, held until the set is complete. */
        final CoherentHold hold = new CoherentHold(RtpsParticipant::log);

        /** Whether the Subscriber of this reader has access scope GROUP (PRESENTATION). */
        boolean groupScope() {
            return qos != null && qos.presentation != null && qos.presentation.accessScope == 2;
        }

        /** Whether the Subscriber of this reader asks for coherent access (PRESENTATION). */
        boolean coherent() {
            return qos != null && qos.presentation != null && qos.presentation.coherentAccess;
        }

        /** Hands on the samples of a complete coherent set together, as one step towards the readers. */
        void release(java.util.List<SerializedSample> samples) {
            if (samples.isEmpty()) {
                return;
            }
            org.zerodds.internal.InProcessBus.instance().atomically(() -> {
                for (SerializedSample s : samples) {
                    try {
                        onSample.accept(s);
                    } catch (Exception ex) {
                        log("reader callback threw: " + ex);
                    }
                }
            });
        }

        /** The GUID of this reader. */
        public byte[] guid() {
            return guid.clone();
        }

        public int matchedCount() {
            return matchedWriters.size();
        }

        /**
         * Whether the historical data of the matched writers has arrived (DDS 1.4
         * §2.2.2.5.3.32): every matched remote writer that is reliable and not VOLATILE has
         * announced its last sequence number in a HEARTBEAT, and this reader has received
         * the samples up to it.
         */
        public boolean historicalDataReceived() {
            if (deliveriesInFlight.get() > 0) {
                return false; // received, not yet handed to the readers
            }
            for (DiscoveryData.Endpoint writer : matchedEndpoints()) {
                if (writer.durabilityKind == Rtps.DURABILITY_VOLATILE
                        || writer.reliabilityKind != Rtps.RELIABILITY_RELIABLE) {
                    continue;
                }
                String key = hex(writer.endpointGuid) + ":" + hex(entityId);
                Long last = announcedLast.get(key);
                if (last == null) {
                    return false;
                }
                ReliableReceiveState state = reliableReaders.get(key);
                long next = state == null ? 1 : state.base();
                if (next <= last) {
                    return false;
                }
            }
            return true;
        }

        /** The remote writers this reader is matched with (SEDP). */
        public List<DiscoveryData.Endpoint> matchedEndpoints() {
            List<DiscoveryData.Endpoint> out = new ArrayList<>();
            for (String guid : matchedWriters.keySet()) {
                DiscoveryData.Endpoint e = remoteWriters.get(guid);
                if (e != null) {
                    out.add(e);
                }
            }
            return out;
        }
    }

    static final class MatchedReader {
        final byte[] participantPrefix;
        final byte[] readerEntityId;
        final Locator locator;

        MatchedReader(byte[] participantPrefix, byte[] readerEntityId, Locator locator) {
            this.participantPrefix = participantPrefix;
            this.readerEntityId = readerEntityId;
            this.locator = locator;
        }
    }

    // ---- discovery observers (DDS 1.4 §2.2.5 built-in topics) ----

    /**
     * Told about the participants and endpoints SPDP and SEDP discover, with
     * the QoS they announce, and about their loss. The callbacks run on the
     * discovery thread; they must not block.
     */
    public interface DiscoveryListener {
        void participantDiscovered(DiscoveryData.Participant participant);

        void participantLost(DiscoveryData.Participant participant);

        void endpointDiscovered(DiscoveryData.Endpoint endpoint);

        void endpointLost(DiscoveryData.Endpoint endpoint);
    }

    private final List<DiscoveryListener> discoveryListeners =
            new java.util.concurrent.CopyOnWriteArrayList<>();

    /**
     * Registers {@code listener}; what has been discovered so far is
     * reported to it first, so it misses nothing.
     */
    public synchronized void addDiscoveryListener(DiscoveryListener listener) {
        discoveryListeners.add(listener);
        for (DiscoveryData.Participant p : peers.values()) {
            deliver(listener, l -> l.participantDiscovered(p));
        }
        for (DiscoveryData.Endpoint e : remoteWriters.values()) {
            deliver(listener, l -> l.endpointDiscovered(e));
        }
        for (DiscoveryData.Endpoint e : remoteReaders.values()) {
            deliver(listener, l -> l.endpointDiscovered(e));
        }
    }

    /** Reports to every listener; a listener that throws is logged and does not stop discovery. */
    private void fire(Consumer<DiscoveryListener> report) {
        for (DiscoveryListener listener : discoveryListeners) {
            deliver(listener, report);
        }
    }

    private void deliver(DiscoveryListener listener, Consumer<DiscoveryListener> report) {
        try {
            report.accept(listener);
        } catch (RuntimeException e) {
            log("discovery listener threw: " + e);
        }
    }

    public void removeDiscoveryListener(DiscoveryListener listener) {
        discoveryListeners.remove(listener);
    }

    /** The GUID prefix of this participant (12 octets). */
    public byte[] participantPrefix() {
        return guidPrefix.clone();
    }

    private volatile byte[] userData = new byte[0];

    /**
     * The USER_DATA this participant announces on SPDP (DDS 1.4 §2.2.5
     * ParticipantBuiltinTopicData). The RTPS participant is shared by the
     * DDS participants of a domain in this JVM; the value announced is the
     * one set last.
     */
    public void setUserData(byte[] value) {
        this.userData = value == null ? new byte[0] : value.clone();
    }

    // ---- state ----
    private final int domainId;
    private final int participantId;
    private final byte[] guidPrefix; // 12
    private final AtomicInteger entityKeyCounter = new AtomicInteger(1);

    private final List<WireWriter> writers = new ArrayList<>();
    private final List<WireReader> readers = new ArrayList<>();

    private final Map<String, DiscoveryData.Participant> peers = new ConcurrentHashMap<>();
    private final Map<String, DiscoveryData.Endpoint> remoteWriters = new ConcurrentHashMap<>();
    private final Map<String, DiscoveryData.Endpoint> remoteReaders = new ConcurrentHashMap<>();
    // Reliable builtin-reader bookkeeping: highest SN received + ACKNACK count
    // per remote builtin writer GUID (SPDP/SEDP).
    private final Map<String, Integer> ackCounters = new ConcurrentHashMap<>();
    private final Map<String, ReliableWriterState> reliableWriters = new ConcurrentHashMap<>();
    private final Map<String, ReliableReceiveState> reliableReaders = new ConcurrentHashMap<>();
    /** The highest last sequence number a writer announced in HEARTBEAT, by writer:reader. */
    private final Map<String, Long> announcedLast = new ConcurrentHashMap<>();
    private final FragmentReassembly fragments = new FragmentReassembly();
    private final TypeLookupService typeLookup = new TypeLookupService();
    private final java.util.concurrent.atomic.AtomicLong typeLookupSequence = new java.util.concurrent.atomic.AtomicLong();
    private final java.util.concurrent.atomic.AtomicLong typeLookupReplySequence = new java.util.concurrent.atomic.AtomicLong();
    private static final class TypeQuery {
        final String peer;
        final int operation;
        final java.util.concurrent.CompletableFuture<DDS.Builtin.TypeLookup_Reply> future;
        final long deadline = System.nanoTime() + java.util.concurrent.TimeUnit.SECONDS.toNanos(3);
        TypeQuery(String peer, int operation, java.util.concurrent.CompletableFuture<DDS.Builtin.TypeLookup_Reply> future) {
            this.peer = peer; this.operation = operation; this.future = future;
        }
    }
    private final Map<Long, TypeQuery> typeQueries = new ConcurrentHashMap<>();

    public void registerTypeObjects(org.zerodds.cdr.TypeObjectRegistry registry) {
        typeLookup.register(registry);
    }

    /** Correlated service query through discovered standard builtin endpoints. */
    public synchronized java.util.concurrent.CompletableFuture<DDS.Builtin.TypeLookup_Reply>
            queryTypeLookup(byte[] peerPrefix, DDS.Builtin.TypeLookup_Call call) {
        if (typeQueries.size() >= 256) throw new IllegalStateException("TypeLookup pending request limit");
        DiscoveryData.Participant peer = peers.get(hex(peerPrefix));
        if (peer == null) throw new IllegalArgumentException("Undiscovered TypeLookup peer");
        long serverEndpoints = Rtps.EP_TYPELOOKUP_REQUEST_READER | Rtps.EP_TYPELOOKUP_REPLY_WRITER;
        if ((peer.builtinEndpointSet & serverEndpoints) != serverEndpoints)
            throw new IllegalArgumentException("Peer does not advertise TypeLookup server endpoints");
        Locator target = peer.metatrafficUnicast != null ? peer.metatrafficUnicast : peer.defaultUnicast;
        if (target == null) throw new IllegalArgumentException("TypeLookup peer has no unicast locator");
        long sequence = typeLookupSequence.incrementAndGet();
        DDS.Builtin.TypeLookup_Request request = new DDS.Builtin.TypeLookup_Request();
        request.header = new DDS.RPC.RequestHeader();
        request.header.requestId = TypeLookupService.identity(new Guid(guidPrefix, Guid.TL_REQ_WRITER).bytes(), sequence);
        request.header.instanceName = "dds.builtin.TOS." + hex(new Guid(peerPrefix, Guid.ENTITYID_PARTICIPANT).bytes());
        request.data = call;
        byte[] payload = typeLookupPayload(TypeLookupService.encodeRequest(request, typeLookupEncoding));
        java.util.concurrent.CompletableFuture<DDS.Builtin.TypeLookup_Reply> future = new java.util.concurrent.CompletableFuture<>();
        typeQueries.put(sequence, new TypeQuery(hex(peerPrefix), call._d, future));
        future.whenComplete((reply, error) -> typeQueries.remove(sequence));
        publishReliable(Guid.TL_REQ_WRITER, Guid.TL_REQ_READER, peerPrefix, address(target), sequence, payload);
        return future;
    }

    /** {@code wire.type_lookup_encoding}; see {@link TypeLookupService.Encoding}. */
    private final TypeLookupService.Encoding typeLookupEncoding = TypeLookupService.Encoding.configured();

    private byte[] typeLookupPayload(byte[] body) {
        if (body.length > 16 * 1024 * 1024 - 8) throw new IllegalArgumentException("TypeLookup sample exceeds receive resource limit");
        return TypeLookupService.payload(body, typeLookupEncoding);
    }

    private void onTypeLookup(byte[] sender, RtpsMessage.Data data, java.net.SocketAddress source) {
        if (data.destPrefix != null && !equalsBytes(data.destPrefix, guidPrefix)) return;
        try {
            // The identifier names the encoding: CDR (XCDR1) or CDR2 (XCDR2).
            TypeLookupService.Encapsulation encapsulation = TypeLookupService.Encapsulation.read(data.serializedPayload);
            org.zerodds.wire.Endianness endian = encapsulation.endian;
            byte[] body = encapsulation.body;
            if (equalsBytes(data.writerId, Guid.TL_REQ_WRITER)) {
                if (!equalsBytes(data.readerId, Guid.TL_REQ_READER)) return;
                DDS.Builtin.TypeLookup_Request request = TypeLookupService.decodeRequest(body, endian, encapsulation.encoding);
                if (!equalsBytes(TypeLookupService.guid(request.header.requestId), new Guid(sender, Guid.TL_REQ_WRITER).bytes()))
                    throw new IllegalArgumentException("TypeLookup request writer identity mismatch");
                String instance = "dds.builtin.TOS." + hex(new Guid(guidPrefix, Guid.ENTITYID_PARTICIPANT).bytes());
                if (!instance.equals(request.header.instanceName)
                        && !"dds.builtin.TOS.00000000000000000000000000000000".equals(request.header.instanceName)) return;
                byte[] reply = typeLookupPayload(TypeLookupService.encodeReply(typeLookup.handle(request), typeLookupEncoding));
                DiscoveryData.Participant peer = peers.get(hex(sender));
                Locator target = peer == null ? null : (peer.metatrafficUnicast != null ? peer.metatrafficUnicast : peer.defaultUnicast);
                InetSocketAddress destination = target != null ? address(target)
                        : (source instanceof InetSocketAddress ? (InetSocketAddress) source : null);
                if (destination != null) publishReliable(Guid.TL_REPLY_WRITER, Guid.TL_REPLY_READER,
                        sender, destination, typeLookupReplySequence.incrementAndGet(), reply);
            } else {
                if (!equalsBytes(data.readerId, Guid.TL_REPLY_READER)) return;
                DDS.Builtin.TypeLookup_Reply reply = TypeLookupService.decodeReply(body, endian, encapsulation.encoding);
                if (!equalsBytes(TypeLookupService.guid(reply.header.relatedRequestId), new Guid(guidPrefix, Guid.TL_REQ_WRITER).bytes()))
                    throw new IllegalArgumentException("TypeLookup related writer identity mismatch");
                TypeQuery pending = typeQueries.get(TypeLookupService.sequence(reply.header.relatedRequestId));
                if (pending == null || !pending.peer.equals(hex(sender)) || pending.operation != reply.return_data._d) return;
                if (reply.header.remoteEx.value() != DDS.RPC.RemoteExceptionCode._REMOTE_EX_OK)
                    pending.future.completeExceptionally(new IllegalStateException("TypeLookup remote exception " + reply.header.remoteEx.value()));
                else pending.future.complete(reply);
            }
        } catch (RuntimeException error) {
            log("TypeLookup refused: " + error);
        }
    }

    private DatagramChannel spdpChannel; // multicast recv + send
    private DatagramChannel unicastChannel; // metatraffic + user recv/send
    private Locator announceLocator; // what we advertise as meta + default unicast
    private final List<NetworkInterface> mcastIfs = new ArrayList<>();
    private volatile boolean running = false;
    private long spdpWriterSn = 0;

    private synchronized long nextSpdpSn() {
        return ++spdpWriterSn;
    }
    private long sedpPubSn = 0;
    private long sedpSubSn = 0;

    private RtpsParticipant(int domainId) {
        this.domainId = domainId;
        this.guidPrefix = new byte[12];
        Random rnd = new Random();
        // bytes 0..2 = vendor id (interop-friendly), 2..8 host/pid, 8..12 random
        guidPrefix[0] = Rtps.VENDOR_ID[0];
        guidPrefix[1] = Rtps.VENDOR_ID[1];
        int host;
        try {
            host = InetAddress.getLocalHost().getHostName().hashCode();
        } catch (Exception e) {
            host = rnd.nextInt();
        }
        guidPrefix[2] = (byte) (host >>> 24);
        guidPrefix[3] = (byte) (host >>> 16);
        long pidTag = java.lang.management.ManagementFactory.getRuntimeMXBean().getName().hashCode();
        guidPrefix[4] = (byte) (pidTag >>> 8);
        guidPrefix[5] = (byte) pidTag;
        for (int i = 6; i < 12; i++) {
            guidPrefix[i] = (byte) rnd.nextInt();
        }
        this.participantId = start();
    }

    public byte[] guidPrefix() {
        return guidPrefix.clone();
    }

    // ---- socket + thread setup ----

    private int start() {
        try {
            String bindHost = System.getProperty("zerodds.rtps.bind", "0.0.0.0");
            String announceHost = System.getProperty("zerodds.rtps.host", primaryIpv4());

            // SPDP multicast socket, bound to the domain's multicast port.
            int mcastPort = Rtps.spdpMulticastPort(domainId);
            spdpChannel = DatagramChannel.open(StandardProtocolFamily.INET);
            spdpChannel.setOption(StandardSocketOptions.SO_REUSEADDR, true);
            spdpChannel.bind(new InetSocketAddress(mcastPort));
            spdpChannel.setOption(StandardSocketOptions.IP_MULTICAST_LOOP, true);
            InetAddress group = InetAddress.getByName(Rtps.SPDP_MULTICAST_ADDRESS);
            for (NetworkInterface nif : multicastInterfaces()) {
                try {
                    spdpChannel.join(group, nif);
                    mcastIfs.add(nif); // remember for per-interface beacon egress
                } catch (Exception ignore) {
                    // interface may not support the group; keep going
                }
            }

            // Unicast socket: probe participant ids for a free metatraffic port.
            int pid = 0;
            for (; pid < 120; pid++) {
                int port = Rtps.metatrafficUnicastPort(domainId, pid);
                try {
                    unicastChannel = DatagramChannel.open(StandardProtocolFamily.INET);
                    // No SO_REUSEADDR here: each participant must own a distinct
                    // unicast port, so the probe advances on a collision.
                    unicastChannel.bind(new InetSocketAddress(bindHost, port));
                    break;
                } catch (IOException busy) {
                    if (unicastChannel != null) {
                        unicastChannel.close();
                        unicastChannel = null;
                    }
                }
            }
            if (unicastChannel == null) {
                throw new IOException("no free RTPS unicast port");
            }
            int boundPort = ((InetSocketAddress) unicastChannel.getLocalAddress()).getPort();
            announceLocator = Locator.udpV4(announceHost, boundPort);

            running = true;
            Thread t1 = new Thread(this::spdpRecvLoop, "rtps-spdp-" + domainId);
            t1.setDaemon(true);
            t1.start();
            Thread t2 = new Thread(this::unicastRecvLoop, "rtps-unicast-" + domainId);
            t2.setDaemon(true);
            t2.start();
            Thread t3 = new Thread(this::beaconLoop, "rtps-beacon-" + domainId);
            t3.setDaemon(true);
            t3.start();

            log("participant up: domain=" + domainId + " prefix=" + hex(guidPrefix)
                    + " unicast=" + announceLocator + " spdp-mcast=" + mcastPort);
            return pid;
        } catch (IOException e) {
            throw new RuntimeException("RTPS participant start failed", e);
        }
    }

    private static List<NetworkInterface> multicastInterfaces() {
        List<NetworkInterface> out = new ArrayList<>();
        try {
            Enumeration<NetworkInterface> e = NetworkInterface.getNetworkInterfaces();
            while (e.hasMoreElements()) {
                NetworkInterface nif = e.nextElement();
                if (nif.isUp() && nif.supportsMulticast()) {
                    out.add(nif);
                }
            }
        } catch (Exception ignore) {
            // fall through
        }
        return out;
    }

    // ---- public API ----

    public synchronized WireWriter createWriter(String topic, String typeName, byte[] encap,
                                                byte[] typeId, boolean keyed, boolean reliable) {
        return createWriter(topic, typeName, encap, typeId, new byte[0], keyed, reliable);
    }

    public synchronized WireWriter createWriter(String topic, String typeName, byte[] encap,
                                                byte[] typeId, byte[] typeInformation, boolean keyed, boolean reliable) {
        return createWriter(topic, typeName, encap, typeId, typeInformation, keyed, reliable, null);
    }

    /**
     * A writer that announces {@code qos} on SEDP (DDS 1.4 §2.2.5, DDSI-RTPS
     * 2.5 Table 9.13) and is matched only with remote readers whose requested
     * QoS its offered QoS satisfies (§2.2.3).
     */
    public synchronized WireWriter createWriter(String topic, String typeName, byte[] encap,
                                                byte[] typeId, byte[] typeInformation, boolean keyed, boolean reliable,
                                                QosParameters qos) {
        return createWriter(topic, typeName, encap, typeId, typeInformation, keyed, reliable, qos, null);
    }

    /** A writer that also matches the remote readers {@code typeMatcher} accepts (type coercion). */
    public synchronized WireWriter createWriter(String topic, String typeName, byte[] encap,
                                                byte[] typeId, byte[] typeInformation, boolean keyed, boolean reliable,
                                                QosParameters qos,
                                                java.util.function.Predicate<DiscoveryData.Endpoint> typeMatcher) {
        return createWriter(topic, typeName, encap, typeId, typeInformation, keyed, reliable, qos,
                typeMatcher, false);
    }

    /**
     * A writer whose type pairs by the assignability of the TypeInformation it and the remote
     * readers announce when {@code byTypeInformation} (XTypes 1.3 7.6.3.4.2), else as above.
     */
    public synchronized WireWriter createWriter(String topic, String typeName, byte[] encap,
                                                byte[] typeId, byte[] typeInformation, boolean keyed, boolean reliable,
                                                QosParameters qos,
                                                java.util.function.Predicate<DiscoveryData.Endpoint> typeMatcher,
                                                boolean byTypeInformation) {
        int k = entityKeyCounter.getAndIncrement();
        byte[] eid = Guid.userEntityId(0, k >>> 8, k,
                keyed ? Guid.KIND_USER_WRITER_WITH_KEY : Guid.KIND_USER_WRITER_NO_KEY);
        WireWriter w = new WireWriter(eid, topic, typeName, encap, typeId, typeInformation, keyed, reliable, qos,
                typeMatcher, byTypeInformation);
        writers.add(w);
        // match against already-discovered remote readers
        for (DiscoveryData.Endpoint r : remoteReaders.values()) {
            tryMatchWriter(w, r);
        }
        announceAllToPeers();
        return w;
    }

    public synchronized WireReader createReader(String topic, String typeName, boolean keyed,
                                                Consumer<byte[]> onSample) {
        // Raw callback compatibility: the caller owns decoding for either representation.
        return createTypedReader(topic, typeName, keyed,
                new int[] {Rtps.DATA_REP_XCDR1, Rtps.DATA_REP_XCDR2}, sample -> onSample.accept(sample.body()));
    }

    public synchronized WireReader createTypedReader(String topic, String typeName, boolean keyed,
            int[] dataRepresentations, Consumer<SerializedSample> onSample) {
        return createTypedReader(topic, typeName, keyed, dataRepresentations, new byte[0], onSample);
    }

    public synchronized WireReader createTypedReader(String topic, String typeName, boolean keyed,
            int[] dataRepresentations, byte[] typeInformation, Consumer<SerializedSample> onSample) {
        return createTypedReader(topic, typeName, keyed, dataRepresentations, typeInformation,
                onSample, null);
    }

    /** A reader that announces {@code qos} on SEDP; see {@link #createWriter}. */
    public synchronized WireReader createTypedReader(String topic, String typeName, boolean keyed,
            int[] dataRepresentations, byte[] typeInformation, Consumer<SerializedSample> onSample,
            QosParameters qos) {
        return createTypedReader(topic, typeName, keyed, dataRepresentations, typeInformation,
                onSample, qos, null);
    }

    /** A reader that also matches the remote writers {@code typeMatcher} accepts (type coercion). */
    public synchronized WireReader createTypedReader(String topic, String typeName, boolean keyed,
            int[] dataRepresentations, byte[] typeInformation, Consumer<SerializedSample> onSample,
            QosParameters qos, java.util.function.Predicate<DiscoveryData.Endpoint> typeMatcher) {
        return createTypedReader(topic, typeName, keyed, dataRepresentations, typeInformation, onSample,
                qos, typeMatcher, false);
    }

    /** As {@link #createWriter(String, String, byte[], byte[], byte[], boolean, boolean, QosParameters, java.util.function.Predicate, boolean)}, for a reader. */
    public synchronized WireReader createTypedReader(String topic, String typeName, boolean keyed,
            int[] dataRepresentations, byte[] typeInformation, Consumer<SerializedSample> onSample,
            QosParameters qos, java.util.function.Predicate<DiscoveryData.Endpoint> typeMatcher,
            boolean byTypeInformation) {
        int k = entityKeyCounter.getAndIncrement();
        byte[] eid = Guid.userEntityId(0, k >>> 8, k,
                keyed ? Guid.KIND_USER_READER_WITH_KEY : Guid.KIND_USER_READER_NO_KEY);
        WireReader r = new WireReader(eid, topic, typeName, keyed, dataRepresentations, typeInformation, onSample, qos,
                typeMatcher, byTypeInformation);
        readers.add(r);
        for (DiscoveryData.Endpoint w : remoteWriters.values()) {
            tryMatchReader(r, w);
        }
        announceAllToPeers();
        return r;
    }

    private static byte[] requireTypeInformation(byte[] value) {
        if (value == null || value.length > 65532) throw new IllegalArgumentException("Invalid TypeInformation parameter length");
        return value.clone();
    }

    // ---- SPDP ----

    private void beaconLoop() {
        long nextSpdp = 0;
        long nextHeartbeat = 0;
        while (running) {
            try {
                long tick = System.currentTimeMillis();
                if (tick >= nextSpdp) { // the SPDP resend period (DDSI-RTPS 2.5 9.6.2.4: 30 s)
                    sendSpdpBeacon();
                    nextSpdp = tick + TIMING.spdpPeriodMillis;
                }
                fragments.expire();
                groupOrder.expire(System.currentTimeMillis());
                long now = System.currentTimeMillis();
                for (Map.Entry<String, DiscoveryData.Participant> entry : peers.entrySet()) {
                    DiscoveryData.Participant peer = entry.getValue();
                    if (peer.leaseSeconds < 0 || now - peer.lastSeenMillis <= peer.leaseSeconds * 1000L) continue;
                    if (!peers.remove(entry.getKey(), peer)) continue;
                    String prefix = entry.getKey();
                    for (ReliableWriterState writer : reliableWriters.values()) writer.removePeer(prefix);
                    reliableReaders.keySet().removeIf(key -> key.startsWith(prefix));
                    ackCounters.keySet().removeIf(key -> key.startsWith(prefix));
                    typeObjectAttempts.keySet().removeIf(key -> key.startsWith(prefix + ":"));
                    List<DiscoveryData.Endpoint> lost = new ArrayList<>();
                    for (Map<String, DiscoveryData.Endpoint> remote : java.util.Arrays.asList(
                            remoteWriters, remoteReaders)) {
                        for (Map.Entry<String, DiscoveryData.Endpoint> endpoint : remote.entrySet()) {
                            if (endpoint.getKey().startsWith(prefix)
                                    && remote.remove(endpoint.getKey(), endpoint.getValue())) {
                                lost.add(endpoint.getValue());
                            }
                        }
                    }
                    for (DiscoveryData.Endpoint endpoint : lost) {
                        fire(listener -> listener.endpointLost(endpoint));
                    }
                    fire(listener -> listener.participantLost(peer));
                    for (WireWriter writer : writers) writer.matched.keySet().removeIf(key -> key.startsWith(prefix));
                    for (WireReader reader : readers) reader.matchedWriters.keySet().removeIf(key -> key.startsWith(prefix));
                }
                if (tick >= nextHeartbeat) {
                    nextHeartbeat = tick + TIMING.heartbeatPeriodMillis;
                    for (ReliableWriterState writer : reliableWriters.values())
                        for (ReliableWriterState.Target target : writer.targets()) sendHeartbeat(writer, target);
                    sendIdleGroupHeartbeats();
                }
                for (TypeQuery query : typeQueries.values()) if (System.nanoTime() >= query.deadline)
                    query.future.completeExceptionally(new java.util.concurrent.TimeoutException("TypeLookup request expired"));
            } catch (Exception e) {
                log("beacon send failed: " + e);
            }
            try {
                Thread.sleep(Math.min(100L, Math.min(TIMING.heartbeatPeriodMillis, TIMING.spdpPeriodMillis)));
            } catch (InterruptedException e) {
                return;
            }
        }
    }

    private void sendSpdpBeacon() throws IOException {
        byte[] payload = DiscoveryData.buildParticipant(guidPrefix, domainId,
                announceLocator, announceLocator, Rtps.EP_ALL_BASIC | Rtps.EP_TYPELOOKUP_ALL, 100,
                userData);
        long spdpSn = nextSpdpSn();
        RtpsMessage.Builder b = new RtpsMessage.Builder(guidPrefix)
                .infoTimestamp((int) (System.currentTimeMillis() / 1000L), 0)
                .data(Guid.ENTITYID_UNKNOWN, Guid.SPDP_WRITER, spdpSn, null, payload, false);
        byte[] dg = b.toBytes();
        InetSocketAddress groupAddr = new InetSocketAddress(
                InetAddress.getByName(Rtps.SPDP_MULTICAST_ADDRESS).getHostAddress(),
                Rtps.spdpMulticastPort(domainId));
        // Egress the beacon on every joined multicast interface — the peer may
        // listen on a different one than our default multicast route.
        for (NetworkInterface nif : mcastIfs) {
            try {
                spdpChannel.setOption(StandardSocketOptions.IP_MULTICAST_IF, nif);
                spdpChannel.send(ByteBuffer.wrap(dg), groupAddr);
            } catch (Exception ignore) {
                // interface may not route the group; try the next
            }
        }
    }

    /** First non-loopback IPv4 address, or 127.0.0.1. Advertised so peers reach us. */
    private static String primaryIpv4() {
        try {
            for (NetworkInterface nif : multicastInterfaces()) {
                if (nif.isLoopback()) {
                    continue;
                }
                Enumeration<InetAddress> addrs = nif.getInetAddresses();
                while (addrs.hasMoreElements()) {
                    InetAddress a = addrs.nextElement();
                    if (a.getAddress().length == 4 && !a.isLoopbackAddress()) {
                        return a.getHostAddress();
                    }
                }
            }
        } catch (Exception ignore) {
            // fall through
        }
        return "127.0.0.1";
    }

    private void spdpRecvLoop() {
        ByteBuffer buf = ByteBuffer.allocate(65536);
        while (running) {
            try {
                buf.clear();
                java.net.SocketAddress src = spdpChannel.receive(buf);
                buf.flip();
                byte[] arr = new byte[buf.remaining()];
                buf.get(arr);
                handleDatagram(arr, arr.length, src);
            } catch (Exception e) {
                if (running) {
                    // transient; keep going
                }
            }
        }
    }

    private void unicastRecvLoop() {
        ByteBuffer buf = ByteBuffer.allocate(65536);
        while (running) {
            try {
                buf.clear();
                java.net.SocketAddress src = unicastChannel.receive(buf);
                buf.flip();
                byte[] arr = new byte[buf.remaining()];
                buf.get(arr);
                handleDatagram(arr, arr.length, src);
            } catch (Exception e) {
                if (running) {
                    // transient; keep going
                }
            }
        }
    }

    private static final boolean DEBUG =
            Boolean.parseBoolean(System.getProperty("zerodds.rtps.debug", "false"));

    private void handleDatagram(byte[] arr, int len, java.net.SocketAddress src) {
        deliveriesInFlight.incrementAndGet();
        try {
            handleDatagramCounted(arr, len, src);
        } finally {
            deliveriesInFlight.decrementAndGet();
        }
    }

    private void handleDatagramCounted(byte[] arr, int len, java.net.SocketAddress src) {
        RtpsMessage.Parsed p = RtpsMessage.decode(arr, len);
        if (p == null || p.header.guidPrefix == null) {
            if (DEBUG) {
                log("RX undecodable dg len=" + len);
            }
            return;
        }
        if (equalsBytes(p.header.guidPrefix, guidPrefix)) {
            return; // our own multicast loopback
        }
        if (DEBUG) {
            StringBuilder sb = new StringBuilder("RX dg len=" + len + " from "
                    + hex(p.header.guidPrefix) + " datas=" + p.data.size());
            for (RtpsMessage.Data d : p.data) {
                sb.append(" [w=").append(hex(d.writerId)).append(" r=").append(hex(d.readerId))
                        .append(" sn=").append(d.writerSn)
                        .append(" plen=").append(d.serializedPayload.length);
                if (d.serializedPayload.length >= 2) {
                    sb.append(" encap=").append(String.format("%02x%02x",
                            d.serializedPayload[0], d.serializedPayload[1]));
                }
                sb.append("]");
            }
            log(sb.toString());
        }
        for (RtpsMessage.Data d : p.data) receiveData(p.header.guidPrefix, d, src);
        for (RtpsMessage.DataFragment fragment : p.dataFragments) {
            if (!addressedHere(fragment.destPrefix)) continue;
            RtpsMessage.Data complete = fragments.accept(p.header.guidPrefix, fragment);
            if (complete != null) receiveData(p.header.guidPrefix, complete, src);
        }
        for (RtpsMessage.AckNack ack : p.ackNacks) onAckNack(p.header.guidPrefix, ack, src);
        for (RtpsMessage.NackFragment nack : p.nackFragments) onNackFragment(p.header.guidPrefix, nack, src);
        for (RtpsMessage.HeartbeatFragment heartbeat : p.heartbeatFragments) {
            if (!addressedHere(heartbeat.destPrefix) || !(src instanceof InetSocketAddress)) continue;
            for (byte[] reader : addressedReaders(p.header.guidPrefix, heartbeat.writerId, heartbeat.readerId)) {
            if (receiveState(p.header.guidPrefix, heartbeat.writerId, reader).base() > heartbeat.writerSn) continue;
            long base = fragments.missing(p.header.guidPrefix, heartbeat.writerId, heartbeat.writerSn);
            if (base <= heartbeat.lastFragmentNum) {
                String key = hex(new Guid(p.header.guidPrefix, heartbeat.writerId).bytes()) + ":" + hex(reader);
                int count = ackCounters.merge(key + ":fragment", 1, Integer::sum);
                sendToAddr(new RtpsMessage.Builder(guidPrefix).infoDestination(p.header.guidPrefix)
                    .nackFrag(reader, heartbeat.writerId, heartbeat.writerSn, base,
                        (int) Math.min(256, heartbeat.lastFragmentNum - base + 1), count).toBytes(), (InetSocketAddress) src);
            }
            }
        }
        for (RtpsMessage.Gap gap : p.gaps) {
            if (!addressedHere(gap.destPrefix)) continue;
            for (byte[] reader : receivingReaders(p.header.guidPrefix, gap.writerId, gap.readerId)) {
            ReliableReceiveState state = receiveState(p.header.guidPrefix, gap.writerId, reader);
            for (RtpsMessage.Data ready : state.gap(gap.gapStart, gap.gapListBase)) dispatchReceivedData(p.header.guidPrefix, ready, src);
            for (int bit = 0; bit < gap.numBits; bit++) {
                if ((gap.bitmap[bit / 32] & (1L << (31 - bit % 32))) != 0) {
                    long sequence = gap.gapListBase + bit;
                    for (RtpsMessage.Data ready : state.gap(sequence, sequence + 1)) dispatchReceivedData(p.header.guidPrefix, ready, src);
                }
            }
            }
        }
        for (RtpsMessage.Heartbeat hb : p.heartbeats) {
            if (!addressedHere(hb.destPrefix)) continue;
            int wk = hb.writerId[3] & 0xFF;
            if (hb.livelinessFlag) {
                noteWriterAlive(hex(new Guid(p.header.guidPrefix, hb.writerId).bytes()));
            }
            if (hb.groupInfo) {
                String wkey = hex(new Guid(p.header.guidPrefix, hb.writerId).bytes());
                if (hb.writerSet != null) writerGroups.put(wkey, hex(hb.writerSet));
                String digest = writerGroups.get(wkey);
                if (digest != null && readerMatchesWriter(wkey))
                    groupOrder.heartbeat(hex(p.header.guidPrefix) + ":" + digest, wkey,
                            hb.currentGsn, hb.firstGsn, hb.lastGsn);
            }
            boolean builtin = wk == Guid.KIND_BUILTIN_WRITER_WITH_KEY || wk == 0xC3;
            if (builtin || readerMatchesWriter(hex(new Guid(p.header.guidPrefix, hb.writerId).bytes())))
                onWriterHeartbeat(p.header.guidPrefix, hb, src);
        }
    }

    private boolean addressedHere(byte[] destination) {
        return destination == null || equalsBytes(destination, new byte[12]) || equalsBytes(destination, guidPrefix);
    }

    private ReliableReceiveState receiveState(byte[] sender, byte[] writer, byte[] reader) {
        return reliableReaders.computeIfAbsent(hex(new Guid(sender, writer).bytes()) + ":" + hex(reader), key -> new ReliableReceiveState());
    }

    /**
     * The local readers a DATA or GAP of {@code writer} goes into the reliable receive state of. A
     * reader takes data only from a writer it has matched (DDSI-RTPS 2.5 8.4.12: the WriterProxy
     * of the writer); a user DATA addressed to a reader that has not matched the writer yet is
     * dropped without a trace in that state, so the reader asks for it once the writer's HEARTBEAT
     * reaches it after the match. Recorded first, the sample was dropped by {@code onUserData} and
     * acknowledged by the next ACKNACK: a request pushed to a service still fetching the client's
     * types was never answered. Built-in readers take every built-in writer.
     */
    private List<byte[]> receivingReaders(byte[] sender, byte[] writer, byte[] reader) {
        List<byte[]> addressed = addressedReaders(sender, writer, reader);
        if ((writer[3] & 0xc0) == 0xc0) return addressed;
        String key = hex(new Guid(sender, writer).bytes());
        List<byte[]> result = new ArrayList<>();
        for (byte[] candidate : addressed) {
            for (WireReader local : readers) {
                if (equalsBytes(local.entityId, candidate) && local.matchedWriters.containsKey(key)) {
                    result.add(candidate);
                    break;
                }
            }
        }
        return result;
    }

    private List<byte[]> addressedReaders(byte[] sender, byte[] writer, byte[] reader) {
        if (reader != null && !equalsBytes(reader, Guid.ENTITYID_UNKNOWN)) return java.util.Collections.singletonList(reader);
        if ((writer[3] & 0xc0) == 0xc0) return java.util.Collections.singletonList(deriveBuiltinReader(writer));
        List<byte[]> result = new ArrayList<>();
        String key = hex(new Guid(sender, writer).bytes());
        for (WireReader local : readers) if (local.matchedWriters.containsKey(key)) result.add(local.entityId);
        return result;
    }

    /** DATA that is being handed to the readers; the receive state counts a sample before its reader has it. */
    private final java.util.concurrent.atomic.AtomicInteger deliveriesInFlight =
            new java.util.concurrent.atomic.AtomicInteger();

    private void receiveData(byte[] sender, RtpsMessage.Data data, java.net.SocketAddress source) {
        if (!addressedHere(data.destPrefix)) return;
        int kind = data.writerId[3] & 255;
        DiscoveryData.Endpoint endpoint = remoteWriters.get(hex(new Guid(sender, data.writerId).bytes()));
        boolean reliable = !equalsBytes(data.writerId, Guid.SPDP_WRITER)
                && (kind == Guid.KIND_BUILTIN_WRITER_WITH_KEY || kind == 0xC3
                    || endpoint != null && endpoint.reliabilityKind == Rtps.RELIABILITY_RELIABLE);
        if (reliable) {
            for (byte[] reader : receivingReaders(sender, data.writerId, data.readerId)) {
                RtpsMessage.Data directed = new RtpsMessage.Data();
                directed.destPrefix = data.destPrefix; directed.readerId = reader;
                directed.writerId = data.writerId; directed.writerSn = data.writerSn;
                directed.serializedPayload = data.serializedPayload; directed.inlineQos = data.inlineQos; directed.keyFlag = data.keyFlag;
                directed.sourceTimestampMillis = data.sourceTimestampMillis;
                for (RtpsMessage.Data ready : receiveState(sender, data.writerId, reader).offer(directed)) dispatchReceivedData(sender, ready, source);
            }
        } else dispatchReceivedData(sender, data, source);
    }

    private void dispatchReceivedData(byte[] sender, RtpsMessage.Data d, java.net.SocketAddress src) {
            int writerKind = d.writerId[3] & 0xFF;
            if (equalsBytes(d.writerId, Guid.SPDP_WRITER)) {
                onSpdp(d);
            } else if (equalsBytes(d.writerId, Guid.SEDP_PUB_WRITER)) {
                onSedpEndpoint(true, d);
            } else if (equalsBytes(d.writerId, Guid.SEDP_SUB_WRITER)) {
                onSedpEndpoint(false, d);
            } else if (equalsBytes(d.writerId, Guid.TL_REQ_WRITER) || equalsBytes(d.writerId, Guid.TL_REPLY_WRITER)) {
                onTypeLookup(sender, d, src);
            } else if (writerKind == Guid.KIND_USER_WRITER_NO_KEY
                    || writerKind == Guid.KIND_USER_WRITER_WITH_KEY) {
                onUserData(sender, d);
            }
    }

    private boolean readerMatchesWriter(String writerGuidHex) {
        for (WireReader r : readers) {
            if (r.matchedWriters.containsKey(writerGuidHex)) {
                return true;
            }
        }
        return false;
    }

    private void onWriterHeartbeat(byte[] senderPrefix, RtpsMessage.Heartbeat hb,
                                    java.net.SocketAddress src) {
        for (byte[] reader : addressedReaders(senderPrefix, hb.writerId, hb.readerId)) onWriterHeartbeat(senderPrefix, hb, src, reader);
    }

    private void onWriterHeartbeat(byte[] senderPrefix, RtpsMessage.Heartbeat hb,
                                    java.net.SocketAddress src, byte[] readerId) {
        String wkey = hex(new Guid(senderPrefix, hb.writerId).bytes()) + ":" + hex(readerId);
        ReliableReceiveState state = receiveState(senderPrefix, hb.writerId, readerId);
        for (RtpsMessage.Data ready : state.heartbeat(hb.firstSn)) dispatchReceivedData(senderPrefix, ready, src);
        announcedLast.merge(wkey, hb.lastSn, Math::max);
        if (pendingAcks.add(wkey)) {
            // R::heartbeatResponseDelay (DDSI-RTPS 2.5 8.4.12.1): one ACKNACK for the HEARTBEATs of the delay
            after(TIMING.heartbeatResponseDelayMillis, () -> {
                pendingAcks.remove(wkey);
                sendAckNack(senderPrefix, hb, src, readerId, wkey, state);
            });
        }
    }

    private void sendAckNack(byte[] senderPrefix, RtpsMessage.Heartbeat hb, java.net.SocketAddress src,
                             byte[] readerId, String wkey, ReliableReceiveState state) {
        long base = state.base();
        int numBits = 0;
        long last = Math.max(hb.lastSn, announcedLast.getOrDefault(wkey, 0L));
        if (last >= base) {
            numBits = (int) Math.min(256, last - base + 1);
        }
        int count = ackCounters.merge(wkey, 1, Integer::sum);
        byte[] dg = new RtpsMessage.Builder(guidPrefix)
                .infoDestination(senderPrefix)
                .ackNack(readerId, hb.writerId, base, numBits, count, false)
                .toBytes();
        DiscoveryData.Participant peer = peers.get(hex(senderPrefix));
        String tgt;
        if (peer != null && peer.metatrafficUnicast != null
                && peer.metatrafficUnicast.isUsableUdpV4()) {
            sendTo(dg, peer.metatrafficUnicast);
            tgt = peer.metatrafficUnicast.toString();
        } else if (src instanceof InetSocketAddress) {
            sendToAddr(dg, (InetSocketAddress) src);
            tgt = src.toString();
        } else {
            tgt = "(none)";
        }
        if (DEBUG) {
            log("ACKNACK w=" + hex(hb.writerId) + " r=" + hex(readerId) + " base=" + base
                    + " numBits=" + numBits + " (hb first=" + hb.firstSn + " last=" + hb.lastSn
                    + ") -> " + tgt);
        }
    }

    private static byte[] deriveBuiltinReader(byte[] writerId) {
        byte[] r = writerId.clone();
        int k = r[3] & 0xFF;
        r[3] = (byte) (k == Guid.KIND_BUILTIN_WRITER_WITH_KEY ? Guid.KIND_BUILTIN_READER_WITH_KEY
                : (k == 0xC3 ? 0xC4 : k));
        return r;
    }

    private void onSpdp(RtpsMessage.Data d) {
        DiscoveryData.Participant peer = DiscoveryData.parseParticipant(d.serializedPayload);
        if (peer == null || peer.guidPrefix == null) {
            return;
        }
        if (equalsBytes(peer.guidPrefix, guidPrefix)) {
            return;
        }
        peer.lastSeenMillis = System.currentTimeMillis();
        String key = hex(peer.guidPrefix);
        boolean isNew = !peers.containsKey(key);
        peers.put(key, peer);
        if (isNew) {
            fire(listener -> listener.participantDiscovered(peer));
            log("SPDP discovered participant " + key
                    + (peer.metatrafficUnicast != null ? " meta=" + peer.metatrafficUnicast : ""));
            answerSpdp(peer);
            announceAllTo(peer);
        }
    }

    /**
     * A new participant is told of this one at once, by unicast (the SPDP period is 30 s, 9.6.2.4, and
     * the new participant only knows us from our next announcement otherwise).
     */
    private void answerSpdp(DiscoveryData.Participant peer) {
        try {
            byte[] payload = DiscoveryData.buildParticipant(guidPrefix, domainId,
                    announceLocator, announceLocator, Rtps.EP_ALL_BASIC | Rtps.EP_TYPELOOKUP_ALL, 100,
                    userData);
            byte[] dg = new RtpsMessage.Builder(guidPrefix)
                    .infoTimestamp((int) (System.currentTimeMillis() / 1000L), 0)
                    .data(Guid.ENTITYID_UNKNOWN, Guid.SPDP_WRITER, nextSpdpSn(), null, payload, false)
                    .toBytes();
            if (peer.metatrafficUnicast != null && peer.metatrafficUnicast.isUsableUdpV4()) {
                sendTo(dg, peer.metatrafficUnicast);
            }
        } catch (Exception e) {
            log("SPDP answer failed: " + e);
        }
    }

    private void onSedpEndpoint(boolean writer, RtpsMessage.Data d) {
        DiscoveryData.Endpoint e = DiscoveryData.parseEndpoint(writer, d.serializedPayload);
        if (e == null) {
            if (DEBUG) {
                log("SEDP parse FAILED (" + (writer ? "pub" : "sub") + ") plen="
                        + d.serializedPayload.length + " encap="
                        + (d.serializedPayload.length >= 2 ? String.format("%02x%02x",
                        d.serializedPayload[0], d.serializedPayload[1]) : "?"));
            }
            return;
        }
        String key = hex(e.endpointGuid);
        if (writer) {
            if (remoteWriters.put(key, e) == null) {
                fire(listener -> listener.endpointDiscovered(e));
                log("SEDP discovered remote WRITER topic='" + e.topicName + "' type='"
                        + e.typeName + "' guid=" + key
                        + (e.typeId != null && e.typeId.length > 0 ? " typeId=" + hex(e.typeId) : ""));
            }
            synchronized (this) {
                for (WireReader r : readers) {
                    tryMatchReader(r, e);
                }
            }
        } else {
            if (remoteReaders.put(key, e) == null) {
                fire(listener -> listener.endpointDiscovered(e));
                log("SEDP discovered remote READER topic='" + e.topicName + "' type='"
                        + e.typeName + "' guid=" + key
                        + (e.typeId != null && e.typeId.length > 0 ? " typeId=" + hex(e.typeId) : ""));
            }
            synchronized (this) {
                for (WireWriter w : writers) {
                    tryMatchWriter(w, e);
                }
            }
        }
    }

    // ---- what the application ignores (DDS 1.4 2.2.2.2.1.14 to 2.2.2.2.1.17) ----
    //
    // All participants of this process and domain share this RTPS participant, so an ignore
    // of a remote participant, publication, subscription or topic holds for all of them.

    private final java.util.Set<String> ignoredPrefixes = ConcurrentHashMap.newKeySet();
    private final java.util.Set<String> ignoredEndpoints = ConcurrentHashMap.newKeySet();
    private final java.util.Set<String> ignoredTopics = ConcurrentHashMap.newKeySet();

    /** The remote participant with this GUID prefix is ignored: nothing it announces is matched. */
    public void ignoreParticipant(byte[] guidPrefix) {
        ignoredPrefixes.add(hex(guidPrefix));
        rematchIgnored();
    }

    /** The remote publication or subscription with this endpoint GUID is ignored. */
    public void ignoreEndpoint(byte[] endpointGuid) {
        ignoredEndpoints.add(hex(endpointGuid));
        rematchIgnored();
    }

    /** Every remote publication and subscription of the topic is ignored. */
    public void ignoreTopic(String topicName) {
        ignoredTopics.add(topicName);
        rematchIgnored();
    }

    private boolean ignores(DiscoveryData.Endpoint endpoint) {
        return ignoredEndpoints.contains(hex(endpoint.endpointGuid))
                || ignoredPrefixes.contains(hex(Wire.slice(endpoint.endpointGuid, 0, 12)))
                || (endpoint.topicName != null && ignoredTopics.contains(endpoint.topicName));
    }

    /** Drops the matches an ignore now forbids. */
    private synchronized void rematchIgnored() {
        for (WireWriter w : writers) {
            w.matched.keySet().removeIf(key -> {
                DiscoveryData.Endpoint e = remoteReaders.get(key);
                return e != null && ignores(e);
            });
        }
        for (WireReader r : readers) {
            r.matchedWriters.keySet().removeIf(key -> {
                DiscoveryData.Endpoint e = remoteWriters.get(key);
                return e != null && ignores(e);
            });
        }
    }

    private void tryMatchWriter(WireWriter w, DiscoveryData.Endpoint remoteReader) {
        if (ignores(remoteReader)) {
            w.matched.remove(hex(remoteReader.endpointGuid));
            return;
        }
        if (!representationsOverlap(new int[] {encapDataRep(w.encap)}, remoteReader.dataRepresentations)) {
            w.matched.remove(hex(remoteReader.endpointGuid));
            return;
        }
        if (!RpcDiscovery.topicsCorrespond(w.topic, w.rpc, remoteReader.topicName, remoteReader.rpc)) {
            return;
        }
        if (!typesMatch(w.byTypeInformation, w.typeName, w.typeInformation, w.typeMatcher,
                remoteReader, true)) {
            return;
        }
        // DDS 1.4 §2.2.3 request-offered: the writer offers, the reader requests.
        String refused = QosParameters.incompatibility(w.qos, remoteReader.qos);
        if (refused != null) {
            log("SEDP no match: local WRITER topic='" + w.topic + "' offers incompatible "
                    + refused + " to remote reader " + hex(remoteReader.endpointGuid));
            return;
        }
        if (remoteReader.unicastLocator == null || !remoteReader.unicastLocator.isUsableUdpV4()) {
            return;
        }
        String key = hex(remoteReader.endpointGuid);
        if (w.matched.containsKey(key)) {
            return;
        }
        byte[] readerEntity = Wire.slice(remoteReader.endpointGuid, 12, 4);
        w.matched.put(key, new MatchedReader(
                Wire.slice(remoteReader.endpointGuid, 0, 12), readerEntity,
                remoteReader.unicastLocator));
        log("SEDP endpoint-match: local WRITER topic='" + w.topic + "' type='" + w.typeName
                + "' -> remote reader " + key + " @ " + remoteReader.unicastLocator
                + typeIdNote(w.typeId, remoteReader.typeId));
    }

    private void tryMatchReader(WireReader r, DiscoveryData.Endpoint remoteWriter) {
        if (ignores(remoteWriter)) {
            r.matchedWriters.remove(hex(remoteWriter.endpointGuid));
            return;
        }
        if (!representationsOverlap(r.dataRepresentations, remoteWriter.dataRepresentations)) {
            r.matchedWriters.remove(hex(remoteWriter.endpointGuid));
            return;
        }
        if (!RpcDiscovery.topicsCorrespond(r.topic, r.rpc, remoteWriter.topicName, remoteWriter.rpc)) {
            return;
        }
        if (!typesMatch(r.byTypeInformation, r.typeName, r.typeInformation, r.typeMatcher,
                remoteWriter, false)) {
            return;
        }
        String refused = QosParameters.incompatibility(remoteWriter.qos, r.qos);
        if (refused != null) {
            log("SEDP no match: local READER topic='" + r.topic + "' requests incompatible "
                    + refused + " of remote writer " + hex(remoteWriter.endpointGuid));
            return;
        }
        String key = hex(remoteWriter.endpointGuid);
        if (r.matchedWriters.putIfAbsent(key, Boolean.TRUE) == null) {
            r.matchedAt.put(key, System.currentTimeMillis());
            log("SEDP endpoint-match: local READER topic='" + r.topic + "' type='" + r.typeName
                    + "' <- remote writer " + key
                    + typeIdNote(new byte[0], remoteWriter.typeId));
        }
    }

    /**
     * The type part of a match. An endpoint that pairs by TypeInformation is judged by
     * {@link TypeConsistency}; a verdict that lacks an object of the peer asks the peer's
     * TypeLookup service for it and matches again when the reply came. Any other endpoint
     * matches the same type name, or what its {@code typeMatcher} accepts.
     *
     * @param localWrites whether the local endpoint is the writer of the pair
     */
    private boolean typesMatch(boolean byTypeInformation, String localTypeName, byte[] localInformation,
            java.util.function.Predicate<DiscoveryData.Endpoint> typeMatcher, DiscoveryData.Endpoint remote,
            boolean localWrites) {
        if (!byTypeInformation) {
            return localTypeName.equals(remote.typeName) || (typeMatcher != null && typeMatcher.test(remote));
        }
        byte[] remoteInformation = remote.typeInformation == null ? new byte[0] : remote.typeInformation;
        TypeConsistency.Verdict verdict = localWrites
                ? TypeConsistency.decide(localInformation, localTypeName, remoteInformation, remote.typeName,
                        this::typeObjectBytes)
                : TypeConsistency.decide(remoteInformation, remote.typeName, localInformation, localTypeName,
                        this::typeObjectBytes);
        switch (verdict.kind) {
            case CONSISTENT:
                return true;
            case UNRESOLVED:
                askTypeObject(Wire.slice(remote.endpointGuid, 0, 12), verdict.missing);
                return false;
            default:
                log("SEDP no match: local " + (localWrites ? "WRITER" : "READER") + " type='" + localTypeName
                        + "' and remote " + hex(remote.endpointGuid) + " type='" + remote.typeName + "': "
                        + verdict.reason);
                return false;
        }
    }

    /** TypeObjects the peers served (XTypes 1.3 7.6.3.3.4), by encoded TypeIdentifier, hash checked. */
    private final Map<String, byte[]> servedTypeObjects = new ConcurrentHashMap<>();
    /** The bound of {@link #servedTypeObjects}: objects past it are not kept. */
    private static final int SERVED_TYPE_OBJECTS_BOUND = 4096;
    /** The identifiers asked of a peer and not answered yet, as {@code peer:identifier}. */
    private final Set<String> typeObjectsAsked = ConcurrentHashMap.newKeySet();
    /** How often an identifier was asked of a peer, as {@code peer:identifier}. */
    private final Map<String, Integer> typeObjectAttempts = new ConcurrentHashMap<>();
    /** Asks of one identifier of one peer at most this often. */
    private static final int TYPE_OBJECT_ATTEMPTS = 3;

    /** The serialized TypeObject of an encoded identifier: this participant's own, or one a peer served. */
    private byte[] typeObjectBytes(byte[] identifier) {
        org.zerodds.cdr.TypeObjectRegistry.Entry own = typeLookup.find(identifier);
        return own != null ? own.object() : servedTypeObjects.get(hex(identifier));
    }

    /**
     * Asks the TypeLookup service of {@code peer} for the object of {@code identifier}
     * ({@code getTypes}, XTypes 1.3 7.6.3.3.4.2) and matches the endpoints of that peer again when
     * the reply came, or the query ended without one.
     */
    private void askTypeObject(byte[] peer, byte[] identifier) {
        String key = hex(peer) + ":" + hex(identifier);
        if (servedTypeObjects.containsKey(hex(identifier))
                || typeObjectAttempts.getOrDefault(key, 0) >= TYPE_OBJECT_ATTEMPTS
                || !typeObjectsAsked.add(key)) {
            return;
        }
        typeObjectAttempts.merge(key, 1, Integer::sum);
        DDS.Builtin.TypeLookup_Call call = new DDS.Builtin.TypeLookup_Call();
        call._d = (int) DDS.Builtin.TypeLookup_getTypes_HashId;
        call.getTypes = new DDS.Builtin.TypeLookup_getTypes_In();
        call.getTypes.type_ids = new DDS.XTypes.TypeIdentifier[] {
            org.zerodds.typelookup.CompleteTypes.decodeIdentifier(identifier)
        };
        java.util.concurrent.CompletableFuture<DDS.Builtin.TypeLookup_Reply> reply;
        try {
            reply = queryTypeLookup(peer, call);
        } catch (RuntimeException refused) {
            typeObjectsAsked.remove(key);
            log("TypeLookup getTypes " + hex(identifier) + " not asked of " + hex(peer) + ": " + refused.getMessage());
            return;
        }
        reply.whenComplete((answer, error) -> {
            if (answer != null) {
                keepServed(answer);
            }
            typeObjectsAsked.remove(key);
            if (!servedTypeObjects.containsKey(hex(identifier))) {
                log("TypeLookup getTypes " + hex(identifier) + " of " + hex(peer) + ": "
                        + (error != null ? error.toString() : "the reply holds no such object"));
            }
            matchAgain(peer);
        });
    }

    /** Keeps the objects of a {@code getTypes} reply whose serialization hashes to their identifier. */
    private void keepServed(DDS.Builtin.TypeLookup_Reply reply) {
        if (reply.return_data == null || reply.return_data._d != (int) DDS.Builtin.TypeLookup_getTypes_HashId
                || reply.return_data.getType == null || reply.return_data.getType.result == null
                || reply.return_data.getType.result.types == null) {
            return;
        }
        for (DDS.XTypes.TypeIdentifierTypeObjectPair pair : reply.return_data.getType.result.types) {
            if (servedTypeObjects.size() >= SERVED_TYPE_OBJECTS_BOUND) {
                log("TypeLookup: " + SERVED_TYPE_OBJECTS_BOUND + " served objects kept, the rest of a reply is not");
                return;
            }
            try {
                byte[] identifier = org.zerodds.typelookup.CompleteTypes.encodeIdentifier(pair.type_identifier);
                byte[] object = org.zerodds.typelookup.CompleteTypes.encodeObject(pair.type_object);
                if (hashesTo(pair.type_identifier, pair.type_object, object)) {
                    servedTypeObjects.putIfAbsent(hex(identifier), object);
                } else {
                    log("TypeLookup: the object served for " + hex(identifier) + " does not hash to it");
                }
            } catch (RuntimeException unreadable) {
                log("TypeLookup: a served object is not kept: " + unreadable);
            }
        }
    }

    /** XTypes 1.3 7.3.4.8: the hash of an object is the first 14 octets of the MD5 of its serialization. */
    private static boolean hashesTo(DDS.XTypes.TypeIdentifier id, DDS.XTypes.TypeObject object, byte[] serialized) {
        if ((id._d != DDS.XTypes.EK_COMPLETE && id._d != DDS.XTypes.EK_MINIMAL) || object._d != id._d
                || id.equivalence_hash == null || id.equivalence_hash.length != 14) {
            return false;
        }
        byte[] md5 = org.zerodds.cdr.Md5.hash(serialized);
        for (int i = 0; i < 14; i++) {
            if ((md5[i] & 0xff) != (id.equivalence_hash[i] & 0xff)) {
                return false;
            }
        }
        return true;
    }

    /** Matches the local endpoints that pair by TypeInformation with the endpoints of {@code peer} again. */
    private void matchAgain(byte[] peer) {
        String prefix = hex(peer);
        synchronized (this) {
            for (WireWriter w : writers) {
                if (!w.byTypeInformation) {
                    continue;
                }
                for (DiscoveryData.Endpoint r : new ArrayList<>(remoteReaders.values())) {
                    if (hex(Wire.slice(r.endpointGuid, 0, 12)).equals(prefix)) {
                        tryMatchWriter(w, r);
                    }
                }
            }
            for (WireReader r : readers) {
                if (!r.byTypeInformation) {
                    continue;
                }
                for (DiscoveryData.Endpoint w : new ArrayList<>(remoteWriters.values())) {
                    if (hex(Wire.slice(w.endpointGuid, 0, 12)).equals(prefix)) {
                        tryMatchReader(r, w);
                    }
                }
            }
        }
    }

    private static String typeIdNote(byte[] localId, byte[] remoteId) {
        if (remoteId == null || remoteId.length == 0) {
            return "";
        }
        return " [TypeIdentifier " + hex(remoteId) + "]";
    }

    private void onUserData(byte[] senderPrefix, RtpsMessage.Data d) {
        byte[] writerGuid = new byte[16];
        System.arraycopy(senderPrefix, 0, writerGuid, 0, 12);
        System.arraycopy(d.writerId, 0, writerGuid, 12, 4);
        String wkey = hex(writerGuid);
        noteWriterAlive(wkey);
        // Track highest received SN so our ACKNACK to the reliable writer
        // advances (positive-acks delivered samples, stops retransmits).
        long coherentSet = d.inlineQos == null ? CoherentHold.NO_SET
                : CoherentSetParameter.decode(d.inlineQos.find(Rtps.PID_COHERENT_SET));
        byte[] groupSeqBytes = d.inlineQos == null ? null : d.inlineQos.find(Rtps.PID_GROUP_SEQ_NUM);
        long gsn = CoherentSetParameter.decode(groupSeqBytes);
        long groupSet = d.inlineQos == null ? CoherentHold.NO_SET
                : CoherentSetParameter.decode(d.inlineQos.find(Rtps.PID_GROUP_COHERENT_SET));
        byte[] digest = d.inlineQos == null ? null : d.inlineQos.find(Rtps.PID_WRITER_GROUP_INFO);
        String groupKey = hex(senderPrefix) + ":" + (digest == null ? "" : hex(digest));
        if (digest != null) {
            writerGroups.put(wkey, hex(digest));
        }
        if (d.serializedPayload == null || d.serializedPayload.length == 0) {
            if (gsn != CoherentHold.NO_SET) {
                // the End Coherent Set DATA of a group coherent set
                boolean grouped = false;
                for (WireReader r : readers) {
                    grouped |= r.matchedWriters.containsKey(wkey) && r.groupScope();
                }
                if (grouped) {
                    groupOrder.ecs(groupKey, wkey, gsn, groupSet);
                }
            }
            // a DATA without serialized data ends the writer's coherent set (DDSI-RTPS 2.5 8.7.6)
            for (WireReader r : readers) {
                if (r.matchedWriters.containsKey(wkey) && r.coherent()) {
                    r.release(r.hold.end(wkey));
                }
            }
            return;
        }
        SerializedSample sample;
        try {
            sample = SerializedSample.parse(d.serializedPayload);
        } catch (org.zerodds.cdr.XcdrException ex) {
            log("refused serialized payload: " + ex.getMessage());
            return;
        }
        // The identity of the sample is the writer's GUID and sequence number; the
        // identity it relates to, if any, is in the inline QoS (DDS-RPC 1.0 §7.8.2).
        org.zerodds.internal.SourceIdentity related = null;
        if (d.inlineQos != null) {
            related = org.zerodds.internal.SourceIdentity.fromBytesLe(
                    d.inlineQos.find(Rtps.PID_RELATED_SAMPLE_IDENTITY));
        }
        sample = sample.withSource(new org.zerodds.internal.SourceIdentity(writerGuid, d.writerSn),
                related).withSourceTimestamp(d.sourceTimestampMillis);
        final List<WireReader> grouped = new ArrayList<>();
        final SerializedSample received = sample;
        for (WireReader r : readers) {
            boolean addressed = equalsBytes(d.readerId, r.entityId)
                    || equalsBytes(d.readerId, Guid.ENTITYID_UNKNOWN);
            if (addressed && r.matchedWriters.containsKey(wkey)
                    && representationsOverlap(r.dataRepresentations, new int[] {sample.representation})) {
                if (gsn != CoherentHold.NO_SET && r.groupScope()) {
                    grouped.add(r); // ordered across the writers of the Publisher (8.7.5)
                    continue;
                }
                if (r.coherent()) {
                    r.release(r.hold.accept(wkey, coherentSet, d.writerSn, sample));
                    continue;
                }
                try {
                    r.onSample.accept(sample);
                } catch (Exception ex) {
                    log("reader callback threw: " + ex);
                }
            }
        }
        if (!grouped.isEmpty()) {
            groupOrder.accept(groupKey, wkey, gsn, groupSet, new Runnable() {
                @Override
                public void run() {
                    for (WireReader r : grouped) {
                        try {
                            r.onSample.accept(received);
                        } catch (Exception ex) {
                            log("reader callback threw: " + ex);
                        }
                    }
                }
            });
        }
    }

    private static final RtpsTimings TIMING = RtpsTimings.CURRENT;

    /** Runs the delayed ACKNACKs and repairs (heartbeatResponseDelay, nackResponseDelay). */
    private final java.util.concurrent.ScheduledExecutorService timers =
            java.util.concurrent.Executors.newSingleThreadScheduledExecutor(new java.util.concurrent.ThreadFactory() {
                @Override
                public Thread newThread(Runnable r) {
                    Thread t = new Thread(r, "rtps-timers");
                    t.setDaemon(true);
                    return t;
                }
            });

    /** The ACKNACKs waiting for their heartbeatResponseDelay, by writer and reader. */
    private final java.util.Set<String> pendingAcks = ConcurrentHashMap.newKeySet();

    private void after(long delayMillis, Runnable action) {
        Runnable guarded = () -> {
            try {
                action.run();
            } catch (Exception e) {
                log("delayed response failed: " + e);
            }
        };
        if (delayMillis <= 0) {
            guarded.run();
        } else if (running) {
            try {
                timers.schedule(guarded, delayMillis, java.util.concurrent.TimeUnit.MILLISECONDS);
            } catch (java.util.concurrent.RejectedExecutionException closed) {
                // the participant is closed
            }
        }
    }

    /** The group (digest in hex) that a remote writer announced in its DATA and HEARTBEATs. */
    private final Map<String, String> writerGroups = new ConcurrentHashMap<>();

    /**
     * The writers that count for the group {@code key} (prefix:digest): discovered as reliable,
     * matched to a reader with access scope GROUP, of this group or not yet heard from (8.7.5).
     */
    private Set<String> countedWriters(String key) {
        String prefix = key.substring(0, key.indexOf(':'));
        String digest = key.substring(key.indexOf(':') + 1);
        Set<String> result = new java.util.HashSet<>();
        for (WireReader r : readers) {
            if (!r.groupScope()) continue;
            for (String wkey : r.matchedWriters.keySet()) {
                if (!wkey.startsWith(prefix)) continue;
                DiscoveryData.Endpoint e = remoteWriters.get(wkey);
                if (e == null || e.reliabilityKind != Rtps.RELIABILITY_RELIABLE) continue;
                String known = writerGroups.get(wkey);
                if (known == null || known.equals(digest)) result.add(wkey);
            }
        }
        return result;
    }

    /**
     * The ordering across the writers of remote Publishers with access scope GROUP. The timeout
     * rule is an option: {@code zerodds.rtps.group-timeout-millis}, off by default.
     */
    private final GroupOrder groupOrder = new GroupOrder(RtpsParticipant::log,
            delivery -> org.zerodds.internal.InProcessBus.instance().atomically(delivery),
            this::countedWriters, Long.getLong("zerodds.rtps.group-timeout-millis", 0L));

    /** Strip the 4-byte encapsulation header (RTPS 2.5 §10.5). */
    static byte[] stripEncap(byte[] payload) {
        return SerializedSample.parse(payload).body();
    }

    // ---- SEDP announce ----

    private void announceAllToPeers() {
        for (DiscoveryData.Participant peer : peers.values()) {
            announceAllTo(peer);
        }
    }

    private synchronized void announceAllTo(DiscoveryData.Participant peer) {
        Locator target = peer.metatrafficUnicast;
        if (target == null || !target.isUsableUdpV4()) {
            return;
        }
        if (!writers.isEmpty() || !readers.isEmpty()) {
            log("SEDP announce " + writers.size() + " writer(s) + " + readers.size()
                    + " reader(s) to peer " + hex(peer.guidPrefix) + " @ " + target);
        }
        for (WireWriter w : writers) {
            byte[] payload = DiscoveryData.buildEndpoint(true, guidPrefix, w.guid, w.topic,
                    w.typeName, w.reliable ? Rtps.RELIABILITY_RELIABLE : Rtps.RELIABILITY_BEST_EFFORT,
                    Rtps.DURABILITY_VOLATILE, new int[] {encapDataRep(w.encap)}, announceLocator,
                    w.typeId, w.typeInformation, w.qos, w.rpc);
            sedpPubSn++;
            publishSedp(Guid.SEDP_PUB_WRITER, Guid.SEDP_PUB_READER, peer.guidPrefix, w.guid,
                    address(target), sedpPubSn, payload);
        }
        for (WireReader r : readers) {
            // Typed readers advertise only their descriptor's implemented representations.
            byte[] payload = DiscoveryData.buildEndpoint(false, guidPrefix, r.guid, r.topic,
                    r.typeName, Rtps.RELIABILITY_BEST_EFFORT, Rtps.DURABILITY_VOLATILE,
                    r.dataRepresentations, announceLocator, new byte[0], r.typeInformation, r.qos,
                    r.rpc);
            sedpSubSn++;
            publishSedp(Guid.SEDP_SUB_WRITER, Guid.SEDP_SUB_READER, peer.guidPrefix, r.guid,
                    address(target), sedpSubSn, payload);
        }
    }

    private static int encapDataRep(byte[] encap) {
        return SerializedSample.parse(encap).representation;
    }

    static boolean representationsOverlap(int[] left, int[] right) {
        for (int a : left) for (int b : right) if (a == b) return true;
        return false;
    }

    // ---- send ----

    private static InetSocketAddress address(Locator locator) {
        try { return new InetSocketAddress(InetAddress.getByAddress(locator.ipv4()), (int) locator.port); }
        catch (java.net.UnknownHostException error) { throw new IllegalArgumentException("Invalid locator", error); }
    }

    /** The sequence number of the last announcement of each endpoint to each peer. */
    private final Map<String, Long> sedpLast = new ConcurrentHashMap<>();

    /**
     * An announcement of an endpoint to a peer. The built-in endpoints keep the last sample of
     * each instance (HISTORY KEEP_LAST 1, DDSI-RTPS 2.5 8.4.15), so the announcement before it
     * leaves the history of the SEDP writer and a peer that never acknowledges does not fill it.
     */
    private void publishSedp(byte[] writerId, byte[] readerId, byte[] peer, byte[] endpointGuid,
            InetSocketAddress address, long sequence, byte[] payload) {
        String key = hex(writerId) + ":" + hex(peer) + ":" + hex(endpointGuid);
        Long previous = sedpLast.get(key);
        ReliableWriterState writer = reliableWriters.get(hex(writerId));
        if (previous != null && writer != null) {
            writer.supersede(previous);
        }
        if (writer != null) {
            long dropped = writer.makeRoom();
            if (dropped != 0) {
                log("SEDP history full: the announcement " + dropped + " left unacknowledged");
            }
        }
        publishReliable(writerId, readerId, peer, address, sequence, payload);
        sedpLast.put(key, sequence);
    }

    private void publishReliable(byte[] writerId, byte[] readerId, byte[] peer,
            InetSocketAddress address, long sequence, byte[] payload) {
        ReliableWriterState writer = reliableWriters.computeIfAbsent(hex(writerId), key -> new ReliableWriterState(writerId));
        ReliableWriterState.Target target = new ReliableWriterState.Target(peer, readerId, address);
        ReliableWriterState.Sample sample = writer.add(sequence, payload, target);
        sendHistory(writer, sample, target);
    }

    private void sendHistory(ReliableWriterState writer, ReliableWriterState.Sample sample,
            ReliableWriterState.Target target) {
        final int fragmentSize = 1200;
        int seconds = (int) (sample.timestampMillis / 1000);
        long fraction = ((sample.timestampMillis % 1000) << 32) / 1000;
        if (sample.payload.length <= fragmentSize) {
            sendToAddr(new RtpsMessage.Builder(guidPrefix).infoDestination(target.prefix)
                    .infoTimestamp(seconds, fraction)
                    .data(target.reader, writer.writer, sample.sequence, sample.inlineQos, sample.payload, false).toBytes(), target.address);
        } else {
            int number = 1;
            for (int offset = 0; offset < sample.payload.length; offset += fragmentSize, number++) {
                byte[] part = java.util.Arrays.copyOfRange(sample.payload, offset, Math.min(offset + fragmentSize, sample.payload.length));
                sendToAddr(new RtpsMessage.Builder(guidPrefix).infoDestination(target.prefix)
                        .infoTimestamp(seconds, fraction)
                        .dataFrag(target.reader, writer.writer, sample.sequence, number, 1,
                                fragmentSize, sample.payload.length,
                                number == 1 ? sample.inlineQos : null, part, false).toBytes(), target.address);
            }
            sendToAddr(new RtpsMessage.Builder(guidPrefix).infoDestination(target.prefix)
                    .heartbeatFrag(target.reader, writer.writer, sample.sequence,
                        (sample.payload.length + fragmentSize - 1) / fragmentSize, writer.nextHeartbeatCount())
                    .toBytes(), target.address);
        }
        sendHeartbeat(writer, target);
    }

    private void sendHeartbeat(ReliableWriterState writer, ReliableWriterState.Target target) {
        long[] range = writer.range(target);
        GroupStamp group = groupInfoOf(writer);
        if (group != null) {
            // HEARTBEAT with the GroupInfoFlag: currentGSN, firstGSN, lastGSN of the writer, and the
            // writerSet of the Publisher unless the option omits it (DDSI-RTPS 2.5 8.3.7.5, 9.4.5.7)
            long[] groups = writer.groupRange();
            long first = range[0] == 0 ? writer.lastSequence() + 1 : range[0];
            long last = range[0] == 0 ? writer.lastSequence() : range[1];
            long current = Math.max(group.groupSeq, groups[1]);
            sendToAddr(new RtpsMessage.Builder(guidPrefix).infoDestination(target.prefix)
                    .heartbeatGroup(target.reader, writer.writer, first, last, writer.nextHeartbeatCount(),
                            false, current, groups[0], groups[1],
                            OMIT_HEARTBEAT_GROUP_DIGESTS ? null : group.digest)
                    .toBytes(), target.address);
            return;
        }
        if (range[0] == 0) return;
        sendToAddr(new RtpsMessage.Builder(guidPrefix).infoDestination(target.prefix)
                .heartbeat(target.reader, writer.writer, range[0], range[1], writer.nextHeartbeatCount(), false)
                .toBytes(), target.address);
    }

    /**
     * Option (the Rust side: {@code wire.omit_heartbeat_group_digests}, default off): the HEARTBEATs
     * with the GroupInfoFlag carry no writerSet and secureWriterSet; the reader then takes the group
     * from the digest of the DATA.
     */
    private static final boolean OMIT_HEARTBEAT_GROUP_DIGESTS =
            Boolean.getBoolean("zerodds.rtps.omit-heartbeat-group-digests");

    /** The group info of the writer's Publisher, or {@code null} if it has no GROUP access or no group sample yet. */
    private GroupStamp groupInfoOf(ReliableWriterState writer) {
        for (WireWriter w : writers) {
            if (equalsBytes(w.entityId, writer.writer)) {
                java.util.function.Supplier<GroupStamp> info = w.groupInfo;
                return info == null ? null : info.get();
            }
        }
        return null;
    }

    /** A writer of a GROUP Publisher with an empty history still tells the readers the group's state. */
    private void sendIdleGroupHeartbeats() {
        for (WireWriter w : writers) {
            if (!w.reliable || w.groupInfo == null || w.groupInfo.get() == null) continue;
            ReliableWriterState state = reliableWriters.computeIfAbsent(hex(w.entityId),
                    key -> new ReliableWriterState(w.entityId));
            for (MatchedReader reader : w.matched.values()) {
                ReliableWriterState.Target target = new ReliableWriterState.Target(
                        reader.participantPrefix, reader.readerEntityId, address(reader.locator));
                if (state.range(target)[0] == 0) sendHeartbeat(state, target);
            }
        }
    }

    private void onAckNack(byte[] sender, RtpsMessage.AckNack ack, java.net.SocketAddress source) {
        if (ack.destPrefix != null && !equalsBytes(ack.destPrefix, guidPrefix)) return;
        ReliableWriterState writer = reliableWriters.get(hex(ack.writerId));
        if (writer == null) return;
        // The repair goes where the samples went, to the locator of the reader proxy (DDSI-RTPS 2.5
        // 8.4.9.2.8), not to the source of the ACKNACK: a reader may send from another socket than
        // the one it announced, and the reply locator of a received message has no port (8.3.4.1).
        // Taken before the ACKNACK takes the acknowledged samples out of the history.
        ReliableWriterState.Target target = writer.target(sender, ack.readerId);
        if (target == null || !writer.acknowledge(sender, ack)) return;
        // W::nackResponseDelay (DDSI-RTPS 2.5 8.4.7.1): the repair goes out after the delay
        after(TIMING.nackResponseDelayMillis, () -> {
            for (int bit = 0; bit < ack.numBits; bit++) {
                if ((ack.bitmap[bit / 32] & (1L << (31 - bit % 32))) == 0) continue;
                long sequence = ack.base + bit;
                if (sequence < ack.base) break;
                ReliableWriterState.Sample sample = writer.requested(sequence, target);
                if (sample != null) sendHistory(writer, sample, target);
                else sendToAddr(new RtpsMessage.Builder(guidPrefix).infoDestination(sender)
                        .gap(ack.readerId, ack.writerId, sequence, sequence + 1).toBytes(), target.address);
            }
        });
    }

    private void onNackFragment(byte[] sender, RtpsMessage.NackFragment nack, java.net.SocketAddress source) {
        if (!addressedHere(nack.destPrefix)) return;
        ReliableWriterState writer = reliableWriters.get(hex(nack.writerId));
        if (writer == null) return;
        // As for an ACKNACK: the fragments go to the locator of the reader proxy.
        ReliableWriterState.Target target = writer.target(sender, nack.readerId);
        if (target == null) return;
        ReliableWriterState.Sample sample = writer.requested(nack.writerSn, target);
        if (sample == null || !writer.acceptFragmentCount(target, nack.count)) return;
        final int fragmentSize = 1200;
        for (int bit = 0; bit < nack.numBits; bit++) {
            if ((nack.bitmap[bit / 32] & (1L << (31 - bit % 32))) == 0) continue;
            long number = nack.bitmapBase + bit;
            long offset = (number - 1) * fragmentSize;
            if (number < 1 || offset >= sample.payload.length) continue;
            byte[] part = java.util.Arrays.copyOfRange(sample.payload, (int) offset, Math.min((int) offset + fragmentSize, sample.payload.length));
            sendToAddr(new RtpsMessage.Builder(guidPrefix).infoDestination(sender)
                .dataFrag(target.reader, writer.writer, sample.sequence, number, 1, fragmentSize,
                    sample.payload.length, null, part, false).toBytes(), target.address);
        }
    }

    private void sendTo(byte[] datagram, Locator loc) {
        try {
            InetSocketAddress addr = new InetSocketAddress(
                    InetAddress.getByAddress(loc.ipv4()), (int) loc.port);
            unicastChannel.send(ByteBuffer.wrap(datagram), addr);
        } catch (Exception e) {
            log("send failed to " + loc + ": " + e);
        }
    }

    private void sendToAddr(byte[] datagram, InetSocketAddress addr) {
        try {
            unicastChannel.send(ByteBuffer.wrap(datagram), addr);
        } catch (Exception e) {
            log("send failed to " + addr + ": " + e);
        }
    }

    public void close() {
        running = false;
        timers.shutdownNow();
        for (TypeQuery query : typeQueries.values()) query.future.completeExceptionally(new IllegalStateException("Participant closed"));
        try {
            if (spdpChannel != null) {
                spdpChannel.close();
            }
            if (unicastChannel != null) {
                unicastChannel.close();
            }
        } catch (IOException ignore) {
            // best effort
        }
        BY_DOMAIN.remove(domainId, this);
    }

    // ---- utils ----

    private static boolean equalsBytes(byte[] a, byte[] b) {
        if (a == null || b == null || a.length != b.length) {
            return false;
        }
        for (int i = 0; i < a.length; i++) {
            if (a[i] != b[i]) {
                return false;
            }
        }
        return true;
    }

    static String hex(byte[] b) {
        StringBuilder sb = new StringBuilder(b.length * 2);
        for (byte x : b) {
            sb.append(String.format("%02x", x));
        }
        return sb.toString();
    }
}
