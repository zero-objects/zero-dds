// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Unit suite mirroring crates/xrce/src/reliable.rs + byte-golden assertion
// for the org.zerodds.endpoint reliable-stream classes.
// Usage: ReliableSelfTest [golden_heartbeat_le.bin golden_acknack_le.bin]
// Prints "ALL OK" and exits 0 on success.

import java.nio.file.Files;
import java.nio.file.Paths;
import java.util.Arrays;
import java.util.List;

import org.zerodds.endpoint.ReliableReceiver;
import org.zerodds.endpoint.ReliableSender;
import org.zerodds.endpoint.ReliableWire;

public final class ReliableSelfTest {

    private ReliableSelfTest() {}

    private static void check(boolean cond, String msg) {
        if (!cond) {
            System.out.println("FAIL: " + msg);
            throw new AssertionError(msg);
        }
    }

    public static void main(String[] args) throws Exception {
        // --- Sender ---
        {
            ReliableSender s = new ReliableSender();
            ReliableSender.SubmitResult a = s.submit(new byte[] {1, 2});
            ReliableSender.SubmitResult b = s.submit(new byte[] {3, 4});
            check(a.status == ReliableSender.Status.OK && a.seq == 0, "monotonic seq 0");
            check(b.status == ReliableSender.Status.OK && b.seq == 1, "monotonic seq 1");
            check(s.inFlightCount() == 2, "in-flight count");
        }
        {
            ReliableSender s = new ReliableSender();
            byte[] huge = new byte[ReliableWire.MAX_PAYLOAD + 1];
            check(s.submit(huge).status == ReliableSender.Status.PAYLOAD_TOO_LARGE, "payload too large");
        }
        {
            ReliableSender s = new ReliableSender();
            for (int i = 0; i < ReliableWire.WINDOW; i++) {
                check(s.submit(new byte[] {0}).status == ReliableSender.Status.OK, "fill window");
            }
            check(s.submit(new byte[] {0}).status == ReliableSender.Status.WINDOW_FULL, "window full");
        }
        {
            ReliableSender s = new ReliableSender();
            s.submit(new byte[] {1});
            long base = 1_000_000L; // arbitrary epoch; only deltas matter
            ReliableWire.Heartbeat hb = s.pendingHeartbeat(base);
            check(hb != null, "heartbeat fires first");
            check(hb.first == 0 && hb.last == 0 && hb.stream == 0x80, "heartbeat body");
            check(s.pendingHeartbeat(base + 100) == null, "heartbeat silenced <500ms");
            check(s.pendingHeartbeat(base + 600) != null, "heartbeat after 500ms");
        }
        {
            ReliableSender s = new ReliableSender();
            check(s.pendingHeartbeat(0L) == null, "no heartbeat when empty");
        }
        {
            ReliableSender s = new ReliableSender();
            s.submit(new byte[] {(byte) 0xA0}); // seq 0
            s.submit(new byte[] {(byte) 0xA1}); // seq 1
            s.submit(new byte[] {(byte) 0xA2}); // seq 2
            // base=2, bitmap=0b1 -> seq2 still missing, 0+1 acknowledged
            s.recvAcknack(new ReliableWire.AckNack(2, 0x01, 0x00, 0x80));
            check(s.inFlightCount() == 1, "acknack clears acked");
            check(s.getInFlight(2) != null, "seq2 retransmittable");
        }
        {
            ReliableSender s = new ReliableSender();
            for (int i = 0; i < 5; i++) {
                s.submit(new byte[] {0});
            }
            s.recvAcknack(new ReliableWire.AckNack(5, 0, 0, 0x80)); // full clear
            check(s.inFlightCount() == 0, "acknack full clear");
        }

        // --- Receiver ---
        {
            ReliableReceiver r = new ReliableReceiver();
            r.recvData(0, new byte[] {10});
            r.recvData(1, new byte[] {11});
            List<byte[]> d = r.drainInOrder();
            check(d.size() == 2 && d.get(0)[0] == 10 && d.get(1)[0] == 11, "in-order drain");
            check(r.expected() == 2, "expected advanced");
        }
        {
            ReliableReceiver r = new ReliableReceiver();
            r.recvData(2, new byte[] {22});
            r.recvData(0, new byte[] {20});
            List<byte[]> d1 = r.drainInOrder();
            check(d1.size() == 1 && d1.get(0)[0] == 20, "reorder: only seq0");
            r.recvData(1, new byte[] {21});
            List<byte[]> d2 = r.drainInOrder();
            check(d2.size() == 2 && d2.get(0)[0] == 21 && d2.get(1)[0] == 22, "reorder: 1+2");
        }
        {
            ReliableReceiver r = new ReliableReceiver();
            r.recvData(0, new byte[] {1});
            r.drainInOrder();
            r.recvData(0, new byte[] {99}); // duplicate
            check(r.outOfOrderCount() == 0, "duplicate dropped");
        }
        {
            ReliableReceiver r = new ReliableReceiver();
            for (int i = 1; i <= ReliableWire.RECV_BUF; i++) {
                check(r.recvData(i, new byte[] {1}) == ReliableReceiver.Status.OK, "fill recv buffer");
            }
            check(
                r.recvData(ReliableWire.RECV_BUF + 1, new byte[] {1}) == ReliableReceiver.Status.BUFFER_FULL,
                "recv buffer full");
        }
        {
            ReliableReceiver r = new ReliableReceiver();
            r.recvData(1, new byte[] {1});
            r.recvData(3, new byte[] {3});
            ReliableWire.AckNack a = r.pendingAcknack(3);
            int bm = a.bitmap();
            check((bm & 1) != 0, "slot 0 missing");
            check((bm & (1 << 2)) != 0, "slot 2 missing");
            check((bm & (1 << 1)) == 0, "slot 1 present");
            check((bm & (1 << 3)) == 0, "slot 3 present");
        }
        {
            ReliableReceiver r = new ReliableReceiver();
            r.recvData(0, new byte[] {3});
            r.reset();
            check(r.expected() == 0 && r.outOfOrderCount() == 0, "reset clears receiver");
        }

        // --- end-to-end loss recovery (in-process) ---
        {
            ReliableSender s = new ReliableSender();
            ReliableReceiver r = new ReliableReceiver();
            int[] seqs = new int[3];
            for (int i = 0; i < 3; i++) {
                seqs[i] = s.submit(new byte[] {(byte) i}).seq;
            }
            r.recvData(seqs[0], new byte[] {0}); // seq1 lost
            r.recvData(seqs[2], new byte[] {2});
            List<byte[]> d = r.drainInOrder();
            check(d.size() == 1, "only seq0 before recovery");
            ReliableWire.AckNack ack = r.pendingAcknack(seqs[2]);
            s.recvAcknack(ack);
            check(s.getInFlight(seqs[1]) != null, "seq1 retransmittable");
            r.recvData(seqs[1], s.getInFlight(seqs[1]));
            List<byte[]> d2 = r.drainInOrder();
            check(d2.size() == 2, "seq1+2 after recovery");
        }

        // --- byte-golden ---
        byte[] hb = ReliableWire.heartbeatFrame(new ReliableWire.Heartbeat(1, 3, 0x80));
        byte[] hbExpect = new byte[] {
            (byte) 0x80, 0x00, 0x01, 0x00, 0x0b, 0x01, 0x05, 0x00, 0x01, 0x00, 0x03, 0x00, (byte) 0x80
        };
        check(Arrays.equals(hb, hbExpect), "heartbeat byte-golden (hardcoded)");

        byte[] ak = ReliableWire.acknackFrame(new ReliableWire.AckNack(1, 0, 0, 0x80));
        byte[] akExpect = new byte[] {
            (byte) 0x80, 0x00, 0x01, 0x00, 0x0a, 0x01, 0x05, 0x00, 0x01, 0x00, 0x00, 0x00, (byte) 0x80
        };
        check(Arrays.equals(ak, akExpect), "acknack byte-golden (hardcoded)");

        if (args.length >= 2) {
            byte[] gHb = Files.readAllBytes(Paths.get(args[0]));
            byte[] gAk = Files.readAllBytes(Paths.get(args[1]));
            check(Arrays.equals(hb, gHb), "heartbeat byte-identical to golden file");
            check(Arrays.equals(ak, gAk), "acknack byte-identical to golden file");
            System.out.println("golden files matched");
        }

        System.out.println("ALL OK");
    }
}
