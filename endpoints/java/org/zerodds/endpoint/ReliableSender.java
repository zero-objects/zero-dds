// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// ReliableSender -- sender half of the XRCE reliable-stream state machine
// (Spec §8.4.10/§8.4.11), mirroring crates/xrce/src/reliable.rs. Buffers
// submitted samples until acknowledged, fires HEARTBEAT on a period gate,
// and prunes/retains in-flight samples per an incoming ACKNACK bitmap.
//
// Sequence numbers are plain `int` in [0,65535] (RFC-1982 16-bit space);
// `TreeMap` gives natural ascending order for the window bounds and iterates
// the same way `BTreeMap<u16,_>` does in the Rust reference (both share the
// same non-circular first/last caveat right at the 65536 wraparound, which
// never occurs within one window of WINDOW=16 in-flight samples).

package org.zerodds.endpoint;

import java.util.ArrayList;
import java.util.List;
import java.util.TreeMap;

public final class ReliableSender {

    public enum Status {
        OK,
        PAYLOAD_TOO_LARGE,
        WINDOW_FULL
    }

    public static final class SubmitResult {
        public final Status status;
        public final int seq;

        SubmitResult(Status status, int seq) {
            this.status = status;
            this.seq = seq;
        }
    }

    private final int stream;
    private int nextSeq;
    private final TreeMap<Integer, byte[]> inFlight = new TreeMap<Integer, byte[]>();
    private boolean haveHeartbeat;
    private long lastHeartbeatMs;

    public ReliableSender() {
        this(ReliableWire.STREAM_RELIABLE);
    }

    public ReliableSender(int stream) {
        this.stream = stream & 0xFF;
    }

    /** Submits a new sample. Assigns the next monotonic seqnr on success. */
    public SubmitResult submit(byte[] payload) {
        if (payload.length > ReliableWire.MAX_PAYLOAD) {
            return new SubmitResult(Status.PAYLOAD_TOO_LARGE, -1);
        }
        if (inFlight.size() >= ReliableWire.WINDOW) {
            return new SubmitResult(Status.WINDOW_FULL, -1);
        }
        int seq = nextSeq;
        inFlight.put(seq, payload);
        nextSeq = (nextSeq + 1) & 0xFFFF;
        return new SubmitResult(Status.OK, seq);
    }

    public int inFlightCount() {
        return inFlight.size();
    }

    /** In-flight payload lookup (e.g. for retransmit). Null if not (or no longer) in-flight. */
    public byte[] getInFlight(int seq) {
        return inFlight.get(seq);
    }

    /** Ascending in-flight seqnrs (deterministic retransmit order). */
    public List<Integer> inFlightSeqs() {
        return new ArrayList<Integer>(inFlight.keySet());
    }

    /**
     * Tick: returns a HEARTBEAT if the heartbeat period elapsed and in-flight
     * samples exist; {@code null} otherwise.
     */
    public ReliableWire.Heartbeat pendingHeartbeat(long nowMs) {
        if (inFlight.isEmpty()) {
            return null;
        }
        boolean due = !haveHeartbeat || (nowMs - lastHeartbeatMs) >= ReliableWire.HEARTBEAT_PERIOD_MS;
        if (!due) {
            return null;
        }
        haveHeartbeat = true;
        lastHeartbeatMs = nowMs;
        return new ReliableWire.Heartbeat(inFlight.firstKey(), inFlight.lastKey(), stream);
    }

    /**
     * Processes an incoming ACKNACK: everything strictly before
     * {@code firstUnacked} is acknowledged, and within the 16-slot bitmap
     * window a clear bit means acknowledged (prune) while a set bit means
     * still missing (retain for retransmit).
     */
    public void recvAcknack(ReliableWire.AckNack ack) {
        int base = ack.firstUnacked;
        int bitmap = ack.bitmap();

        List<Integer> before = new ArrayList<Integer>();
        for (Integer k : inFlight.keySet()) {
            if (ReliableWire.seqLt(k, base)) {
                before.add(k);
            }
        }
        for (Integer k : before) {
            inFlight.remove(k);
        }

        for (int i = 0; i < 16; i++) {
            int seq = (base + i) & 0xFFFF;
            if (((bitmap >> i) & 1) == 0) {
                inFlight.remove(seq);
            }
        }
    }

    /** Resets the stream state (e.g. after a RESET submessage). */
    public void reset() {
        nextSeq = 0;
        inFlight.clear();
        haveHeartbeat = false;
    }
}
