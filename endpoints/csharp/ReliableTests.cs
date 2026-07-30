// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Unit suite for Reliable.cs, mirroring crates/xrce/src/reliable.rs's own test
// module (and endpoints/d/reliable_test.d) plus a byte-golden assertion for the
// HEARTBEAT/ACKNACK wire codec.
//
// usage: ReliableTests [golden_heartbeat_le.bin golden_acknack_le.bin]
// Prints "ALL OK" and exits 0 on success (nonzero + "N FAILURE(S)" otherwise).

using System;
using System.IO;
using ZeroDDS.Endpoint;

namespace ZeroDDS.Tests
{
    public static class ReliableTests
    {
        private static int _failures;

        private static void Check(bool cond, string msg)
        {
            if (!cond)
            {
                Console.WriteLine($"FAIL: {msg}");
                _failures++;
            }
        }

        private static bool Eq(byte[] a, byte[] b)
        {
            if (a.Length != b.Length) return false;
            for (int i = 0; i < a.Length; i++)
            {
                if (a[i] != b[i]) return false;
            }
            return true;
        }

        public static int Main(string[] args)
        {
            // ---- Sender ----
            {
                var s = new ReliableSender();
                Check(s.Submit(new byte[] { 1, 2 }, out var a) == SubmitStatus.Ok && a == 0, "monotonic seq 0");
                Check(s.Submit(new byte[] { 3, 4 }, out var b) == SubmitStatus.Ok && b == 1, "monotonic seq 1");
                Check(s.InFlightCount == 2, "in-flight count");
            }
            {
                var s = new ReliableSender();
                var huge = new byte[ReliableWire.MaxPayload + 1];
                Check(s.Submit(huge, out _) == SubmitStatus.PayloadTooLarge, "payload too large");
            }
            {
                var s = new ReliableSender();
                for (int i = 0; i < ReliableWire.Window; i++)
                {
                    Check(s.Submit(new byte[] { 0 }, out _) == SubmitStatus.Ok, "fill window");
                }
                Check(s.Submit(new byte[] { 0 }, out _) == SubmitStatus.WindowFull, "window full");
            }
            {
                var s = new ReliableSender();
                s.Submit(new byte[] { 1 }, out _);
                var t0 = DateTime.UtcNow;
                var hb = s.PendingHeartbeat(t0);
                Check(hb.HasValue, "heartbeat fires first");
                Check(hb!.Value.First == 0 && hb.Value.Last == 0 && hb.Value.StreamId == 0x80, "heartbeat body");
                Check(!s.PendingHeartbeat(t0.AddMilliseconds(100)).HasValue, "heartbeat silenced <500ms");
                Check(s.PendingHeartbeat(t0.AddMilliseconds(600)).HasValue, "heartbeat after 500ms");
            }
            {
                var s = new ReliableSender();
                Check(!s.PendingHeartbeat(DateTime.UtcNow).HasValue, "no heartbeat when empty");
            }
            {
                var s = new ReliableSender();
                s.Submit(new byte[] { 0xA0 }, out _); // seq 0
                s.Submit(new byte[] { 0xA1 }, out _); // seq 1
                s.Submit(new byte[] { 0xA2 }, out _); // seq 2
                // base=2, bitmap=0b1 -> seq2 missing, 0+1 acked.
                s.RecvAckNack(new AckNack(2, 0x01, 0x00, 0x80));
                Check(s.InFlightCount == 1, "acknack clears acked");
                Check(s.GetInFlight(2) != null, "seq2 retransmittable");
            }
            {
                var s = new ReliableSender();
                for (int i = 0; i < 5; i++)
                {
                    s.Submit(new byte[] { 0 }, out _);
                }
                s.RecvAckNack(new AckNack(5, 0, 0, 0x80)); // full clear
                Check(s.InFlightCount == 0, "acknack full clear");
            }

            // ---- Receiver ----
            {
                var r = new ReliableReceiver();
                r.RecvData(0, new byte[] { 10 });
                r.RecvData(1, new byte[] { 11 });
                var d = r.DrainInOrder();
                Check(d.Count == 2 && d[0].Payload[0] == 10 && d[1].Payload[0] == 11, "in-order drain");
                Check(r.Expected == 2, "expected advanced");
            }
            {
                var r = new ReliableReceiver();
                r.RecvData(2, new byte[] { 22 });
                r.RecvData(0, new byte[] { 20 });
                var d1 = r.DrainInOrder();
                Check(d1.Count == 1 && d1[0].Payload[0] == 20, "reorder: only seq0");
                r.RecvData(1, new byte[] { 21 });
                var d2 = r.DrainInOrder();
                Check(d2.Count == 2 && d2[0].Payload[0] == 21 && d2[1].Payload[0] == 22, "reorder: seq1+2");
            }
            {
                var r = new ReliableReceiver();
                r.RecvData(0, new byte[] { 1 });
                r.DrainInOrder();
                r.RecvData(0, new byte[] { 99 }); // duplicate
                Check(r.OutOfOrderCount == 0, "duplicate dropped");
            }
            {
                var r = new ReliableReceiver();
                for (ushort i = 1; i <= ReliableWire.ReceiverBuffer; i++)
                {
                    Check(r.RecvData(i, new byte[] { 1 }) == RecvStatus.Ok, "fill recv buffer");
                }
                Check(
                    r.RecvData((ushort)(ReliableWire.ReceiverBuffer + 1), new byte[] { 1 }) == RecvStatus.BufferFull,
                    "recv buffer full");
            }
            {
                var r = new ReliableReceiver();
                r.RecvData(1, new byte[] { 1 });
                r.RecvData(3, new byte[] { 3 });
                var a = r.PendingAckNack(3);
                ushort bm = a.Bitmap;
                Check((bm & 1) != 0, "slot 0 missing");
                Check((bm & (1 << 2)) != 0, "slot 2 missing");
                Check((bm & (1 << 1)) == 0, "slot 1 present");
                Check((bm & (1 << 3)) == 0, "slot 3 present");
            }
            {
                var r = new ReliableReceiver();
                r.RecvData(0, new byte[] { 1 });
                r.Reset();
                Check(r.Expected == 0 && r.OutOfOrderCount == 0, "reset clears receiver");
            }

            // ---- end-to-end loss recovery (in-process) ----
            {
                var s = new ReliableSender();
                var r = new ReliableReceiver();
                var seqs = new ushort[3];
                for (int i = 0; i < 3; i++)
                {
                    s.Submit(new byte[] { (byte)i }, out seqs[i]);
                }
                r.RecvData(seqs[0], new byte[] { 0 }); // seq1 lost
                r.RecvData(seqs[2], new byte[] { 2 });
                var d = r.DrainInOrder();
                Check(d.Count == 1, "only seq0 before recovery");
                var ack = r.PendingAckNack(seqs[2]);
                s.RecvAckNack(ack);
                Check(s.GetInFlight(seqs[1]) != null, "seq1 retransmittable");
                r.RecvData(seqs[1], new byte[] { 1 });
                var d2 = r.DrainInOrder();
                Check(d2.Count == 2, "seq1+2 after recovery");
            }

            // ---- RFC-1982 regression: HEARTBEAT window + loss recovery across
            //      the 16-bit wrap (mirrors crates/xrce's wrap regression tests).
            //      Seeds sender/receiver up to the wrap via the public API only
            //      (Submit + full-ack / RecvData + drain), then straddles 0x0000.
            {
                var s = new ReliableSender();
                ushort seq; // walk nextSeq to 0xFFFE: submit one, fully-ack it, repeat.
                do
                {
                    s.Submit(new byte[] { 0 }, out seq);
                    s.RecvAckNack(new AckNack((short)(seq + 1), 0, 0, 0x80));
                } while (seq != 0xFFFD);
                Check(s.InFlightCount == 0, "wrap seed: sender window drained");

                s.Submit(new byte[] { 10 }, out var q0); // 0xFFFE
                s.Submit(new byte[] { 11 }, out var q1); // 0xFFFF (lost)
                s.Submit(new byte[] { 12 }, out var q2); // 0x0000
                s.Submit(new byte[] { 13 }, out var q3); // 0x0001
                Check(q0 == 0xFFFE && q1 == 0xFFFF && q2 == 0x0000 && q3 == 0x0001, "wrap seqs");

                var hbw = s.PendingHeartbeat(DateTime.UtcNow);
                Check(hbw.HasValue && (ushort)hbw!.Value.First == 0xFFFE && (ushort)hbw.Value.Last == 0x0001,
                    "heartbeat window across wrap = [0xFFFE,0x0001] (not numeric 0x0000,0xFFFF)");

                var r = new ReliableReceiver(); // seed expected to 0xFFFE.
                for (int k = 0; k <= 0xFFFD; k++)
                {
                    r.RecvData((ushort)k, new byte[] { 0 });
                    r.DrainInOrder();
                }
                Check(r.Expected == 0xFFFE, "wrap seed: receiver expects 0xFFFE");

                r.RecvData(q0, new byte[] { 10 }); // 0xFFFF lost
                r.RecvData(q2, new byte[] { 12 });
                r.RecvData(q3, new byte[] { 13 });
                var dw = r.DrainInOrder();
                Check(dw.Count == 1 && dw[0].Payload[0] == 10, "only 0xFFFE before recovery");
                Check(r.Expected == 0xFFFF, "receiver blocked at 0xFFFF");

                var ackw = r.PendingAckNack(q3);
                Check((ushort)ackw.FirstUnacked == 0xFFFF, "acknack base = 0xFFFF across wrap");
                Check((ackw.Bitmap & 0b1) != 0, "0xFFFF NACKed");
                Check((ackw.Bitmap & 0b110) == 0, "0x0000/0x0001 present");

                s.RecvAckNack(ackw);
                Check(s.GetInFlight(q1) != null, "0xFFFF retransmittable");
                Check(s.GetInFlight(q0) == null && s.GetInFlight(q2) == null && s.GetInFlight(q3) == null,
                    "0xFFFE/0x0000/0x0001 acked");
                Check(s.InFlightCount == 1, "only 0xFFFF left in-flight");

                r.RecvData(q1, new byte[] { 11 });
                var dw2 = r.DrainInOrder();
                Check(dw2.Count == 3 && dw2[0].Payload[0] == 11 && dw2[1].Payload[0] == 12 && dw2[2].Payload[0] == 13,
                    "0xFFFF,0x0000,0x0001 deliver in RFC-1982 order");
            }

            // ---- byte-golden ----
            var hbFrame = ReliableWire.HeartbeatFrame(new Heartbeat(1, 3, 0x80), 1);
            byte[] hbExpect = { 0x80, 0x00, 0x01, 0x00, 0x0b, 0x01, 0x05, 0x00, 0x01, 0x00, 0x03, 0x00, 0x80 };
            Check(Eq(hbFrame, hbExpect), "heartbeat byte-golden (hardcoded)");

            var akFrame = ReliableWire.AckNackFrame(new AckNack(1, 0, 0, 0x80), 1);
            byte[] akExpect = { 0x80, 0x00, 0x01, 0x00, 0x0a, 0x01, 0x05, 0x00, 0x01, 0x00, 0x00, 0x00, 0x80 };
            Check(Eq(akFrame, akExpect), "acknack byte-golden (hardcoded)");

            if (args.Length >= 2)
            {
                var gHb = File.ReadAllBytes(args[0]);
                var gAk = File.ReadAllBytes(args[1]);
                Check(Eq(hbFrame, gHb), "heartbeat byte-identical to golden file");
                Check(Eq(akFrame, gAk), "acknack byte-identical to golden file");
                Console.WriteLine("golden files matched");
            }

            if (_failures > 0)
            {
                Console.WriteLine($"{_failures} FAILURE(S)");
                return 1;
            }
            Console.WriteLine("ALL OK");
            return 0;
        }
    }
}
