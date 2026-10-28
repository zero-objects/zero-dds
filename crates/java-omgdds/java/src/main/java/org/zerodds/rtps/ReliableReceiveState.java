// SPDX-License-Identifier: Apache-2.0
package org.zerodds.rtps;

import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import java.util.TreeMap;
import java.util.TreeSet;

/** Ordered receive window. A high out-of-order SN never acknowledges a hole. */
final class ReliableReceiveState {
    private long contiguous;
    private long pendingBytes;
    private final TreeMap<Long, RtpsMessage.Data> pending = new TreeMap<>();
    private final TreeSet<Long> irrelevant = new TreeSet<>();

    synchronized List<RtpsMessage.Data> offer(RtpsMessage.Data data) {
        if (data.writerSn <= contiguous) return new ArrayList<>();
        if (data.writerSn == contiguous + 1) {
            contiguous++;
            List<RtpsMessage.Data> ready = new ArrayList<>();
            ready.add(data);
            ready.addAll(drain());
            return ready;
        }
        RtpsMessage.Data previous = pending.get(data.writerSn);
        if (previous != null && !Arrays.equals(previous.serializedPayload, data.serializedPayload))
            throw new IllegalArgumentException("Conflicting duplicate RTPS sample");
        if (previous == null && (pending.size() >= 256
                || pendingBytes + data.serializedPayload.length > 16L * 1024 * 1024))
            throw new IllegalStateException("Reliable receive window exhausted");
        if (previous == null) pendingBytes += data.serializedPayload.length;
        pending.put(data.writerSn, data);
        return drain();
    }

    synchronized List<RtpsMessage.Data> heartbeat(long first) {
        // Earlier samples no longer belong to the writer's advertised history.
        List<RtpsMessage.Data> ready = new ArrayList<>();
        if (first > contiguous + 1) {
            ready.addAll(pending.headMap(first).values());
            for (RtpsMessage.Data data : ready) pendingBytes -= data.serializedPayload.length;
            pending.headMap(first).clear();
            irrelevant.headSet(first).clear();
            contiguous = first - 1;
        }
        ready.addAll(drain());
        return ready;
    }

    synchronized List<RtpsMessage.Data> gap(long start, long end) {
        if (start < 1 || end < start) throw new IllegalArgumentException("Invalid RTPS GAP");
        if (start <= contiguous + 1) {
            contiguous = Math.max(contiguous, end - 1);
            for (RtpsMessage.Data discarded : pending.headMap(end).values()) pendingBytes -= discarded.serializedPayload.length;
            pending.headMap(end).clear();
            irrelevant.headSet(end).clear();
        } else {
            if (end - start > 256 || irrelevant.size() + end - start > 256) throw new IllegalArgumentException("GAP exceeds receive window");
            for (long sequence = start; sequence < end; sequence++) irrelevant.add(sequence);
        }
        return drain();
    }

    synchronized long base() { return contiguous + 1; }

    private List<RtpsMessage.Data> drain() {
        List<RtpsMessage.Data> ready = new ArrayList<>();
        while (contiguous != Long.MAX_VALUE) {
            long next = contiguous + 1;
            if (irrelevant.remove(next)) {
                RtpsMessage.Data discarded = pending.remove(next);
                if (discarded != null) pendingBytes -= discarded.serializedPayload.length;
                contiguous = next; continue;
            }
            RtpsMessage.Data data = pending.remove(next);
            if (data == null) break;
            pendingBytes -= data.serializedPayload.length;
            contiguous = next;
            ready.add(data);
        }
        return ready;
    }
}
