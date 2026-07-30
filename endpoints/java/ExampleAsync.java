// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Deep example (async): the same sensor-telemetry flow, but the subscriber does
// not own the run-loop. A reader thread drains the transport into a
// BlockingQueue of decoded bodies; the consumer blocks on take() — the idiomatic
// java.util.concurrent model. Every field is decoded.

import java.util.concurrent.BlockingQueue;
import java.util.concurrent.ConcurrentLinkedQueue;
import java.util.concurrent.LinkedBlockingQueue;

public final class ExampleAsync {

    static final class Reading {
        final long   id;
        final float  value;
        final String label;

        Reading(long id, float value, String label) {
            this.id = id;
            this.value = value;
            this.label = label;
        }

        byte[] marshal(int endian) {
            Zdw.Writer w = new Zdw.Writer(endian);
            w.u32(id);
            w.f32(value);
            w.str(label);
            return w.bytes();
        }

        static Reading decode(byte[] body) {
            Zdw.Reader r = new Zdw.Reader(body, Zdw.LE);
            return new Reading(r.u32(), r.f32(), r.str());
        }
    }

    public static void main(String[] args) throws InterruptedException {
        final int total = 5;
        final ConcurrentLinkedQueue<byte[]> transport = new ConcurrentLinkedQueue<byte[]>();

        for (int i = 0; i < total; i++) {
            Reading rd = new Reading(0x2000L + i, 100.0f - i,
                                     String.format("sensor-%02d", i));
            transport.add(ZdwEndpoint.xrceWriteFrame(ZdwEndpoint.SESSION_NOKEY,
                ZdwEndpoint.STREAM_BEST_EFFORT, i + 1, rd.marshal(Zdw.LE)));
        }

        final BlockingQueue<byte[]> inbox = new LinkedBlockingQueue<byte[]>();
        Thread reader = new Thread(new Runnable() {
            @Override
            public void run() {
                while (!Thread.currentThread().isInterrupted()) {
                    byte[] frame = transport.poll();
                    if (frame == null) {
                        try {
                            Thread.sleep(1);
                        } catch (InterruptedException e) {
                            return;
                        }
                        continue;
                    }
                    inbox.add(ZdwEndpoint.xrceReadBody(frame));
                }
            }
        });
        reader.setDaemon(true);
        reader.start();

        for (int got = 0; got < total; got++) {
            Reading rd = Reading.decode(inbox.take());
            System.out.printf("async reading %d: id=0x%x value=%.1f label=\"%s\"%n",
                              got, rd.id, rd.value, rd.label);
        }
        reader.interrupt();
        System.out.println("ALL OK");
    }
}
