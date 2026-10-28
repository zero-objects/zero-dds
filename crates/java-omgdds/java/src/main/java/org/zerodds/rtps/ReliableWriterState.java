// SPDX-License-Identifier: Apache-2.0
package org.zerodds.rtps;

import java.net.InetSocketAddress;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.TreeMap;

/** Shared RTPS send history for user and builtin writers, not a service codec. */
final class ReliableWriterState {
    /** Bound of the unacknowledged samples of an application writer; unlimited unless an option sets it. */
    static final int MAX_SAMPLES = RtpsTimings.CURRENT.sendHistoryMaxSamples;
    static final long MAX_BYTES = RtpsTimings.CURRENT.sendHistoryMaxBytes;
    /**
     * Bound of the history of a built-in discovery writer: its history is KEEP_LAST 1 of each endpoint
     * (DDSI-RTPS 2.5 8.4.15); an announcement of a peer that never acknowledges must not stop the others.
     */
    static final int BUILTIN_MAX_SAMPLES = 256;
    static final class Target {
        final byte[] prefix, reader;
        final InetSocketAddress address;
        Target(byte[] prefix, byte[] reader, InetSocketAddress address) {
            this.prefix = prefix.clone(); this.reader = reader.clone(); this.address = address;
        }
        String key() { return org.zerodds.cdr.HexBytes.encode(new Guid(prefix, reader).bytes()); }
    }
    static final class Sample {
        final long sequence;
        final byte[] payload;
        final long timestampMillis = System.currentTimeMillis();
        final Map<String, Target> targets = new LinkedHashMap<>();
        /** The inline QoS the DATA of this sample carries, or null. */
        final ParameterList inlineQos;
        /** The group sequence number of this sample, 0 if its writer is not in a group. */
        long gsn;
        Sample(long sequence, byte[] payload, ParameterList inlineQos) {
            this.sequence = sequence;
            this.payload = payload.clone();
            this.inlineQos = inlineQos;
        }
    }
    final byte[] writer;
    private final TreeMap<Long, Sample> history = new TreeMap<>();
    private final Map<String, Integer> ackCounts = new LinkedHashMap<>();
    private final Map<String, Integer> fragmentCounts = new LinkedHashMap<>();
    private long bytes, lastSequence;
    private int heartbeatCount;

    ReliableWriterState(byte[] writer) { this.writer = writer.clone(); }

    synchronized Sample add(long sequence, byte[] payload, Target target) {
        return add(sequence, payload, target, null);
    }

    synchronized Sample add(long sequence, byte[] payload, Target target, ParameterList inlineQos) {
        return add(sequence, payload, target, inlineQos, 0L);
    }

    synchronized Sample add(long sequence, byte[] payload, Target target, ParameterList inlineQos, long gsn) {
        if (sequence <= 0) throw new IllegalArgumentException("Positive RTPS sequence required");
        Sample sample = history.get(sequence);
        if (sample == null) {
            if (sequence <= lastSequence) throw new IllegalStateException("RTPS writer sequence reused");
            if (history.size() >= MAX_SAMPLES || bytes + payload.length > MAX_BYTES || bytes + payload.length < 0)
                throw new IllegalStateException("Reliable writer history exhausted before acknowledgement");
            sample = new Sample(sequence, payload, inlineQos);
            sample.gsn = gsn;
            if (gsn > lastGroupGsn) {
                lastGroupGsn = gsn;
            }
            history.put(sequence, sample);
            bytes += payload.length;
            lastSequence = sequence;
        } else if (!Arrays.equals(sample.payload, payload)) {
            throw new IllegalArgumentException("Conflicting payload for RTPS writer sequence");
        }
        sample.targets.put(target.key(), target);
        return sample;
    }

    /**
     * A newer sample of the same instance replaces the one under {@code sequence} (HISTORY
     * KEEP_LAST 1 of the built-in endpoints, DDSI-RTPS 2.5 8.4.15): it leaves the history, and a
     * reader that asks for it is sent a GAP.
     */
    synchronized void supersede(long sequence) {
        Sample old = history.remove(sequence);
        if (old != null) {
            bytes -= old.payload.length;
            notifyAll();
        }
    }

    /**
     * Makes room for one more sample: the oldest sample not yet acknowledged leaves the history when it
     * is full. For the built-in discovery writers, whose history is KEEP_LAST 1 of each endpoint
     * (DDSI-RTPS 2.5 8.4.15), the oldest announcement is the one a newer announcement of the endpoint
     * or the loss of the peer makes void; a peer that never acknowledges must not stop announcements
     * to the others.
     *
     * @return the sequence number that left, or 0 if there was room.
     */
    synchronized long makeRoom() {
        if (history.size() < BUILTIN_MAX_SAMPLES) {
            return 0;
        }
        long oldest = history.firstKey();
        supersede(oldest);
        return oldest;
    }

    synchronized boolean acknowledge(byte[] prefix, RtpsMessage.AckNack ack) {
        String key = org.zerodds.cdr.HexBytes.encode(new Guid(prefix, ack.readerId).bytes());
        if (ack.base < 1 || ack.base > lastSequence + 1) return false;
        Integer previous = ackCounts.get(key);
        if (previous != null && Integer.compareUnsigned(ack.count - previous, 0x80000000) >= 0) return false;
        if (previous != null && ack.count == previous) return false;
        boolean known = false;
        for (Sample sample : history.values()) if (sample.targets.containsKey(key)) { known = true; break; }
        if (!known) return false;
        ackCounts.put(key, ack.count);
        java.util.Iterator<Map.Entry<Long, Sample>> iterator = history.entrySet().iterator();
        while (iterator.hasNext()) {
            Sample sample = iterator.next().getValue();
            if (sample.sequence < ack.base) sample.targets.remove(key);
            if (sample.targets.isEmpty()) { bytes -= sample.payload.length; iterator.remove(); }
        }
        notifyAll();
        return true;
    }

    /** Whether every sample has been acknowledged by every target it was sent to. */
    synchronized boolean allAcknowledged() {
        return history.isEmpty();
    }

    /**
     * Blocks until {@link #allAcknowledged()} or {@code timeoutNanos} have passed
     * ({@code Long.MAX_VALUE} waits without a limit).
     *
     * @return whether every sample was acknowledged.
     */
    synchronized boolean awaitAcknowledged(long timeoutNanos) throws InterruptedException {
        long start = System.nanoTime();
        while (!history.isEmpty()) {
            if (timeoutNanos == Long.MAX_VALUE) {
                wait();
                continue;
            }
            long remaining = timeoutNanos - (System.nanoTime() - start);
            if (remaining <= 0) {
                return false;
            }
            wait(remaining / 1_000_000L, (int) (remaining % 1_000_000L));
        }
        return true;
    }

    /**
     * Waits until fewer than {@code maxSamples} samples are unacknowledged (the history of a
     * KEEP_ALL writer is bounded by RESOURCE_LIMITS, DDS 1.4 2.2.3.19).
     *
     * @return whether there is room, false if {@code timeoutNanos} passed first.
     */
    synchronized boolean awaitSpace(int maxSamples, long timeoutNanos) throws InterruptedException {
        long start = System.nanoTime();
        while (history.size() >= maxSamples) {
            if (timeoutNanos == Long.MAX_VALUE) {
                wait();
                continue;
            }
            long remaining = timeoutNanos - (System.nanoTime() - start);
            if (remaining <= 0) {
                return false;
            }
            wait(remaining / 1_000_000L, (int) (remaining % 1_000_000L));
        }
        return true;
    }

    /**
     * The target the samples for the reader {@code prefix}/{@code reader} went to: the locator of
     * its reader proxy, or {@code null} if no sample of the history was sent to it.
     */
    synchronized Target target(byte[] prefix, byte[] reader) {
        String key = org.zerodds.cdr.HexBytes.encode(new Guid(prefix, reader).bytes());
        for (Sample sample : history.values()) {
            Target target = sample.targets.get(key);
            if (target != null) {
                return target;
            }
        }
        return null;
    }

    synchronized Sample requested(long sequence, Target target) {
        Sample sample = history.get(sequence);
        return sample != null && sample.targets.containsKey(target.key()) ? sample : null;
    }

    synchronized List<Target> targets() {
        Map<String, Target> targets = new LinkedHashMap<>();
        for (Sample sample : history.values()) targets.putAll(sample.targets);
        return new ArrayList<>(targets.values());
    }

    private long lastGroupGsn;

    /** The last sequence number this writer sent. */
    synchronized long lastSequence() {
        return lastSequence;
    }

    /**
     * The group sequence numbers of this writer: the first one still in its history and the last one
     * it wrote ({0, 0}: none; first = last + 1 if the history holds none of them).
     */
    synchronized long[] groupRange() {
        long first = 0;
        for (Sample sample : history.values()) {
            if (sample.gsn > 0) {
                first = sample.gsn;
                break;
            }
        }
        if (first == 0) {
            first = lastGroupGsn + 1;
        }
        return new long[] {first, lastGroupGsn};
    }

    synchronized long[] range(Target target) {
        long first = 0, last = 0;
        for (Sample sample : history.values()) if (sample.targets.containsKey(target.key())) {
            if (first == 0) first = sample.sequence;
            last = sample.sequence;
        }
        return new long[] {first, last};
    }

    synchronized int nextHeartbeatCount() { return ++heartbeatCount; }

    synchronized void removePeer(String prefix) {
        ackCounts.keySet().removeIf(key -> key.startsWith(prefix));
        fragmentCounts.keySet().removeIf(key -> key.startsWith(prefix));
        java.util.Iterator<Sample> iterator = history.values().iterator();
        while (iterator.hasNext()) {
            Sample sample = iterator.next();
            sample.targets.keySet().removeIf(key -> key.startsWith(prefix));
            if (sample.targets.isEmpty()) { bytes -= sample.payload.length; iterator.remove(); }
        }
        notifyAll();
    }

    synchronized boolean acceptFragmentCount(Target target, int count) {
        Integer previous = fragmentCounts.get(target.key());
        if (previous != null && (count == previous || Integer.compareUnsigned(count - previous, 0x80000000) >= 0)) return false;
        fragmentCounts.put(target.key(), count);
        return true;
    }
}
