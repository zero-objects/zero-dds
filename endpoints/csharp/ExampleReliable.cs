// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Reliable endpoint demo (DDS-XRCE reliable stream), mirroring
// endpoints/rust/examples/example_reliable.rs. Modes:
//   ExampleReliable <port>  -- reliable SENDER to 127.0.0.1:<port> (used by the
//                              endpoint-e2e loss-recovery test). Enqueues N
//                              samples through the async-decoupled writer; the
//                              drain thread retransmits on ACKNACK until the
//                              window drains, then exits.
//   ExampleReliable bench   -- producer latency: non-blocking Enqueue
//                              (decoupled) vs inline Socket.Send, ns/op.
//   ExampleReliable         -- standalone in-process demo: a lossy receiver
//                              thread + the decoupled sender; the receiver
//                              prints the recovered contiguous sequence.

using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Net;
using System.Net.Sockets;
using System.Threading;
using ZeroDDS;
using ZeroDDS.Endpoint;

namespace ZeroDDS.Examples
{
    public static class ExampleReliable
    {
        private const int N = 12;

        // Reading { uint32 id; float value; string label } -- the same shape as
        // the Rust example's `Reading`, marshalled with the pure-C# XCDR2 writer
        // (ZeroDDS.Writer), byte-identical to the Rust core.
        private static byte[] Sample(int i)
        {
            var w = new Writer(Endianness.Little);
            w.PutU32((uint)i);
            w.PutF32(i);
            w.PutString($"s{i}");
            return w.Bytes();
        }

        public static int Main(string[] args)
        {
            if (args.Length >= 1 && args[0] == "bench") return Bench();
            if (args.Length >= 1 && ushort.TryParse(args[0], out ushort port)) return SendToPeer(port);
            return StandaloneDemo();
        }

        /// E2E sender: enqueue N samples through the decoupled writer, drain, exit.
        private static int SendToPeer(ushort port)
        {
            var sock = new Socket(AddressFamily.InterNetwork, SocketType.Dgram, ProtocolType.Udp);
            sock.Connect(new IPEndPoint(IPAddress.Loopback, port));
            var writer = AsyncReliableWriter.Start(sock, 64);
            for (int i = 0; i < N; i++)
            {
                var item = Sample(i);
                while (!writer.Handle.Enqueue(item)) Thread.Yield(); // ring full -> spin
            }
            writer.Shutdown(); // retransmits until the window drains, then joins
            Console.WriteLine($"SENT {N} reliable samples");
            return 0;
        }

        /// Producer latency: decoupled Enqueue (channel write, no I/O) vs. inline
        /// Socket.Send (a syscall on the producer thread). `iters` stays below the
        /// ring capacity so Enqueue never hits backpressure -- this measures the
        /// pure producer cost, not the drain thread.
        private static int Bench()
        {
            const int iters = 50_000;

            // Decoupled: the drain thread consumes; a sink thread empties the
            // socket so sends never block.
            var sink = new Socket(AddressFamily.InterNetwork, SocketType.Dgram, ProtocolType.Udp);
            sink.Bind(new IPEndPoint(IPAddress.Loopback, 0));
            int sinkPort = ((IPEndPoint)sink.LocalEndPoint!).Port;
            var sinkThread = new Thread(() =>
            {
                sink.ReceiveTimeout = 200;
                var buf = new byte[2048];
                while (true)
                {
                    try { sink.Receive(buf); }
                    catch (SocketException) { break; }
                }
            })
            { IsBackground = true };
            sinkThread.Start();

            var dsock = new Socket(AddressFamily.InterNetwork, SocketType.Dgram, ProtocolType.Udp);
            dsock.Connect(new IPEndPoint(IPAddress.Loopback, sinkPort));
            var writer = AsyncReliableWriter.Start(dsock, 1 << 20);
            var s = Sample(1);
            var sw = Stopwatch.StartNew();
            for (int i = 0; i < iters; i++)
            {
                var item = s;
                while (!writer.Handle.Enqueue(item)) Thread.Yield();
            }
            long enqueueNs = sw.ElapsedTicks * 100L / iters; // 1 Stopwatch tick == 100ns
            writer.Shutdown();

            // Inline: marshal already done; a syscall per iteration on this thread.
            var isock = new Socket(AddressFamily.InterNetwork, SocketType.Dgram, ProtocolType.Udp);
            isock.Connect(new IPEndPoint(IPAddress.Loopback, sinkPort));
            var sw2 = Stopwatch.StartNew();
            for (int i = 0; i < iters; i++)
            {
                isock.Send(ReliableWire.WriteFrame((ushort)i, s));
            }
            long inlineNs = sw2.ElapsedTicks * 100L / iters;

            Console.WriteLine($"LATENCY enqueue(decoupled)={enqueueNs}ns inline(send)={inlineNs}ns");
            return 0;
        }

        /// In-process demo: a lossy receiver (drops every 3rd distinct sample
        /// once) + the decoupled sender; the receiver prints the recovered
        /// contiguous sequence.
        private static int StandaloneDemo()
        {
            var rx = new Socket(AddressFamily.InterNetwork, SocketType.Dgram, ProtocolType.Udp);
            rx.Bind(new IPEndPoint(IPAddress.Loopback, 0));
            int rxPort = ((IPEndPoint)rx.LocalEndPoint!).Port;

            var delivered = new List<uint>();
            var receiver = new Thread(() =>
            {
                rx.ReceiveTimeout = 200;
                var state = new ReliableReceiver();
                var droppedOnce = new HashSet<ushort>();
                int count = 0;
                EndPoint? lastFrom = null;
                var deadline = DateTime.UtcNow + TimeSpan.FromSeconds(10);
                var buf = new byte[2048];
                while (delivered.Count < N && DateTime.UtcNow < deadline)
                {
                    EndPoint from = new IPEndPoint(IPAddress.Any, 0);
                    int n;
                    try { n = rx.ReceiveFrom(buf, ref from); }
                    catch (SocketException) { continue; }
                    lastFrom = from;
                    var frame = buf.AsSpan(0, n);
                    if (ReliableWire.TryUnframeWrite(frame, out var seq, out var body))
                    {
                        count++;
                        // Drop every 3rd distinct sample once, forcing one retransmit.
                        if (count % 3 == 0 && droppedOnce.Add(seq)) continue;
                        state.RecvData(seq, body);
                        foreach (var (_, payload) in state.DrainInOrder())
                        {
                            var r = new Reader(payload, Endianness.Little);
                            delivered.Add(r.GetU32());
                        }
                        rx.SendTo(ReliableWire.AckNackFrame(state.PendingAckNack(), 1), from);
                    }
                    else if (ReliableWire.TryParseHeartbeat(frame, out _))
                    {
                        rx.SendTo(ReliableWire.AckNackFrame(state.PendingAckNack(), 1), from);
                    }
                }
                // Final all-acked ACKNACKs so the sender's window drains
                // deterministically instead of relying on its close-deadline.
                if (lastFrom != null)
                {
                    for (int k = 0; k < 3; k++)
                    {
                        rx.SendTo(ReliableWire.AckNackFrame(state.PendingAckNack(), 1), lastFrom);
                    }
                }
            })
            { IsBackground = true };
            receiver.Start();

            var sock = new Socket(AddressFamily.InterNetwork, SocketType.Dgram, ProtocolType.Udp);
            sock.Connect(new IPEndPoint(IPAddress.Loopback, rxPort));
            var writer = AsyncReliableWriter.Start(sock, 64);
            for (int i = 0; i < N; i++)
            {
                var item = Sample(i);
                while (!writer.Handle.Enqueue(item)) Thread.Yield();
            }
            writer.Shutdown();
            receiver.Join(TimeSpan.FromSeconds(15));

            Console.WriteLine($"RECOVERED [{string.Join(", ", delivered)}]");
            bool ok = delivered.Count == N;
            for (int i = 0; ok && i < N; i++) ok &= delivered[i] == (uint)i;
            if (!ok)
            {
                Console.Error.WriteLine($"MISMATCH: expected 0..{N - 1} gap-free in order");
                return 1;
            }
            Console.WriteLine($"OK: {N} samples delivered gap-free in order despite injected loss");
            return 0;
        }
    }
}
