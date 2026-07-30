// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// ReliableReceiver -- receiver half of the XRCE reliable-stream state
// machine (Spec §8.4.10/§8.4.11), mirroring crates/xrce/src/reliable.rs.
// Buffers out-of-order samples, delivers them in order starting at
// `expected`, and computes the ACKNACK bitmap of missing slots.

package org.zerodds.endpoint;

import java.util.ArrayList;
import java.util.List;
import java.util.TreeMap;

public final class ReliableReceiver {

    public enum Status {
        OK,
        BUFFER_FULL
    }

    private final int stream;
    private int expectedSeq;
    private final TreeMap<Integer, byte[]> received = new TreeMap<Integer, byte[]>();

    public ReliableReceiver() {
        this(ReliableWire.STREAM_RELIABLE);
    }

    public ReliableReceiver(int stream) {
        this.stream = stream & 0xFF;
    }

    /** A sample with {@code seq + payload} arrived: buffer it (dup < expected dropped). */
    public Status recvData(int seq, byte[] payload) {
        if (ReliableWire.seqLt(seq, expectedSeq)) {
            return Status.OK; // duplicate, already delivered -> silently drop
        }
        if (received.containsKey(seq)) {
            return Status.OK; // already buffered
        }
        if (received.size() >= ReliableWire.RECV_BUF) {
            return Status.BUFFER_FULL;
        }
        received.put(seq, payload);
        return Status.OK;
    }

    /** Returns all contiguous samples available from {@code expected}, advancing it. */
    public List<byte[]> drainInOrder() {
        List<byte[]> out = new ArrayList<byte[]>();
        while (true) {
            byte[] payload = received.remove(expectedSeq);
            if (payload == null) {
                break;
            }
            out.add(payload);
            expectedSeq = (expectedSeq + 1) & 0xFFFF;
        }
        return out;
    }

    public int expected() {
        return expectedSeq;
    }

    public int outOfOrderCount() {
        return received.size();
    }

    /**
     * Computes the ACKNACK for the missing slots in {@code [expected, expected+16)}.
     * {@code hint} (nullable) is the last seqnr the sender is known to have sent
     * (from a HEARTBEAT); slots beyond it are not marked missing.
     */
    public ReliableWire.AckNack pendingAcknack(Integer hint) {
        int bitmap = 0;
        for (int i = 0; i < 16; i++) {
            int seq = (expectedSeq + i) & 0xFFFF;
            if (hint != null && ReliableWire.seqGt(seq, hint)) {
                continue;
            }
            if (!received.containsKey(seq)) {
                bitmap |= 1 << i;
            }
        }
        return new ReliableWire.AckNack(expectedSeq, bitmap & 0xFF, (bitmap >> 8) & 0xFF, stream);
    }

    public void reset() {
        expectedSeq = 0;
        received.clear();
    }
}
