// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Deep example (sync): a realistic sensor-telemetry flow. A publisher frames
// five typed Reading(id, value, label) samples and delivers them; the
// subscriber owns the run-loop and polls, decoding EVERY field byte-for-byte.

import java.util.ArrayDeque;

public final class ExampleSync {

    // Reading { uint32 id; float value; string label }.
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

    public static void main(String[] args) {
        final int total = 5;
        ArrayDeque<byte[]> transport = new ArrayDeque<byte[]>();

        for (int i = 0; i < total; i++) {
            Reading rd = new Reading(0x1000L + i, 20.0f + i * 0.5f,
                                     String.format("bay-%02d", i));
            transport.add(ZdwEndpoint.xrceWriteFrame(ZdwEndpoint.SESSION_NOKEY,
                ZdwEndpoint.STREAM_BEST_EFFORT, i + 1, rd.marshal(Zdw.LE)));
        }

        int got = 0;
        while (got < total) {
            byte[] frame = transport.poll();
            if (frame == null) {
                break;
            }
            Reading rd = Reading.decode(ZdwEndpoint.xrceReadBody(frame));
            System.out.printf("sync reading %d: id=0x%x value=%.1f label=\"%s\"%n",
                              got, rd.id, rd.value, rd.label);
            got++;
        }

        if (got != total) {
            System.err.println("incomplete: got " + got + " of " + total);
            System.exit(1);
        }
        System.out.println("ALL OK");
    }
}
