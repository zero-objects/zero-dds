// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Reliable XRCE stream (stream_id >= 128, DDS-XRCE spec Sec 8.4.10/8.4.11) for
// the pure-C# endpoint SDK (ADR 0013). Self-contained: the state machine is
// mirrored byte-for-byte from crates/xrce/src/reliable.rs (also
// endpoints/rust/src/reliable.rs), the WRITE_DATA/HEARTBEAT/ACKNACK wire codec
// is byte-identical to endpoints/c, endpoints/rust, and crates/xrce, and
// AsyncReliableWriter is the async-decoupled writer: a bounded
// System.Threading.Channels ring (SPSC) plus a dedicated drain Thread that owns
// the ReliableSender and the UDP socket. The producer's Enqueue is a
// non-blocking channel write -- it never performs I/O; the drain thread does all
// sends, the HEARTBEAT timer, and ACKNACK-driven retransmit.
//
// Wire (little-endian; byte-identical to golden_heartbeat_le.bin /
// golden_acknack_le.bin from zerodds-endpoint-golden): an 8-byte header
// [session, stream, seq_lo, seq_hi, submsg, flags, len_lo, len_hi] then body.
// WRITE_DATA id=0x07 flags=0x03 on stream 0x80 (header seq = the sample's
// RFC-1982 seqnr). HEARTBEAT id=0x0B / ACKNACK id=0x0A, flags=0x01, on stream
// 0x00 (control); body is 5 bytes, the target reliable stream id travels in the
// body's last byte.

using System;
using System.Collections.Generic;
using System.Threading;
using System.Threading.Channels;

namespace ZeroDDS.Endpoint
{
    /// Wire constants and the frame codec.
    public static class ReliableWire
    {
        public const byte Session = 0x80;
        public const byte StreamReliable = 0x80;
        public const byte StreamNone = 0x00;
        public const byte SmWriteData = 0x07;
        public const byte SmAckNack = 0x0A;
        public const byte SmHeartbeat = 0x0B;
        public const byte WriteFlags = 0x03;
        public const byte CtrlFlags = 0x01;

        /// Sender window cap -- matches the 16-bit ACKNACK bitmap.
        public const int Window = 16;

        /// Receiver out-of-order buffer cap (DoS bound).
        public const int ReceiverBuffer = 64;

        /// Per-sample payload cap (u16 submessage length limit).
        public const int MaxPayload = 65_535;

        /// Heartbeat period (spec recommends 100 ms; 500 ms conservative without a
        /// Tx pacing layer underneath).
        public static readonly TimeSpan HeartbeatPeriod = TimeSpan.FromMilliseconds(500);

        /// RFC-1982 16-bit "a is strictly before b" (wrapping serial comparison).
        public static bool SeqLt(ushort a, ushort b) => (short)(a - b) < 0;

        /// RFC-1982 16-bit "a is strictly after b".
        public static bool SeqGt(ushort a, ushort b) => (short)(a - b) > 0;

        /// Builds a reliable WRITE_DATA frame; the header seq carries the sample's
        /// RFC-1982 seqnr.
        public static byte[] WriteFrame(ushort seq, byte[] sample)
        {
            var o = new byte[8 + sample.Length];
            o[0] = Session;
            o[1] = StreamReliable;
            o[2] = (byte)(seq & 0xFF);
            o[3] = (byte)(seq >> 8);
            o[4] = SmWriteData;
            o[5] = WriteFlags;
            ushort n = (ushort)sample.Length;
            o[6] = (byte)(n & 0xFF);
            o[7] = (byte)(n >> 8);
            Array.Copy(sample, 0, o, 8, sample.Length);
            return o;
        }

        /// Parses a reliable WRITE_DATA frame into (seq, sample).
        public static bool TryUnframeWrite(ReadOnlySpan<byte> frame, out ushort seq, out byte[] sample)
        {
            if (frame.Length < 8 || frame[4] != SmWriteData)
            {
                seq = 0;
                sample = Array.Empty<byte>();
                return false;
            }
            seq = (ushort)(frame[2] | (frame[3] << 8));
            sample = frame[8..].ToArray();
            return true;
        }

        private static byte[] CtrlHeader(byte submsg, ushort msgSeq) => new byte[]
        {
            Session, StreamNone,
            (byte)(msgSeq & 0xFF), (byte)(msgSeq >> 8),
            submsg, CtrlFlags, 5, 0,
        };

        /// Builds a HEARTBEAT control frame. Byte-identical to golden_heartbeat_le.bin
        /// when hb = (1, 3, 0x80) and msgSeq = 1.
        public static byte[] HeartbeatFrame(Heartbeat hb, ushort msgSeq)
        {
            var f = new List<byte>(13);
            f.AddRange(CtrlHeader(SmHeartbeat, msgSeq));
            f.Add((byte)(hb.First & 0xFF));
            f.Add((byte)((hb.First >> 8) & 0xFF));
            f.Add((byte)(hb.Last & 0xFF));
            f.Add((byte)((hb.Last >> 8) & 0xFF));
            f.Add(hb.StreamId);
            return f.ToArray();
        }

        /// Parses a HEARTBEAT frame (keys on the submessage id; header otherwise ignored).
        public static bool TryParseHeartbeat(ReadOnlySpan<byte> frame, out Heartbeat hb)
        {
            if (frame.Length < 13 || frame[4] != SmHeartbeat)
            {
                hb = default;
                return false;
            }
            hb = new Heartbeat(
                (short)(frame[8] | (frame[9] << 8)),
                (short)(frame[10] | (frame[11] << 8)),
                frame[12]);
            return true;
        }

        /// Builds an ACKNACK control frame. Byte-identical to golden_acknack_le.bin
        /// when ack = (1, bitmap=0, 0x80) and msgSeq = 1.
        public static byte[] AckNackFrame(AckNack ack, ushort msgSeq)
        {
            var f = new List<byte>(13);
            f.AddRange(CtrlHeader(SmAckNack, msgSeq));
            f.Add((byte)(ack.FirstUnacked & 0xFF));
            f.Add((byte)((ack.FirstUnacked >> 8) & 0xFF));
            f.Add(ack.NackLo);
            f.Add(ack.NackHi);
            f.Add(ack.StreamId);
            return f.ToArray();
        }

        /// Parses an ACKNACK frame (keys on the submessage id; header otherwise ignored).
        public static bool TryParseAckNack(ReadOnlySpan<byte> frame, out AckNack ack)
        {
            if (frame.Length < 13 || frame[4] != SmAckNack)
            {
                ack = default;
                return false;
            }
            ack = new AckNack(
                (short)(frame[8] | (frame[9] << 8)),
                frame[10],
                frame[11],
                frame[12]);
            return true;
        }
    }

    /// HEARTBEAT body.
    public readonly struct Heartbeat
    {
        public readonly short First;
        public readonly short Last;
        public readonly byte StreamId;

        public Heartbeat(short first, short last, byte streamId)
        {
            First = first;
            Last = last;
            StreamId = streamId;
        }
    }

    /// ACKNACK body. The 16-bit NACK bitmap travels as two little-endian bytes
    /// (bit i set => seqnr FirstUnacked+i still missing).
    public readonly struct AckNack
    {
        public readonly short FirstUnacked;
        public readonly byte NackLo;
        public readonly byte NackHi;
        public readonly byte StreamId;

        public AckNack(short firstUnacked, byte nackLo, byte nackHi, byte streamId)
        {
            FirstUnacked = firstUnacked;
            NackLo = nackLo;
            NackHi = nackHi;
            StreamId = streamId;
        }

        public ushort Bitmap => (ushort)(NackLo | (NackHi << 8));
    }

    public enum SubmitStatus { Ok, PayloadTooLarge, WindowFull }

    public enum RecvStatus { Ok, BufferFull }

    /// Reliable sender: assigns seqnrs, holds in-flight samples until
    /// acknowledged, emits HEARTBEATs, and clears/keeps samples on ACKNACK.
    /// Mirrors the sender half of crates/xrce/src/reliable.rs ReliableStreamState.
    public sealed class ReliableSender
    {
        private ushort _nextSeq;
        private readonly SortedDictionary<ushort, byte[]> _inFlight = new();
        private DateTime? _lastHeartbeat;

        /// In-flight (unacknowledged) sample count.
        public int InFlightCount => _inFlight.Count;

        /// Submits a sample, assigning it the next seqnr.
        public SubmitStatus Submit(byte[] payload, out ushort seq)
        {
            seq = 0;
            if (payload.Length > ReliableWire.MaxPayload) return SubmitStatus.PayloadTooLarge;
            if (_inFlight.Count >= ReliableWire.Window) return SubmitStatus.WindowFull;
            seq = _nextSeq;
            _inFlight[seq] = payload;
            _nextSeq = (ushort)(_nextSeq + 1);
            return SubmitStatus.Ok;
        }

        /// The in-flight payload for `seq`, if still unacknowledged (for retransmit).
        public byte[]? GetInFlight(ushort seq) => _inFlight.TryGetValue(seq, out var p) ? p : null;

        /// All in-flight seqnrs, ascending (for retransmit against a NACK bitmap).
        public IReadOnlyList<ushort> InFlightSeqs()
        {
            var seqs = new List<ushort>(_inFlight.Count);
            seqs.AddRange(_inFlight.Keys);
            return seqs;
        }

        /// Returns a HEARTBEAT if the period elapsed and samples are in flight.
        public Heartbeat? PendingHeartbeat(DateTime now)
        {
            if (_inFlight.Count == 0) return null;
            bool due = _lastHeartbeat is null || (now - _lastHeartbeat.Value) >= ReliableWire.HeartbeatPeriod;
            if (!due) return null;
            _lastHeartbeat = now;
            // RFC-1982 window base (oldest unacked) + end (newest unacked); NOT the
            // numeric SortedDictionary first/last key, which is wrong across a
            // 16-bit wrap (window 0xFFFE,0xFFFF,0x0000,0x0001 -> base 0xFFFE / end
            // 0x0001, not 0x0000 / 0xFFFF). Mirrors window_base /
            // serial_max_in_flight in crates/xrce/src/reliable.rs.
            ushort first = 0, last = 0;
            bool any = false;
            foreach (var k in _inFlight.Keys)
            {
                if (!any)
                {
                    first = k;
                    last = k;
                    any = true;
                }
                else
                {
                    if (ReliableWire.SeqLt(k, first)) first = k;
                    if (ReliableWire.SeqGt(k, last)) last = k;
                }
            }
            return new Heartbeat((short)first, (short)last, ReliableWire.StreamReliable);
        }

        /// Processes an ACKNACK: drops everything strictly before `FirstUnacked`
        /// and every clear-bit slot in [FirstUnacked, FirstUnacked+16); keeps
        /// set-bit (still-missing) slots for retransmit.
        public void RecvAckNack(AckNack ack)
        {
            ushort baseSeq = (ushort)ack.FirstUnacked;
            ushort bitmap = ack.Bitmap;
            var toDrop = new List<ushort>();
            foreach (var k in _inFlight.Keys)
            {
                if (ReliableWire.SeqLt(k, baseSeq)) toDrop.Add(k);
            }
            foreach (var k in toDrop) _inFlight.Remove(k);
            for (int i = 0; i < 16; i++)
            {
                ushort seq = (ushort)(baseSeq + i);
                if (((bitmap >> i) & 1) == 0) _inFlight.Remove(seq);
            }
        }

        /// Re-arms the heartbeat clock so the next tick fires immediately
        /// (used by the drain loop while it waits out a close-drain).
        internal void ResetHeartbeatClock() => _lastHeartbeat = null;
    }

    /// Reliable receiver: buffers out-of-order samples, delivers them
    /// contiguously, and reports the missing slots via ACKNACK.
    public sealed class ReliableReceiver
    {
        private ushort _expected;
        private readonly SortedDictionary<ushort, byte[]> _received = new();

        /// Next expected incoming seqnr.
        public ushort Expected => _expected;

        /// Buffered out-of-order sample count.
        public int OutOfOrderCount => _received.Count;

        /// Accepts a sample; drops duplicates before `Expected` or already buffered.
        public RecvStatus RecvData(ushort seq, byte[] payload)
        {
            if (ReliableWire.SeqLt(seq, _expected)) return RecvStatus.Ok; // duplicate, drop
            if (_received.ContainsKey(seq)) return RecvStatus.Ok; // already buffered
            if (_received.Count >= ReliableWire.ReceiverBuffer) return RecvStatus.BufferFull;
            _received[seq] = payload;
            return RecvStatus.Ok;
        }

        /// Delivers all contiguous samples from `Expected`, advancing it.
        public List<(ushort Seq, byte[] Payload)> DrainInOrder()
        {
            var outp = new List<(ushort, byte[])>();
            while (_received.TryGetValue(_expected, out var payload))
            {
                outp.Add((_expected, payload));
                _received.Remove(_expected);
                _expected = (ushort)(_expected + 1);
            }
            return outp;
        }

        /// Computes the ACKNACK marking the missing slots in [Expected, Expected+16).
        /// A clear bit unambiguously means "received" when `hint` is omitted; with
        /// a `hint` (e.g. the HEARTBEAT's last-in-flight seqnr), slots strictly
        /// after it are left un-marked (not yet sent by the peer).
        public AckNack PendingAckNack(ushort? hint = null)
        {
            ushort bitmap = 0;
            for (int i = 0; i < 16; i++)
            {
                ushort seq = (ushort)(_expected + i);
                if (hint.HasValue && ReliableWire.SeqGt(seq, hint.Value)) continue;
                if (!_received.ContainsKey(seq)) bitmap |= (ushort)(1 << i);
            }
            return new AckNack((short)_expected, (byte)(bitmap & 0xFF), (byte)(bitmap >> 8), ReliableWire.StreamReliable);
        }

        /// Clears all receiver state.
        public void Reset()
        {
            _expected = 0;
            _received.Clear();
        }
    }

    /// Shared closed-flag between a `ReliableWriterHandle` and its
    /// `AsyncReliableWriter`. Tracked independently of the channel's own
    /// completion (which only completes once the channel has also been fully
    /// *drained* -- exactly the condition the drain thread needs to detect
    /// separately, since a stalled window can leave items unread indefinitely).
    internal sealed class WriterCloseState
    {
        private volatile bool _closed;
        public bool Closed => _closed;
        public void SetClosed() => _closed = true;
    }

    /// Producer-side handle to the async-decoupled writer. `Enqueue` is a
    /// non-blocking channel write -- it never performs I/O; the drain thread does
    /// all sends. `Close` signals the drain thread that no more samples are
    /// coming, once the send window has drained it exits.
    public sealed class ReliableWriterHandle
    {
        private readonly ChannelWriter<byte[]> _writer;
        private readonly WriterCloseState _state;

        internal ReliableWriterHandle(ChannelWriter<byte[]> writer, WriterCloseState state)
        {
            _writer = writer;
            _state = state;
        }

        /// Non-blocking enqueue. Returns false (backpressure) if the ring is full;
        /// the caller should retry (e.g. after a `Thread.Yield()`), the same
        /// contract as the Rust/D/C SPSC-ring writers.
        public bool Enqueue(byte[] sample) => _writer.TryWrite(sample);

        /// Signals the drain thread that no more samples will be enqueued.
        public void Close()
        {
            _state.SetClosed();
            _writer.TryComplete();
        }
    }

    /// The async-decoupled reliable writer: a bounded, single-writer/single-reader
    /// System.Threading.Channels ring plus a dedicated drain Thread that owns the
    /// ReliableSender and the UDP socket. Mirrors
    /// endpoints/rust/src/reliable.rs::AsyncReliableWriter.
    public sealed class AsyncReliableWriter : IDisposable
    {
        private readonly Channel<byte[]> _channel;
        private readonly System.Net.Sockets.Socket _sock;
        private readonly Thread _drain;
        private readonly WriterCloseState _closeState = new();
        private bool _shutdownCalled;

        /// A handle the producer(s) hold; Enqueue never touches the kernel.
        public ReliableWriterHandle Handle { get; }

        private AsyncReliableWriter(System.Net.Sockets.Socket sock, int capacity)
        {
            _sock = sock;
            _channel = Channel.CreateBounded<byte[]>(new BoundedChannelOptions(capacity)
            {
                SingleWriter = true,
                SingleReader = true,
                FullMode = BoundedChannelFullMode.Wait,
            });
            Handle = new ReliableWriterHandle(_channel.Writer, _closeState);
            _drain = new Thread(DrainLoop) { IsBackground = true, Name = "zerodds-reliable-drain" };
            _drain.Start();
        }

        /// Spawns the drain thread. `sock` must already target the peer (connected
        /// UDP socket); the drain thread owns it exclusively from here on.
        public static AsyncReliableWriter Start(System.Net.Sockets.Socket sock, int capacity = 1024) =>
            new(sock, capacity);

        private void DrainLoop()
        {
            _sock.ReceiveTimeout = 5;
            var sender = new ReliableSender();
            ushort ctlSeq = 1;
            var buf = new byte[2048];
            DateTime? closeDeadline = null;

            while (true)
            {
                // 1) Drain the channel into the sender window; send fresh WRITE_DATA.
                while (sender.InFlightCount < ReliableWire.Window && _channel.Reader.TryRead(out var sample))
                {
                    if (sender.Submit(sample, out var seq) == SubmitStatus.Ok)
                    {
                        TrySend(ReliableWire.WriteFrame(seq, sample));
                    }
                }

                // 2) HEARTBEAT when due.
                var hb = sender.PendingHeartbeat(DateTime.UtcNow);
                if (hb.HasValue)
                {
                    TrySend(ReliableWire.HeartbeatFrame(hb.Value, ctlSeq));
                    ctlSeq++;
                }

                // 3) Drain incoming ACKNACKs -> retransmit the still-missing samples.
                while (TryReceive(buf, out int n) && n > 0)
                {
                    if (ReliableWire.TryParseAckNack(buf.AsSpan(0, n), out var ack))
                    {
                        sender.RecvAckNack(ack);
                        ushort baseSeq = (ushort)ack.FirstUnacked;
                        ushort bitmap = ack.Bitmap;
                        for (int i = 0; i < 16; i++)
                        {
                            if (((bitmap >> i) & 1) != 1) continue;
                            ushort seq = (ushort)(baseSeq + i);
                            var p = sender.GetInFlight(seq);
                            if (p != null) TrySend(ReliableWire.WriteFrame(seq, p));
                        }
                    }
                }

                // 4) Exit once closed AND the window has drained (ring empty, every
                //    in-flight sample acknowledged) -- or, as a safety valve, once
                //    the post-close drain deadline elapses (a vanished peer -- or a
                //    permanently-full window with nobody ACKing -- must never wedge
                //    Shutdown() forever).
                if (_closeState.Closed)
                {
                    closeDeadline ??= DateTime.UtcNow + TimeSpan.FromSeconds(5);
                    bool ringEmpty = !_channel.Reader.TryPeek(out _);
                    if (sender.InFlightCount == 0 && ringEmpty) break;
                    if (DateTime.UtcNow > closeDeadline) break; // best-effort give-up
                    // Keep heartbeating so the peer keeps ACKing until the window empties.
                    sender.ResetHeartbeatClock();
                }
            }
        }

        private void TrySend(byte[] frame)
        {
            try { _sock.Send(frame); }
            catch (System.Net.Sockets.SocketException) { /* best-effort */ }
        }

        private bool TryReceive(byte[] buf, out int n)
        {
            try
            {
                n = _sock.Receive(buf);
                return true;
            }
            catch (System.Net.Sockets.SocketException)
            {
                n = 0;
                return false;
            }
        }

        /// Closes the writer and blocks until the drain thread finishes (the
        /// window has drained, or the best-effort deadline elapsed). Idempotent.
        public void Shutdown()
        {
            if (_shutdownCalled)
            {
                _drain.Join();
                return;
            }
            _shutdownCalled = true;
            Handle.Close();
            _drain.Join();
        }

        public void Dispose() => Shutdown();
    }
}
