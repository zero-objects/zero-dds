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
 * <p>Cross-process delivery requires the pure-Java RTPS-stack which is a
 * separate sprint (see audit {@code dds-java-psm-1.0.md} Open-Items).
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

        public Message(byte[] payload, byte[] keyHash, ChangeKind kind, long writerId,
                       int ownershipStrength, boolean exclusive, List<String> partitions,
                       long sourceTimestampMillis) {
            this.payload = payload;
            this.keyHash = keyHash == null ? new byte[0] : keyHash;
            this.kind = kind;
            this.writerId = writerId;
            this.ownershipStrength = ownershipStrength;
            this.exclusive = exclusive;
            this.partitions = partitions;
            this.sourceTimestampMillis = sourceTimestampMillis;
        }
    }

    /** A subscribing reader. */
    public interface Endpoint {
        void onMessage(Message m);
    }

    /** A registered writer — used for late-join replay + liveliness tracking. */
    public interface WriterPresence {
        long writerId();

        /** Effective partition names of this writer's Publisher. */
        List<String> partitions();

        boolean exclusive();

        int ownershipStrength();

        /** Replay retained samples (TRANSIENT_LOCAL) to a freshly-joined reader. */
        void replayTo(Endpoint reader);

        /** Whether this writer is currently asserting liveliness. */
        boolean isAlive();
    }

    private static final class Channel {
        final CopyOnWriteArrayList<Endpoint> readers = new CopyOnWriteArrayList<>();
        final CopyOnWriteArrayList<WriterPresence> writers = new CopyOnWriteArrayList<>();
        // Legacy raw byte[] listeners (back-compat path).
        final CopyOnWriteArrayList<Consumer<byte[]>> rawListeners = new CopyOnWriteArrayList<>();
    }

    private final Map<String, Channel> channels = new ConcurrentHashMap<>();

    private InProcessBus() {}

    private static String key(int domainId, String topicName) {
        return domainId + ":" + topicName;
    }

    private Channel channel(int domainId, String topicName) {
        return channels.computeIfAbsent(key(domainId, topicName), k -> new Channel());
    }

    // ---- structured reader/writer registration --------------------------

    /** Register a reader. Returns the live writers already present (for late-join). */
    public List<WriterPresence> registerReader(int domainId, String topicName, Endpoint reader) {
        Channel ch = channel(domainId, topicName);
        ch.readers.add(reader);
        return new ArrayList<>(ch.writers);
    }

    public void unregisterReader(int domainId, String topicName, Endpoint reader) {
        Channel ch = channels.get(key(domainId, topicName));
        if (ch != null) {
            ch.readers.remove(reader);
        }
    }

    public void registerWriter(int domainId, String topicName, WriterPresence writer) {
        channel(domainId, topicName).writers.add(writer);
    }

    public void unregisterWriter(int domainId, String topicName, WriterPresence writer) {
        Channel ch = channels.get(key(domainId, topicName));
        if (ch != null) {
            ch.writers.remove(writer);
        }
    }

    /** Live writers currently registered on the channel (snapshot). */
    public List<WriterPresence> writers(int domainId, String topicName) {
        Channel ch = channels.get(key(domainId, topicName));
        return ch == null ? List.of() : new ArrayList<>(ch.writers);
    }

    public List<Endpoint> readers(int domainId, String topicName) {
        Channel ch = channels.get(key(domainId, topicName));
        return ch == null ? List.of() : new ArrayList<>(ch.readers);
    }

    /** Deliver a structured message to all registered readers. */
    public void publish(int domainId, String topicName, Message m) {
        Channel ch = channels.get(key(domainId, topicName));
        if (ch == null) return;
        for (Endpoint r : ch.readers) {
            r.onMessage(m);
        }
        if (m.payload != null) {
            for (Consumer<byte[]> l : ch.rawListeners) {
                l.accept(m.payload);
            }
        }
    }

    // ---- legacy raw byte[] path (kept for back-compat) ------------------

    public void subscribe(int domainId, String topicName, Consumer<byte[]> listener) {
        channel(domainId, topicName).rawListeners.add(listener);
    }

    public void unsubscribe(int domainId, String topicName, Consumer<byte[]> listener) {
        Channel ch = channels.get(key(domainId, topicName));
        if (ch != null) {
            ch.rawListeners.remove(listener);
        }
    }

    public void publish(int domainId, String topicName, byte[] sample) {
        Channel ch = channels.get(key(domainId, topicName));
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
