// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Native pure-C# endpoint SDK (ADR 0013): a from-scratch XCDR2 wire-core on
// .NET, byte-identical to the Rust core and the other SDKs. Sync (poll) and
// async (an IAsyncEnumerable stream — the idiomatic C# async model).

using System;
using System.Collections.Generic;
using System.Runtime.CompilerServices;
using System.Text;
using System.Threading;
using System.Threading.Tasks;

namespace ZeroDDS
{
    public enum Endianness { Little, Big }

    // --- Writer: a byte buffer + endian; alignment relative to buffer start, cap 4 ---
    public sealed class Writer
    {
        private readonly List<byte> _buf = new();
        private readonly Endianness _endian;

        public Writer(Endianness endian) => _endian = endian;

        private void Align(int a)
        {
            int cap = a > 4 ? 4 : a;
            while (_buf.Count % cap != 0) _buf.Add(0);
        }

        // le: the value already encoded little-endian; reversed for big.
        private void PutLE(int a, byte[] le)
        {
            Align(a);
            if (_endian == Endianness.Big)
                for (int i = le.Length - 1; i >= 0; i--) _buf.Add(le[i]);
            else
                _buf.AddRange(le);
        }

        public void PutU8(byte v) => _buf.Add(v);
        public void PutU16(ushort v) => PutLE(2, new byte[] { (byte)v, (byte)(v >> 8) });
        public void PutU32(uint v) =>
            PutLE(4, new byte[] { (byte)v, (byte)(v >> 8), (byte)(v >> 16), (byte)(v >> 24) });

        public void PutU64(ulong v)
        {
            var le = new byte[8];
            for (int i = 0; i < 8; i++) le[i] = (byte)(v >> (8 * i));
            PutLE(4, le);
        }

        public void PutF32(float v)
        {
            var b = BitConverter.GetBytes(v);
            if (!BitConverter.IsLittleEndian) Array.Reverse(b); // logical little-endian bits
            PutLE(4, b);
        }

        // Wire rule (docs/specs/zerodds-xcdr2-csharp-1.0.md §"string"): uint32
        // (UTF-8 byte count incl. NUL) + UTF-8 bytes + NUL. The length prefix is
        // the BYTE count of the UTF-8 encoding — multibyte characters occupy more
        // than one byte, and ASCII silently corrupts non-ASCII input. Byte-
        // identical to the C/Rust core.
        public void PutString(string s)
        {
            var raw = Encoding.UTF8.GetBytes(s);
            PutU32((uint)(raw.Length + 1));
            _buf.AddRange(raw);
            PutU8(0);
        }

        public void PutSeqU8(byte[] b)
        {
            PutU32((uint)b.Length);
            _buf.AddRange(b);
        }

        public byte[] Bytes() => _buf.ToArray();
    }

    // --- Reader: a byte cursor ---
    public sealed class Reader
    {
        private readonly byte[] _data;
        private int _pos;
        private readonly Endianness _endian;

        public Reader(byte[] data, Endianness endian) { _data = data; _endian = endian; }

        private byte[] Take(int a, int n)
        {
            int cap = a > 4 ? 4 : a;
            while (_pos % cap != 0) _pos++;
            var chunk = new byte[n];
            Array.Copy(_data, _pos, chunk, 0, n);
            _pos += n;
            if (_endian == Endianness.Big) Array.Reverse(chunk);
            return chunk;
        }

        public byte GetU8() => _data[_pos++];
        public ushort GetU16() { var b = Take(2, 2); return (ushort)(b[0] | (b[1] << 8)); }
        public uint GetU32() { var b = Take(4, 4); return (uint)(b[0] | (b[1] << 8) | (b[2] << 16) | (b[3] << 24)); }

        public ulong GetU64()
        {
            var b = Take(4, 8);
            ulong v = 0;
            for (int i = 0; i < 8; i++) v |= (ulong)b[i] << (8 * i);
            return v;
        }

        public float GetF32()
        {
            var b = Take(4, 4);
            if (!BitConverter.IsLittleEndian) Array.Reverse(b);
            return BitConverter.ToSingle(b, 0);
        }

        // Inverse of PutString: read n bytes (n incl. the NUL), decode the first
        // n-1 as UTF-8.
        public string GetString()
        {
            int n = (int)GetU32();
            var s = Encoding.UTF8.GetString(_data, _pos, n - 1);
            _pos += n;
            return s;
        }

        public byte[] GetSeqU8()
        {
            int n = (int)GetU32();
            var b = new byte[n];
            Array.Copy(_data, _pos, b, 0, n);
            _pos += n;
            return b;
        }
    }

    // --- XRCE WRITE_DATA framing ---
    public static class Xrce
    {
        public const byte SessionNoKey = 0x80;
        public const byte StreamBestEffort = 0x01;

        // The 16-bit submessage_length field cannot describe a sample larger
        // than 0xFFFF; refuse rather than truncate the length while copying the
        // full payload (the C SDK rejects the same way).
        public static byte[] WriteFrame(ushort seq, byte[] sample)
        {
            if (sample.Length > 0xFFFF)
                throw new ArgumentException("sample exceeds 16-bit submessage_length", nameof(sample));
            var o = new byte[8 + sample.Length];
            o[0] = SessionNoKey;
            o[1] = StreamBestEffort;
            o[2] = (byte)(seq & 0xff);
            o[3] = (byte)((seq >> 8) & 0xff);
            o[4] = 0x07;
            o[5] = 0x03;
            o[6] = (byte)(sample.Length & 0xff);
            o[7] = (byte)((sample.Length >> 8) & 0xff);
            Array.Copy(sample, 0, o, 8, sample.Length);
            return o;
        }

        // Direction (DDS-XRCE spec §8.3.5): WRITE_DATA (0x07) is client->agent —
        // what this endpoint SENDS (WriteFrame); DATA (0x09) is agent->client —
        // what it RECEIVES from the hub. ReadFrame accepts both (parity with the
        // C/Python SDKs): 0x09 for real hub traffic, 0x07 for loopback. It
        // returns null — never throws, never a bogus sample — on a short header,
        // an unknown submessage id, or a declared length that runs past the
        // datagram (truncated / wrong length); the body is bounded by the
        // declared length so trailing bytes never leak in.
        public static byte[]? ReadFrame(byte[] frame)
        {
            if (frame.Length < 8 || (frame[4] != 0x09 && frame[4] != 0x07)) return null;
            int smLen = frame[6] | (frame[7] << 8);
            if (8 + smLen > frame.Length) return null;
            var body = new byte[smLen];
            Array.Copy(frame, 8, body, 0, smLen);
            return body;
        }
    }

    // --- transport ---
    public interface ITransport
    {
        void Deliver(byte[] frame);
        byte[]? Receive();
    }

    public sealed class MemTransport : ITransport
    {
        private readonly Queue<byte[]> _q = new();
        public void Deliver(byte[] frame) { lock (_q) _q.Enqueue(frame); }
        public byte[]? Receive() { lock (_q) return _q.Count > 0 ? _q.Dequeue() : null; }
    }

    // --- sync client ---
    public sealed class Client
    {
        private readonly ITransport _transport;
        private ushort _seq = 1;

        public Client(ITransport transport) => _transport = transport;

        public void Write(byte[] sample)
        {
            var frame = Xrce.WriteFrame(_seq, sample);
            _seq++;
            _transport.Deliver(frame);
        }

        // One non-blocking receive: the sample body, or null.
        public byte[]? Poll()
        {
            var frame = _transport.Receive();
            return frame == null ? null : Xrce.ReadFrame(frame);
        }
    }

    // --- async reader: an IAsyncEnumerable stream (idiomatic C#) ---
    public sealed class AsyncReader
    {
        private readonly ITransport _transport;
        public AsyncReader(ITransport transport) => _transport = transport;

        // await foreach (var body in reader.Stream()) { ... }
        public async IAsyncEnumerable<byte[]> Stream(
            [EnumeratorCancellation] CancellationToken ct = default)
        {
            while (!ct.IsCancellationRequested)
            {
                var frame = _transport.Receive();
                if (frame == null)
                {
                    await Task.Delay(1, ct).ConfigureAwait(false);
                    continue;
                }
                var body = Xrce.ReadFrame(frame);
                if (body != null) yield return body;
            }
        }
    }
}
