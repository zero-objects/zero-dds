// SPDX-License-Identifier: Apache-2.0
package org.zerodds.internal;

import org.omg.dds.core.policy.Durability;
import org.omg.dds.core.policy.History;
import org.omg.dds.core.policy.ResourceLimits;
import org.zerodds.core.OutOfResourcesException;

import java.io.IOException;
import java.util.ArrayDeque;
import java.util.ArrayList;
import java.util.Deque;
import java.util.HashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.TimeUnit;
import java.util.function.LongSupplier;

/**
 * In-memory durability service for the pure-Java PSM.
 *
 * <p>DDS 1.4 §2.2.3.4: a DataWriter with DURABILITY kind TRANSIENT or
 * PERSISTENT writes "as if" there were a built-in durability DataWriter that
 * keeps samples available to late-joining DataReaders beyond the writer's own
 * lifetime. TRANSIENT_LOCAL samples stay in the DataWriter and die with it;
 * TRANSIENT and PERSISTENT samples live here, keyed by (domain, topic, type)
 * and instance. TRANSIENT entries are dropped when the last participant of a
 * domain is closed; PERSISTENT entries outlive the participant for the process
 * (there is no permanent storage in the pure-Java stack).
 *
 * <p>The DURABILITY_SERVICE policy of the writer configures this store
 * (DDS 1.4 §2.2.3.5): {@code history_kind} and {@code history_depth} give the
 * samples kept per instance, {@code max_samples}, {@code max_instances} and
 * {@code max_samples_per_instance} bound the store. A sample that would exceed a
 * limit is refused with an {@link OutOfResourcesException}; nothing is dropped
 * without the writer being told.
 *
 * <p>{@code service_cleanup_delay} (DDS 1.4 §2.2.3.4) decides when the store
 * forgets an instance: after the instance has been disposed, no writer writes
 * it any more (each unregistered it) and the delay has elapsed since both were
 * true. Until then a late joiner is told the instance is disposed.
 *
 * <p>PERSISTENT entries are also written to a journal in the directory named by the system property
 * {@code zerodds.durability.dir}, and replayed from it when the topic is first used, so that they
 * outlive the process (DDS 1.4 §2.2.3.4). Without the property they stay in memory like TRANSIENT
 * ones. A journal that cannot be written or read is an {@link OutOfResourcesException}; nothing is
 * dropped silently.
 *
 * <p>Each retained entry records the offered DURABILITY kind that produced it,
 * so a reader pulls only entries its requested kind can accept
 * ({@code requested <= offered}, §2.2.3.4 RxO ordering
 * VOLATILE &lt; TRANSIENT_LOCAL &lt; TRANSIENT &lt; PERSISTENT).
 */
public final class DurabilityService {
    /** Synthetic writer identity for samples replayed from durable storage. */
    public static final long DURABLE_WRITER_ID = -7_000_000_002L;

    private static final DurabilityService INSTANCE = new DurabilityService();

    public static DurabilityService instance() {
        return INSTANCE;
    }

    /** One retained sample with the time it was written and the LIFESPAN it was written under. */
    private static final class Sample {
        final byte[] payload;
        final long timestampMillis;
        final long lifespanMillis;

        Sample(byte[] payload, long timestampMillis, long lifespanMillis) {
            this.payload = payload;
            this.timestampMillis = timestampMillis;
            this.lifespanMillis = lifespanMillis;
        }
    }

    /** One retained instance of a durable topic. */
    private static final class Entry {
        final Durability.Kind kind;
        final Deque<Sample> payloads = new ArrayDeque<>();
        /** The writers that wrote the instance and have not unregistered it. */
        final Set<Long> writers = new HashSet<>();
        boolean disposed;
        /** The time from which the store forgets the instance, or -1 while it keeps it. */
        long purgeAtMillis = -1;
        long cleanupDelayMillis;

        Entry(Durability.Kind kind) {
            this.kind = kind;
        }
    }

    /** A retained sample with the source timestamp and the expiration time it was written with. */
    public static final class Kept {
        public final byte[] payload;
        public final long timestampMillis;
        /** Milliseconds since the epoch, {@code 0} for a sample that does not expire. */
        public final long expirationMillis;

        public Kept(byte[] payload, long timestampMillis, long expirationMillis) {
            this.payload = payload;
            this.timestampMillis = timestampMillis;
            this.expirationMillis = expirationMillis;
        }

        public boolean expiredAt(long nowMillis) {
            return expirationMillis != 0L && nowMillis >= expirationMillis;
        }
    }

    // (domainId:topic:type) -> (instance key -> retained entry)
    private final Map<String, Map<InstanceKey, Entry>> stores = new ConcurrentHashMap<>();

    private volatile LongSupplier clock = System::currentTimeMillis;

    private DurabilityService() {}

    private static String channelKey(int domainId, String topic, String type) {
        return domainId + ":" + topic + ":" + type;
    }

    /**
     * Replaces the clock that decides when {@code service_cleanup_delay} has elapsed
     * (milliseconds since the epoch). For tests; {@link #reset()} restores the system clock.
     */
    public void useClock(LongSupplier millis) {
        this.clock = millis;
    }

    /**
     * Retains a sample for a durable writer under the DURABILITY_SERVICE policy
     * {@code service} of that writer.
     *
     * @param timestampMillis  the source timestamp of the sample.
     * @param lifespanMillis   the LIFESPAN of the writer, or {@link InProcessBus.Message#NO_LIFESPAN}.
     * @throws OutOfResourcesException  if the sample would exceed a limit of the policy.
     */
    public void retain(int domainId, String topic, String type, InstanceKey key,
                       byte[] payload, Durability.Kind kind, long writerId,
                       org.omg.dds.core.policy.DurabilityService service,
                       long timestampMillis, long lifespanMillis) {
        retain(domainId, topic, type, key, payload, kind, writerId, service.getHistoryKind(),
                service.getHistoryDepth(), service.getMaxSamples(), service.getMaxInstances(),
                service.getMaxSamplesPerInstance(), cleanupMillis(service), timestampMillis,
                lifespanMillis);
    }

    private void retain(int domainId, String topic, String type, InstanceKey key,
                        byte[] payload, Durability.Kind kind, long writerId,
                        History.Kind historyKind, int historyDepth, int maxSamples,
                        int maxInstances, int maxSamplesPerInstance, long cleanupDelayMillis,
                        long timestampMillis, long lifespanMillis) {
        String channel = channelKey(domainId, topic, type);
        Map<InstanceKey, Entry> perInstance = channel(channel, true);
        synchronized (perInstance) {
            sweep(channel, perInstance);
            Entry entry = perInstance.get(key);
            boolean created = false;
            if (entry == null) {
                if (maxInstances != ResourceLimits.LENGTH_UNLIMITED
                        && perInstance.size() >= maxInstances) {
                    throw new OutOfResourcesException("DURABILITY_SERVICE max_instances "
                            + maxInstances + " reached");
                }
                entry = new Entry(kind);
                created = true;
            }
            int cap = Integer.MAX_VALUE;
            if (historyKind == History.Kind.KEEP_LAST) {
                cap = Math.max(1, historyDepth);
            }
            if (maxSamplesPerInstance != ResourceLimits.LENGTH_UNLIMITED) {
                if (historyKind == History.Kind.KEEP_ALL
                        && entry.payloads.size() >= maxSamplesPerInstance) {
                    throw new OutOfResourcesException("DURABILITY_SERVICE max_samples_per_instance "
                            + maxSamplesPerInstance + " reached");
                }
                cap = Math.min(cap, maxSamplesPerInstance);
            }
            int dropped = Math.max(0, entry.payloads.size() + 1 - cap);
            if (maxSamples != ResourceLimits.LENGTH_UNLIMITED) {
                int total = 0;
                for (Entry other : perInstance.values()) {
                    total += other.payloads.size();
                }
                if (total - dropped + 1 > maxSamples) {
                    throw new OutOfResourcesException("DURABILITY_SERVICE max_samples "
                            + maxSamples + " reached");
                }
            }
            // The limits hold; only now does a new instance enter the store.
            if (created) {
                perInstance.put(key, entry);
            }
            for (int i = 0; i < dropped; i++) {
                entry.payloads.pollFirst();
            }
            entry.payloads.addLast(new Sample(payload, timestampMillis, lifespanMillis));
            // A write brings the instance back to life: it is neither disposed nor due for purging.
            entry.disposed = false;
            entry.purgeAtMillis = -1;
            entry.cleanupDelayMillis = cleanupDelayMillis;
            entry.writers.add(writerId);
            if (entry.kind == Durability.Kind.PERSISTENT) {
                journalRetained(channel, key, payload, timestampMillis, lifespanMillis, cap,
                        cleanupDelayMillis);
            }
        }
    }

    /**
     * The instance was disposed by a writer. The store keeps it, marked disposed, until
     * no writer writes it any more and {@code service_cleanup_delay} has elapsed.
     */
    public void disposed(int domainId, String topic, String type, InstanceKey key,
                         org.omg.dds.core.policy.DurabilityService service) {
        String channel = channelKey(domainId, topic, type);
        Map<InstanceKey, Entry> perInstance = channel(channel, false);
        if (perInstance == null) {
            return;
        }
        synchronized (perInstance) {
            Entry entry = perInstance.get(key);
            if (entry != null) {
                entry.disposed = true;
                entry.cleanupDelayMillis = cleanupMillis(service);
                schedulePurge(entry);
                if (entry.kind == Durability.Kind.PERSISTENT) {
                    journalDisposed(channel, key, entry.cleanupDelayMillis);
                }
            }
        }
    }

    /**
     * {@code writerId} unregistered the instance: it no longer writes it. The store forgets
     * a disposed instance once no writer is left and {@code service_cleanup_delay} has
     * elapsed.
     */
    public void unregistered(int domainId, String topic, String type, InstanceKey key,
                             long writerId, org.omg.dds.core.policy.DurabilityService service) {
        Map<InstanceKey, Entry> perInstance = channel(channelKey(domainId, topic, type), false);
        if (perInstance == null) {
            return;
        }
        synchronized (perInstance) {
            Entry entry = perInstance.get(key);
            if (entry != null) {
                entry.writers.remove(writerId);
                entry.cleanupDelayMillis = cleanupMillis(service);
                schedulePurge(entry);
            }
        }
    }

    /** Whether the store holds the instance, after dropping what is due. For tests. */
    public boolean holds(int domainId, String topic, String type, InstanceKey key) {
        String channel = channelKey(domainId, topic, type);
        Map<InstanceKey, Entry> perInstance = channel(channel, false);
        if (perInstance == null) {
            return false;
        }
        synchronized (perInstance) {
            sweep(channel, perInstance);
            return perInstance.containsKey(key);
        }
    }

    /** The purge time of an instance that is disposed and has no writer left. */
    private void schedulePurge(Entry entry) {
        if (entry.disposed && entry.writers.isEmpty() && entry.purgeAtMillis < 0) {
            entry.purgeAtMillis = entry.cleanupDelayMillis == Long.MAX_VALUE
                    ? Long.MAX_VALUE
                    : clock.getAsLong() + entry.cleanupDelayMillis;
        }
    }

    /** Drops the instances whose cleanup delay has elapsed. */
    private void sweep(String channel, Map<InstanceKey, Entry> perInstance) {
        long now = clock.getAsLong();
        java.util.Iterator<Map.Entry<InstanceKey, Entry>> it = perInstance.entrySet().iterator();
        while (it.hasNext()) {
            Map.Entry<InstanceKey, Entry> e = it.next();
            Entry entry = e.getValue();
            if (entry.purgeAtMillis >= 0 && entry.purgeAtMillis != Long.MAX_VALUE
                    && now >= entry.purgeAtMillis) {
                if (entry.kind == Durability.Kind.PERSISTENT) {
                    journalPurged(channel, e.getKey());
                }
                it.remove();
            }
        }
    }

    /**
     * The instances of a topic: those in memory, else those replayed from the journal of a
     * PERSISTENT topic, else (when {@code create}) none yet.
     */
    private Map<InstanceKey, Entry> channel(String channel, boolean create) {
        Map<InstanceKey, Entry> existing = stores.get(channel);
        if (existing != null) {
            return existing;
        }
        Map<InstanceKey, Entry> loaded = null;
        String dir = DurabilityJournal.directory();
        if (dir != null) {
            loaded = replayJournal(dir, channel);
        }
        if (loaded == null && !create) {
            return null;
        }
        Map<InstanceKey, Entry> fresh = loaded != null ? loaded : new ConcurrentHashMap<InstanceKey, Entry>();
        Map<InstanceKey, Entry> raced = stores.putIfAbsent(channel, fresh);
        return raced != null ? raced : fresh;
    }

    private Map<InstanceKey, Entry> replayJournal(String dir, String channel) {
        final Map<InstanceKey, Entry> loaded = new ConcurrentHashMap<>();
        try {
            boolean found = DurabilityJournal.load(dir, channel, new DurabilityJournal.Sink() {
                @Override
                public void retained(byte[] key, byte[] payload, long timestampMillis,
                                     long lifespanMillis, int cap, long cleanupDelayMillis) {
                    Entry entry = loaded.get(new InstanceKey(key));
                    if (entry == null) {
                        entry = new Entry(Durability.Kind.PERSISTENT);
                        loaded.put(new InstanceKey(key), entry);
                    }
                    entry.payloads.addLast(new Sample(payload, timestampMillis, lifespanMillis));
                    while (entry.payloads.size() > cap) {
                        entry.payloads.pollFirst();
                    }
                    entry.disposed = false;
                    entry.cleanupDelayMillis = cleanupDelayMillis;
                }

                @Override
                public void disposed(byte[] key, long cleanupDelayMillis) {
                    Entry entry = loaded.get(new InstanceKey(key));
                    if (entry != null) {
                        entry.disposed = true;
                        entry.cleanupDelayMillis = cleanupDelayMillis;
                    }
                }

                @Override
                public void purged(byte[] key) {
                    loaded.remove(new InstanceKey(key));
                }
            });
            if (!found) {
                return null;
            }
        } catch (IOException e) {
            throw new OutOfResourcesException("cannot read the persistent store of " + channel
                    + ": " + e.getMessage());
        }
        // No writer of an earlier process is left: a disposed instance waits out its delay.
        for (Entry entry : loaded.values()) {
            schedulePurge(entry);
        }
        return loaded;
    }

    private static void journalRetained(String channel, InstanceKey key, byte[] payload,
                                        long timestampMillis, long lifespanMillis, int cap,
                                        long cleanupDelayMillis) {
        String dir = DurabilityJournal.directory();
        if (dir == null) {
            return;
        }
        try {
            DurabilityJournal.appendRetained(dir, channel, key.bytes(), payload, timestampMillis,
                    lifespanMillis, cap, cleanupDelayMillis);
        } catch (IOException e) {
            throw new OutOfResourcesException("cannot write the persistent store of " + channel
                    + ": " + e.getMessage());
        }
    }

    private static void journalDisposed(String channel, InstanceKey key, long cleanupDelayMillis) {
        String dir = DurabilityJournal.directory();
        if (dir == null) {
            return;
        }
        try {
            DurabilityJournal.appendDisposed(dir, channel, key.bytes(), cleanupDelayMillis);
        } catch (IOException e) {
            throw new OutOfResourcesException("cannot write the persistent store of " + channel
                    + ": " + e.getMessage());
        }
    }

    private static void journalPurged(String channel, InstanceKey key) {
        String dir = DurabilityJournal.directory();
        if (dir == null) {
            return;
        }
        try {
            DurabilityJournal.appendPurged(dir, channel, key.bytes());
        } catch (IOException e) {
            throw new OutOfResourcesException("cannot write the persistent store of " + channel
                    + ": " + e.getMessage());
        }
    }

    private static long cleanupMillis(org.omg.dds.core.policy.DurabilityService service) {
        org.omg.dds.core.Duration delay = service.getServiceCleanupDelay();
        return delay.isInfinite() ? Long.MAX_VALUE : delay.getDuration(TimeUnit.MILLISECONDS);
    }

    /**
     * Replays retained samples to a freshly-enabled reader whose requested
     * DURABILITY kind can accept them ({@code requested <= offered}). A disposed
     * instance is replayed with its samples followed by the dispose.
     */
    public void replay(int domainId, String topic, String type, InProcessBus.Endpoint reader,
                       Durability.Kind requested) {
        String channel = channelKey(domainId, topic, type);
        Map<InstanceKey, Entry> perInstance = channel(channel, false);
        if (perInstance == null) {
            return;
        }
        // Copy under the store's monitor, deliver without it: the reader's
        // listener callback runs in application code (DDS-Java-PSM 1.0 §7.2.4).
        List<InProcessBus.Message> replay = new ArrayList<>();
        synchronized (perInstance) {
            sweep(channel, perInstance);
            for (Map.Entry<InstanceKey, Entry> e : perInstance.entrySet()) {
                Entry entry = e.getValue();
                if (requested.ordinal() > entry.kind.ordinal()) {
                    continue;
                }
                for (Sample sample : entry.payloads) {
                    // The sample keeps the source timestamp it was written with, so
                    // the reader's LIFESPAN and DESTINATION_ORDER see the original.
                    replay.add(new InProcessBus.Message(sample.payload, e.getKey().bytes(),
                            InProcessBus.ChangeKind.WRITE, DURABLE_WRITER_ID, 0, false,
                            java.util.Collections.emptyList(), sample.timestampMillis)
                            .withLifespan(sample.lifespanMillis));
                }
                if (entry.disposed) {
                    replay.add(new InProcessBus.Message(null, e.getKey().bytes(),
                            InProcessBus.ChangeKind.DISPOSE, DURABLE_WRITER_ID, 0, false,
                            java.util.Collections.emptyList(), clock.getAsLong()));
                }
            }
        }
        for (InProcessBus.Message m : replay) {
            reader.onMessage(m);
        }
    }

    /**
     * Removes every non-PERSISTENT entry of a domain. Called when the domain's
     * participants are closed; PERSISTENT data outlives the participant.
     */
    public void clearDomain(int domainId) {
        String prefix = domainId + ":";
        stores.forEach((channel, perInstance) -> {
            if (channel.startsWith(prefix)) {
                perInstance.entrySet().removeIf(e -> e.getValue().kind != Durability.Kind.PERSISTENT);
            }
        });
    }

    /** Test helper — clears all durable storage and restores the system clock. */
    public void reset() {
        stores.clear();
        clock = System::currentTimeMillis;
    }
}
