// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Producer-latency micro-bench: the async-decoupled writer's BlockingQueue
// enqueue vs. an inline UDP DatagramSocket#send. Prints median ns for both --
// the syscall the enqueue avoids on the producer's hot path. Mirrors
// endpoints/d/reliable_bench.d (SpscRing.enqueue vs. sendTo).

import java.net.DatagramPacket;
import java.net.DatagramSocket;
import java.net.InetAddress;
import java.util.Arrays;
import java.util.concurrent.ArrayBlockingQueue;
import java.util.concurrent.BlockingQueue;

import org.zerodds.endpoint.ReliableWire;

public final class ReliableBench {

    private ReliableBench() {}

    public static void main(String[] args) throws Exception {
        final int iters = 20_000;
        byte[] frame = ReliableWire.writeFrame(0, new byte[] {1, 2, 3, 4});

        InetAddress host = InetAddress.getByName("127.0.0.1");
        DatagramSocket sink = new DatagramSocket(0, host); // bound, never read -- a real destination

        // Inline path: a real sendto per sample (kernel transition).
        DatagramSocket sock = new DatagramSocket();
        DatagramPacket pkt = new DatagramPacket(frame, frame.length, host, sink.getLocalPort());
        long[] inlineNs = new long[iters];
        for (int i = 0; i < iters; i++) {
            long t0 = System.nanoTime();
            sock.send(pkt);
            inlineNs[i] = System.nanoTime() - t0;
        }

        // Decoupled path: BlockingQueue enqueue only (no syscall on the producer).
        // A daemon drain thread keeps the queue from saturating, the way
        // AsyncReliableWriter's real drain thread would.
        final BlockingQueue<byte[]> queue = new ArrayBlockingQueue<byte[]>(1024);
        final Thread drain = new Thread(new Runnable() {
            @Override
            public void run() {
                try {
                    while (!Thread.currentThread().isInterrupted()) {
                        queue.take();
                    }
                } catch (InterruptedException e) {
                    // stop
                }
            }
        }, "bench-drain");
        drain.setDaemon(true);
        drain.start();

        long[] queueNs = new long[iters];
        for (int i = 0; i < iters; i++) {
            long t0 = System.nanoTime();
            queue.offer(frame);
            queueNs[i] = System.nanoTime() - t0;
        }
        drain.interrupt();

        Arrays.sort(inlineNs);
        Arrays.sort(queueNs);
        long inlineMed = inlineNs[iters / 2];
        long queueMed = queueNs[iters / 2];
        double ratio = queueMed == 0 ? (double) inlineMed : (double) inlineMed / (double) queueMed;
        System.out.printf(
            "producer latency: queue-enqueue median = %d ns, inline-send median = %d ns (%.0fx)%n",
            queueMed, inlineMed, ratio);

        sock.close();
        sink.close();
    }
}
