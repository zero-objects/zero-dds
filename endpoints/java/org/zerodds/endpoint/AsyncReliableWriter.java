// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// AsyncReliableWriter -- async-decoupled reliable writer (ADR 0013) for the
// pure-Java endpoint SDK. Mirrors endpoints/c's zerodds_reliable_async.[ch]
// (SPSC ring + drain thread) and endpoints/d's reliable.d `SpscRing`, using
// the idiomatic `java.util.concurrent` primitive: the producer enqueues a
// raw sample into a bounded `BlockingQueue` -- no socket syscall, no lock
// contention on the hot path -- and returns. A single drain `Thread` owns
// the `ReliableSender` state end-to-end: it pulls queued samples into the
// send window, frames + sends WRITE_DATA, fires HEARTBEAT on a period gate,
// and on every inbound ACKNACK prunes acknowledged samples (via
// ReliableSender#recvAcknack) and retransmits the ones the peer still marks
// missing. This folds the C/D split (ring-only bench primitive vs. a
// separate synchronous network loop) into one thread, because the real XRCE
// reliable protocol needs the ACKNACK round-trip to drive retransmission --
// not just a self-acking bench producer.

package org.zerodds.endpoint;

import java.io.IOException;
import java.net.DatagramPacket;
import java.net.DatagramSocket;
import java.net.InetAddress;
import java.net.SocketTimeoutException;
import java.util.Arrays;
import java.util.concurrent.ArrayBlockingQueue;
import java.util.concurrent.BlockingQueue;
import java.util.concurrent.atomic.AtomicBoolean;
import java.util.concurrent.atomic.AtomicLong;

public final class AsyncReliableWriter implements AutoCloseable {

    private static final int QUEUE_CAPACITY = 4096;
    private static final int SOCKET_POLL_MS = 20;
    private static final long DRAIN_DEADLINE_MS = 20_000;

    private final BlockingQueue<byte[]> queue = new ArrayBlockingQueue<byte[]>(QUEUE_CAPACITY);
    private final ReliableSender sender = new ReliableSender();
    private final DatagramSocket socket;
    private final InetAddress peerHost;
    private final int peerPort;
    private final long drainDeadlineMs;
    private final Thread drainThread;
    private final AtomicBoolean producerDone = new AtomicBoolean(false);
    private final AtomicBoolean stopped = new AtomicBoolean(false);
    private final AtomicLong deliveredCount = new AtomicLong(0);
    private volatile boolean drained;
    private volatile Exception drainError;

    public AsyncReliableWriter(InetAddress peerHost, int peerPort) throws IOException {
        this(peerHost, peerPort, DRAIN_DEADLINE_MS);
    }

    /**
     * @param drainDeadlineMs how long the drain thread keeps flushing AFTER
     *     {@link #finish(long)}/{@link #close()} signals no more input, before it
     *     gives up on an unacknowledged window. It bounds the drain phase only --
     *     it never terminates a live, still-producing writer.
     */
    public AsyncReliableWriter(InetAddress peerHost, int peerPort, long drainDeadlineMs)
            throws IOException {
        this.peerHost = peerHost;
        this.peerPort = peerPort;
        this.drainDeadlineMs = drainDeadlineMs;
        this.socket = new DatagramSocket();
        this.socket.setSoTimeout(SOCKET_POLL_MS);
        this.drainThread = new Thread(new Runnable() {
            @Override
            public void run() {
                drainLoop();
            }
        }, "zdw-reliable-drain");
        this.drainThread.setDaemon(true);
    }

    public void start() {
        drainThread.start();
    }

    /** Producer hot path: enqueue only (no syscall). Blocks only if the queue is momentarily full. */
    public void submit(byte[] payload) {
        try {
            queue.put(payload);
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
        }
    }

    /** Non-blocking enqueue (used by the producer-latency bench). */
    public boolean offer(byte[] payload) {
        return queue.offer(payload);
    }

    /**
     * Signals that no more samples will be submitted, then blocks (up to
     * {@code timeoutMs}) until the drain thread has emptied both the queue
     * and the reliable send window (i.e. every sample acknowledged).
     * Returns whether it fully drained.
     */
    public boolean finish(long timeoutMs) {
        producerDone.set(true);
        try {
            drainThread.join(timeoutMs);
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
        }
        return drained;
    }

    public long delivered() {
        return deliveredCount.get();
    }

    public Exception drainError() {
        return drainError;
    }

    private void drainLoop() {
        byte[] buf = new byte[8192];
        // Set only once the producer signals done (finish/close); a still-live
        // writer must never be bounded by a global lifetime deadline -- that was
        // the bug: a long-lived writer got killed after 20s without finish().
        long drainDeadline = -1L;
        try {
            while (!stopped.get()) {
                // 1) Move queued samples into the sender's window: submit -> history
                //    (in-flight map) -> frame -> send WRITE_DATA.
                while (sender.inFlightCount() < ReliableWire.WINDOW) {
                    byte[] payload = queue.poll();
                    if (payload == null) {
                        break;
                    }
                    ReliableSender.SubmitResult r = sender.submit(payload);
                    if (r.status != ReliableSender.Status.OK) {
                        // Window was just checked above; only a payload-too-large
                        // sample could land here -- drop it rather than wedge the loop.
                        break;
                    }
                    send(ReliableWire.writeFrame(r.seq, payload));
                    deliveredCount.incrementAndGet();
                }

                // 2) HEARTBEAT timer (first immediately, then every 500ms while
                //    samples remain unacknowledged).
                ReliableWire.Heartbeat hb = sender.pendingHeartbeat(System.currentTimeMillis());
                if (hb != null) {
                    send(ReliableWire.heartbeatFrame(hb));
                }

                // 3) Drain one inbound ACKNACK (short socket timeout so this never
                //    blocks the producer-queue drain above for long).
                int n;
                try {
                    DatagramPacket pkt = new DatagramPacket(buf, buf.length);
                    socket.receive(pkt);
                    n = pkt.getLength();
                } catch (SocketTimeoutException e) {
                    n = -1;
                }
                if (n >= 0) {
                    byte[] frame = Arrays.copyOf(buf, n);
                    ReliableWire.AckNack ack = ReliableWire.parseAckNack(frame);
                    if (ack != null) {
                        sender.recvAcknack(ack); // prune: drops every acknowledged sample
                        retransmitMissing(ack);
                    }
                }

                // 4) Only after the producer signals done: drain the queue + send
                //    window, but no longer than drainDeadlineMs so an unacked
                //    window (dead peer) cannot hang the thread forever.
                if (producerDone.get()) {
                    if (queue.isEmpty() && sender.inFlightCount() == 0) {
                        drained = true;
                        break;
                    }
                    if (drainDeadline < 0) {
                        drainDeadline = System.currentTimeMillis() + drainDeadlineMs;
                    }
                    if (System.currentTimeMillis() >= drainDeadline) {
                        break;
                    }
                }
            }
        } catch (IOException e) {
            drainError = e;
        } finally {
            stopped.set(true);
        }
    }

    private void retransmitMissing(ReliableWire.AckNack ack) throws IOException {
        int base = ack.firstUnacked;
        int bitmap = ack.bitmap();
        for (int i = 0; i < 16; i++) {
            if (((bitmap >> i) & 1) == 0) {
                continue; // acknowledged or outside the sender's in-flight set
            }
            int seq = (base + i) & 0xFFFF;
            byte[] payload = sender.getInFlight(seq);
            if (payload != null) {
                send(ReliableWire.writeFrame(seq, payload));
            }
        }
    }

    private void send(byte[] frame) throws IOException {
        socket.send(new DatagramPacket(frame, frame.length, peerHost, peerPort));
    }

    @Override
    public void close() {
        stopped.set(true);
        socket.close();
    }
}
