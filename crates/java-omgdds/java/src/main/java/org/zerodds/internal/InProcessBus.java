// SPDX-License-Identifier: Apache-2.0
package org.zerodds.internal;

import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.CopyOnWriteArrayList;
import java.util.function.Consumer;

/**
 * In-process Loopback-Bus for the pure-Java PSM.
 *
 * <p>Publishers post to a shared {@link ConcurrentHashMap} keyed by
 * {@code (domainId, topicName)}; subscribers register {@link Endpoint}s.
 * There is no RTPS wire — the intended use is in-process unit-testing +
 * pure-Java applications that don't need cross-process delivery.
 *
 * <p>Beyond raw byte fan-out the bus now carries enough metadata to let the
 * binding enforce the per-instance QoS policies from DDS-DCPS 1.4:
 * a {@link Message} carries the instance key bytes, the change kind
 * (write/dispose/unregister), the originating writer id + ownership
 * strength, and the writer's effective partition set. The bus also keeps a
 * registry of live writers so a late-joining reader can be told about
 * already-present writers (TRANSIENT_LOCAL replay, LIVELINESS) — DDS-DCPS
 * 1.4 §2.2.3.4 / §2.2.3.11.
 *
 * <p>Cross-process delivery is the optional pure-Java RTPS stack of
 * {@code org.zerodds.rtps}, which the writers and readers open alongside this
 * bus when {@code -Dzerodds.rtps.enable=true} is set.
 */
public final class InProcessBus {
    private static final InProcessBus INSTANCE = new InProcessBus();

    public static InProcessBus instance() {
        return INSTANCE;
    }

    /** Change kind carried on the wire — DDS-DCPS 1.4 §2.2.3.4 / §2.2.4.2. */
    public enum ChangeKind {
        WRITE,
        DISPOSE,
        UNREGISTER,
    }

    /** A delivered sample plus its DDS instance/source metadata. */
    public static final class Message {
        public final byte[] payload;       // XCDR2 bytes (null for pure dispose/unregister)
        public final byte[] keyHash;       // instance key bytes ([] => single instance)
        public final ChangeKind kind;
        public final long writerId;        // originating writer identity
        public final int ownershipStrength;
        public final boolean exclusive;    // writer OWNERSHIP == EXCLUSIVE
        public final List<String> partitions; // writer's effective partition names
        public final long sourceTimestampMillis;
        /** The writing entity's handle — the sample's publication_handle. */
        public final org.omg.dds.core.InstanceHandle writerHandle;
        /** The writer GUID and sequence number of the sample, or {@code null} (a replay, a dispose). */
        public final SourceIdentity identity;
        /** The identity of the sample this one relates to (DDS-RPC 1.0 §7.8.2), or {@code null}. */
        public final SourceIdentity related;
        /** The LIFESPAN of the writer in milliseconds, or {@link #NO_LIFESPAN} for an infinite one. */
        public final long lifespanMillis;
        /** The {@link #lifespanMillis} of a sample that does not expire (DDS 1.4 §2.2.3.16). */
        public static final long NO_LIFESPAN = -1L;

        public Message(byte[] payload, byte[] keyHash, ChangeKind kind, long writerId,
                       int ownershipStrength, boolean exclusive, List<String> partitions,
                       long sourceTimestampMillis) {
            this(payload, keyHash, kind, writerId, ownershipStrength, exclusive, partitions,
                    sourceTimestampMillis, null);
        }

        public Message(byte[] payload, byte[] keyHash, ChangeKind kind, long writerId,
                       int ownershipStrength, boolean exclusive, List<String> partitions,
                       long sourceTimestampMillis,
                       org.omg.dds.core.InstanceHandle writerHandle) {
            this(payload, keyHash, kind, writerId, ownershipStrength, exclusive, partitions,
                    sourceTimestampMillis, writerHandle, null, null);
        }

        public Message(byte[] payload, byte[] keyHash, ChangeKind kind, long writerId,
                       int ownershipStrength, boolean exclusive, List<String> partitions,
                       long sourceTimestampMillis,
                       org.omg.dds.core.InstanceHandle writerHandle,
                       SourceIdentity identity, SourceIdentity related) {
            this(payload, keyHash, kind, writerId, ownershipStrength, exclusive, partitions,
                    sourceTimestampMillis, writerHandle, identity, related, NO_LIFESPAN);
        }

        public Message(byte[] payload, byte[] keyHash, ChangeKind kind, long writerId,
                       int ownershipStrength, boolean exclusive, List<String> partitions,
                       long sourceTimestampMillis,
                       org.omg.dds.core.InstanceHandle writerHandle,
                       SourceIdentity identity, SourceIdentity related, long lifespanMillis) {
            this.lifespanMillis = lifespanMillis;
            this.identity = identity;
            this.related = related;
            this.payload = payload;
            this.keyHash = keyHash == null ? new byte[0] : keyHash;
            this.kind = kind;
            this.writerId = writerId;
            this.ownershipStrength = ownershipStrength;
            this.exclusive = exclusive;
            this.partitions = partitions;
            this.sourceTimestampMillis = sourceTimestampMillis;
            this.writerHandle = writerHandle;
        }

        /** This message with the identity of the sample and of the one it relates to. */
        public Message withIdentity(SourceIdentity identity, SourceIdentity related) {
            return new Message(payload, keyHash, kind, writerId, ownershipStrength, exclusive,
                    partitions, sourceTimestampMillis, writerHandle, identity, related,
                    lifespanMillis);
        }

        /** This message with the LIFESPAN of its writer. */
        public Message withLifespan(long lifespanMillis) {
            return new Message(payload, keyHash, kind, writerId, ownershipStrength, exclusive,
                    partitions, sourceTimestampMillis, writerHandle, identity, related,
                    lifespanMillis);
        }

        /**
         * The time from which the sample no longer exists for a reader, or
         * {@link Long#MAX_VALUE} when it never expires: source timestamp plus the writer's
         * LIFESPAN (DDS 1.4 §2.2.3.16).
         */
        public long expiresAtMillis() {
            return lifespanMillis < 0 ? Long.MAX_VALUE : sourceTimestampMillis + lifespanMillis;
        }
    }

    /** A subscribing reader. */
    public interface Endpoint {
        void onMessage(Message m);

        /**
         * Effective partition names of this reader's Subscriber. The bus does
         * not match on them itself — it reports every writer on the topic and
         * lets each side apply §2.2.3.13 — but a writer needs the reader's set
         * to decide whether a reader that appeared is one of its matches.
         */
        default List<String> partitions() {
            return java.util.Collections.emptyList();
        }

        /** The reading entity's handle — a reader's own instance handle. */
        default org.omg.dds.core.InstanceHandle instanceHandle() {
            return null;
        }

        /** The GUID of this reader (DDS-RPC 1.0 §7.6.2.1: the target of a related-entity key). */
        default byte[] endpointGuid() {
            return null;
        }

        /** The DDS-RPC 1.0 §7.6.2.1 discovery extensions of this reader, if it has any. */
        default org.zerodds.rtps.RpcDiscovery rpcDiscovery() {
            return org.zerodds.rtps.RpcDiscovery.NONE;
        }

        /**
         * The requested QoS of this reader (DDS 1.4 §2.2.3 RxO). Used by a
         * writer to refuse an incompatible match and record
         * OFFERED_INCOMPATIBLE_QOS. May be {@code null} for endpoints that do
         * not participate in RxO matching.
         */
        default java.util.Map<Class<? extends org.omg.dds.core.policy.QosPolicy>,
                ? extends org.omg.dds.core.policy.QosPolicy> requestedQos() {
            return null;
        }

        /**
         * A writer on this reader's topic was registered ({@code present}) or
         * unregistered. Raised so a DataReaderListener can report
         * SUBSCRIPTION_MATCHED without a poll; the reader applies the
         * partition rule itself.
         */
        default void onWriterPresenceChanged(WriterPresence writer, boolean present) {}

        /**
         * Whether this reader also accepts writers of another type name (the
         * TypeConsistencyEnforcement of the reader is DECLARED or ASSIGNABLE).
         */
        default boolean coercesTypes() {
            return false;
        }

        /** Whether the reader accepts samples of the writer type {@code writerType} (see {@link #coercesTypes}). */
        default boolean acceptsType(String writerTypeName, org.omg.dds.type.dynamic.DynamicType writerType) {
            return false;
        }
    }

    /** A registered writer — used for late-join replay + liveliness tracking. */
    public interface WriterPresence {
        long writerId();

        /** Effective partition names of this writer's Publisher. */
        List<String> partitions();

        /**
         * The writing entity's handle. A reader copies it into the sample's
         * {@code publication_handle} (DDS-DCPS 1.4 §2.2.2.5.5, SampleInfo).
         */
        default org.omg.dds.core.InstanceHandle writerHandle() {
            return null;
        }

        boolean exclusive();

        int ownershipStrength();

        /** The offered QoS of this writer (DDS 1.4 §2.2.3 RxO). */
        default java.util.Map<Class<? extends org.omg.dds.core.policy.QosPolicy>,
                ? extends org.omg.dds.core.policy.QosPolicy> offeredQos() {
            return null;
        }

        /** Replay retained samples (TRANSIENT_LOCAL) to a freshly-joined reader. */
        void replayTo(Endpoint reader);

        /** Whether this writer is currently asserting liveliness. */
        boolean isAlive();

        /**
         * A reader on this writer's topic was registered ({@code present}) or
         * unregistered — the mirror of
         * {@link Endpoint#onWriterPresenceChanged}, for PUBLICATION_MATCHED.
         */
        default void onReaderPresenceChanged(Endpoint reader, boolean present) {}

        /** The type of the samples this writer sends, or {@code null} if it has none to offer for coercion. */
        default org.omg.dds.type.dynamic.DynamicType dynamicType() {
            return null;
        }
    }

    /** A reader of one type name that accepts a writer of another (type coercion). */
    private static final class Link {
        final WriterPresence writer;
        final Endpoint reader;

        Link(WriterPresence writer, Endpoint reader) {
            this.writer = writer;
            this.reader = reader;
        }
    }

    private static final class Channel {
        final int domainId;
        final String topicName;
        final String typeName;
        final CopyOnWriteArrayList<Endpoint> readers = new CopyOnWriteArrayList<>();
        final CopyOnWriteArrayList<WriterPresence> writers = new CopyOnWriteArrayList<>();
        /** Readers of other type names that receive what the writers of this channel send. */
        final CopyOnWriteArrayList<Link> links = new CopyOnWriteArrayList<>();

        Channel(int domainId, String topicName, String typeName) {
            this.domainId = domainId;
            this.topicName = topicName;
            this.typeName = typeName;
        }
        // Legacy raw byte[] listeners (back-compat path).
        final CopyOnWriteArrayList<Consumer<byte[]>> rawListeners = new CopyOnWriteArrayList<>();
    }

    private final Map<String, Channel> channels = new ConcurrentHashMap<>();

    private InProcessBus() {}

    /**
     * The gate of coherent sets (DDS 1.4 2.2.2.4.1.10): the delivery of a set holds the write side,
     * every read or take of a reader holds the read side, so a reader on another thread sees
     * the set as a whole or not at all.
     */
    private final java.util.concurrent.locks.ReentrantReadWriteLock coherentGate =
            new java.util.concurrent.locks.ReentrantReadWriteLock();

    /** Runs {@code delivery}, which hands the samples of one coherent set to the readers, as one step. */
    public void atomically(Runnable delivery) {
        java.util.concurrent.locks.Lock write = coherentGate.writeLock();
        write.lock();
        try {
            delivery.run();
        } finally {
            write.unlock();
        }
    }

    /** The side a reader holds while it reads or takes; see {@link #atomically}. */
    public java.util.concurrent.locks.Lock readerGate() {
        return coherentGate.readLock();
    }

    private static String key(int domainId, String topicName, String typeName) {
        // DDS 1.2 §2.1.2.5 / §7.1.3: a writer and a reader communicate only on
        // a matching topic name *and* a matching type name. Keying the channel
        // by both prevents a reader from decoding foreign bytes with its own
        // TypeSupport when two different types share a topic name.
        return domainId + ":" + topicName + ":" + typeName;
    }

    private Channel channel(int domainId, String topicName, String typeName) {
        return channels.computeIfAbsent(key(domainId, topicName, typeName),
                k -> new Channel(domainId, topicName, typeName));
    }

    // ---- structured reader/writer registration --------------------------

    /** Register a reader. Returns the live writers already present (for late-join). */
    public List<WriterPresence> registerReader(int domainId, String topicName, String typeName, Endpoint reader) {
        Channel ch = channel(domainId, topicName, typeName);
        ch.readers.add(reader);
        List<WriterPresence> present = new ArrayList<>();
        for (WriterPresence w : ch.writers) {
            if (!ignored(reader, w)) {
                present.add(w);
            }
        }
        // Both sides learn about the pairing, in both directions: a reader
        // joining a topic that already has writers is as much a match as a
        // writer joining a topic that already has readers. A pair one side's
        // participant ignores (DDS 1.4 2.2.2.2.1.14 to 2.2.2.2.1.17) is no match.
        for (WriterPresence w : present) {
            reader.onWriterPresenceChanged(w, true);
            w.onReaderPresenceChanged(reader, true);
        }
        if (reader.coercesTypes()) {
            for (Channel other : channels.values()) {
                if (other == ch || other.domainId != domainId || !other.topicName.equals(topicName)) {
                    continue;
                }
                for (WriterPresence w : other.writers) {
                    if (link(other, w, reader)) {
                        present.add(w);
                    }
                }
            }
        }
        return present;
    }

    /**
     * Type coercion (TypeConsistencyEnforcement DECLARED or ASSIGNABLE): {@code reader} of one
     * type name takes the samples of {@code writer}, registered under another type name of the
     * same topic, if the reader accepts the writer's type. Both sides learn of the match.
     */
    private boolean link(Channel writerChannel, WriterPresence writer, Endpoint reader) {
        if (ignored(reader, writer) || !reader.acceptsType(writerChannel.typeName, writer.dynamicType())) {
            return false;
        }
        for (Link l : writerChannel.links) {
            if (l.writer == writer && l.reader == reader) {
                return false;
            }
        }
        writerChannel.links.add(new Link(writer, reader));
        reader.onWriterPresenceChanged(writer, true);
        writer.onReaderPresenceChanged(reader, true);
        return true;
    }

    private void unlinkReader(Endpoint reader) {
        for (Channel ch : channels.values()) {
            for (Link l : ch.links) {
                if (l.reader == reader && ch.links.remove(l)) {
                    l.writer.onReaderPresenceChanged(reader, false);
                }
            }
        }
    }

    private void unlinkWriter(WriterPresence writer) {
        for (Channel ch : channels.values()) {
            for (Link l : ch.links) {
                if (l.writer == writer && ch.links.remove(l)) {
                    l.reader.onWriterPresenceChanged(writer, false);
                }
            }
        }
    }

    public void unregisterReader(int domainId, String topicName, String typeName, Endpoint reader) {
        Channel ch = channels.get(key(domainId, topicName, typeName));
        if (ch != null && ch.readers.remove(reader)) {
            for (WriterPresence w : ch.writers) {
                w.onReaderPresenceChanged(reader, false);
            }
        }
        unlinkReader(reader);
    }

    public void registerWriter(int domainId, String topicName, String typeName, WriterPresence writer) {
        Channel ch = channel(domainId, topicName, typeName);
        ch.writers.add(writer);
        for (Endpoint r : ch.readers) {
            if (ignored(r, writer)) {
                continue;
            }
            r.onWriterPresenceChanged(writer, true);
            writer.onReaderPresenceChanged(r, true);
        }
        for (Channel other : channels.values()) {
            if (other == ch || other.domainId != domainId || !other.topicName.equals(topicName)) {
                continue;
            }
            for (Endpoint r : other.readers) {
                if (r.coercesTypes()) {
                    link(ch, writer, r);
                }
            }
        }
    }

    /** Whether the participant of the reader or of the writer ignores the other (see {@link Ignores}). */
    private static boolean ignored(Endpoint reader, WriterPresence writer) {
        return Ignores.pairIgnored(reader.instanceHandle(), writer.writerHandle());
    }

    /**
     * A participant has ignored something (DDS 1.4 2.2.2.2.1.14 to 2.2.2.2.1.17): the pairs of
     * readers and writers that are matched and that an ignore now separates are told that the
     * other side is gone, so their matched counts fall and no sample passes between them.
     */
    public void rematchIgnored() {
        for (Channel ch : channels.values()) {
            for (Endpoint r : ch.readers) {
                for (WriterPresence w : ch.writers) {
                    if (ignored(r, w)) {
                        r.onWriterPresenceChanged(w, false);
                        w.onReaderPresenceChanged(r, false);
                    }
                }
            }
        }
    }

    public void unregisterWriter(int domainId, String topicName, String typeName, WriterPresence writer) {
        Channel ch = channels.get(key(domainId, topicName, typeName));
        if (ch != null && ch.writers.remove(writer)) {
            for (Endpoint r : ch.readers) {
                r.onWriterPresenceChanged(writer, false);
            }
        }
        unlinkWriter(writer);
    }

    /**
     * The partitions of a registered writer changed from {@code before} to what
     * {@link WriterPresence#partitions()} answers now (DDS 1.4 §2.2.3.13: PARTITION can change):
     * each reader whose overlap with the writer changed is told, as of a match that appeared or
     * ended. Pairs whose overlap stays are not touched.
     */
    public void writerPartitionsChanged(int domainId, String topicName, String typeName,
                                        WriterPresence writer, List<String> before) {
        Channel ch = channels.get(key(domainId, topicName, typeName));
        if (ch == null || !ch.writers.contains(writer)) {
            return;
        }
        for (Endpoint r : ch.readers) {
            boolean was = org.zerodds.rtps.QosParameters.partitionsOverlap(before, r.partitions());
            boolean now = org.zerodds.rtps.QosParameters.partitionsOverlap(writer.partitions(), r.partitions());
            if (was != now) {
                r.onWriterPresenceChanged(writer, now);
                writer.onReaderPresenceChanged(r, now);
            }
        }
    }

    /** The mirror of {@link #writerPartitionsChanged} for a registered reader. */
    public void readerPartitionsChanged(int domainId, String topicName, String typeName,
                                        Endpoint reader, List<String> before) {
        Channel ch = channels.get(key(domainId, topicName, typeName));
        if (ch == null || !ch.readers.contains(reader)) {
            return;
        }
        for (WriterPresence w : ch.writers) {
            boolean was = org.zerodds.rtps.QosParameters.partitionsOverlap(w.partitions(), before);
            boolean now = org.zerodds.rtps.QosParameters.partitionsOverlap(w.partitions(), reader.partitions());
            if (was != now) {
                reader.onWriterPresenceChanged(w, now);
                w.onReaderPresenceChanged(reader, now);
            }
        }
    }

    /** Live writers currently registered on the channel (snapshot). */
    public List<WriterPresence> writers(int domainId, String topicName, String typeName) {
        Channel ch = channels.get(key(domainId, topicName, typeName));
        return ch == null ? java.util.Collections.emptyList() : new ArrayList<>(ch.writers);
    }

    public List<Endpoint> readers(int domainId, String topicName, String typeName) {
        Channel ch = channels.get(key(domainId, topicName, typeName));
        return ch == null ? java.util.Collections.emptyList() : new ArrayList<>(ch.readers);
    }

    /** Deliver a structured message to all registered readers. */
    public void publish(int domainId, String topicName, String typeName, Message m) {
        publish(domainId, topicName, typeName, m, null);
    }

    /**
     * As {@link #publish(int, String, String, Message)}, to the readers that announce the
     * service instance {@code instance} only (DDS-RPC 1.0 §7.11.1.4.3); {@code null} is every
     * reader.
     */
    public void publish(int domainId, String topicName, String typeName, Message m,
            String instance) {
        Channel ch = channels.get(key(domainId, topicName, typeName));
        if (ch == null) return;
        if (System.currentTimeMillis() >= m.expiresAtMillis()) {
            // DDS 1.4 §2.2.3.16 LIFESPAN: a sample already past its expiration time when it is
            // delivered is not delivered at all.
            return;
        }
        for (Endpoint r : ch.readers) {
            if (m.writerHandle != null && Ignores.pairIgnored(r.instanceHandle(), m.writerHandle)) {
                continue;
            }
            if (instance != null && !r.rpcDiscovery().serviceInstanceName().equals(instance)) {
                continue;
            }
            r.onMessage(m);
        }
        for (Link l : ch.links) {
            if (l.writer.writerId() == m.writerId
                    && !(m.writerHandle != null
                            && Ignores.pairIgnored(l.reader.instanceHandle(), m.writerHandle))
                    && !(instance != null
                            && !l.reader.rpcDiscovery().serviceInstanceName().equals(instance))) {
                l.reader.onMessage(m);
            }
        }
        if (m.payload != null && instance == null) {
            for (Consumer<byte[]> l : ch.rawListeners) {
                l.accept(m.payload);
            }
        }
    }

    // ---- legacy raw byte[] path (kept for back-compat) ------------------

    public void subscribe(int domainId, String topicName, Consumer<byte[]> listener) {
        channel(domainId, topicName, "").rawListeners.add(listener);
    }

    public void unsubscribe(int domainId, String topicName, Consumer<byte[]> listener) {
        Channel ch = channels.get(key(domainId, topicName, ""));
        if (ch != null) {
            ch.rawListeners.remove(listener);
        }
    }

    public void publish(int domainId, String topicName, byte[] sample) {
        Channel ch = channels.get(key(domainId, topicName, ""));
        if (ch == null) return;
        for (Consumer<byte[]> l : ch.rawListeners) {
            l.accept(sample);
        }
    }

    /** Spec-only test-helper — clears all subscriptions. */
    public void reset() {
        channels.clear();
    }
}
