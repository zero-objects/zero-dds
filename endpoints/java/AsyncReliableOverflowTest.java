// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Regression test for AsyncReliableWriter's drain-deadline fix.
//
// The pre-fix drain loop armed a single global deadline at construction, so a
// long-lived writer that never calls finish() was torn down after ~20s (here:
// after the injected short deadline), silently dropping every sample submitted
// afterwards. This test submits two bursts separated by MORE than the drain
// deadline WITHOUT calling finish(), and requires that every sample is still
// delivered -- i.e. the deadline bounds only the post-finish() drain phase, not
// the writer's lifetime. It then verifies finish() drains + terminates promptly.
//
// A lossless in-process UDP loopback peer receives the WRITE_DATA frames and
// returns cumulative ACKNACKs.

import java.net.DatagramPacket;
import java.net.DatagramSocket;
import java.net.InetAddress;
import java.net.SocketTimeoutException;
import java.util.Arrays;
import java.util.concurrent.atomic.AtomicBoolean;
import java.util.concurrent.atomic.AtomicInteger;

import org.zerodds.endpoint.AsyncReliableWriter;
import org.zerodds.endpoint.ReliableReceiver;
import org.zerodds.endpoint.ReliableWire;

public final class AsyncReliableOverflowTest {

    private AsyncReliableOverflowTest() {}

    private static void check(boolean cond, String msg) {
        if (!cond) {
            System.out.println("FAIL: " + msg);
            throw new AssertionError(msg);
        }
    }

    public static void main(String[] args) throws Exception {
        final int total = 40;
        final long drainDeadlineMs = 400;

        final DatagramSocket peer = new DatagramSocket(0, InetAddress.getLoopbackAddress());
        peer.setSoTimeout(50);
        final int peerPort = peer.getLocalPort();

        final ReliableReceiver rcv = new ReliableReceiver();
        final AtomicInteger delivered = new AtomicInteger(0);
        final AtomicBoolean peerRun = new AtomicBoolean(true);

        Thread peerThread = new Thread(new Runnable() {
            @Override
            public void run() {
                byte[] buf = new byte[8192];
                while (peerRun.get()) {
                    try {
                        DatagramPacket pkt = new DatagramPacket(buf, buf.length);
                        peer.receive(pkt);
                        byte[] frame = Arrays.copyOf(buf, pkt.getLength());
                        ReliableWire.WriteData wd = ReliableWire.parseWrite(frame);
                        if (wd == null) {
                            continue; // HEARTBEAT etc.
                        }
                        if (rcv.recvData(wd.seq, wd.sample) != ReliableReceiver.Status.OK) {
                            continue;
                        }
                        delivered.addAndGet(rcv.drainInOrder().size());
                        // cumulative ACKNACK: everything < expected acknowledged.
                        ReliableWire.AckNack ack =
                                new ReliableWire.AckNack(rcv.expected(), 0, 0, 0x80);
                        byte[] af = ReliableWire.acknackFrame(ack);
                        peer.send(new DatagramPacket(af, af.length, pkt.getAddress(), pkt.getPort()));
                    } catch (SocketTimeoutException e) {
                        // idle poll
                    } catch (Exception e) {
                        if (peerRun.get()) {
                            System.out.println("peer error: " + e);
                        }
                        return;
                    }
                }
            }
        }, "test-peer");
        peerThread.setDaemon(true);
        peerThread.start();

        AsyncReliableWriter w = new AsyncReliableWriter(
                InetAddress.getLoopbackAddress(), peerPort, drainDeadlineMs);
        w.start();

        // Burst 1 at t0.
        for (int i = 0; i < total / 2; i++) {
            w.submit(new byte[] {(byte) i});
        }
        // Gap LONGER than the drain deadline, with NO finish(): the pre-fix writer
        // dies here and loses burst 2.
        Thread.sleep(drainDeadlineMs + 300);
        // Burst 2.
        for (int i = total / 2; i < total; i++) {
            w.submit(new byte[] {(byte) i});
        }

        // Wait (bounded) for all samples, then finish() must drain + terminate.
        for (int k = 0; k < 100 && delivered.get() < total; k++) {
            Thread.sleep(50);
        }
        long t0 = System.currentTimeMillis();
        boolean fully = w.finish(3000);
        long finishMs = System.currentTimeMillis() - t0;
        w.close();
        peerRun.set(false);
        peer.close();

        check(delivered.get() == total,
                "long-lived writer delivered " + delivered.get() + " of " + total
                        + " (global deadline killed it?)");
        check(fully, "finish() reported not fully drained");
        check(finishMs < 3000, "finish() did not terminate promptly (" + finishMs + "ms)");

        System.out.println("ALL OK (delivered " + total + "/" + total
                + ", no premature termination, finish in " + finishMs + "ms)");
    }
}
