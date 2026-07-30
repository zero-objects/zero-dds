// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Native pure-Swift endpoint SDK (ADR 0013): a from-scratch XCDR wire-core, no
// C shim, byte-identical to the Rust core and the other SDKs. Sync (poll) and
// async (an AsyncStream — the idiomatic Swift concurrency model).

import Foundation

public enum Endianness: Sendable { case little, big }

// --- Writer: a byte buffer + endian; alignment relative to buffer start, cap 4 ---

public struct Writer {
    public private(set) var buf: [UInt8] = []
    let endian: Endianness
    public init(_ endian: Endianness) { self.endian = endian }

    mutating func align(_ a: Int) {
        let cap = a > 4 ? 4 : a
        while buf.count % cap != 0 { buf.append(0) }
    }
    // le: the value already encoded little-endian; reversed for big.
    mutating func putLE(_ a: Int, _ le: [UInt8]) {
        align(a)
        buf.append(contentsOf: endian == .big ? le.reversed() : le)
    }
    public mutating func putU8(_ v: UInt8) { buf.append(v) }
    public mutating func putBool(_ v: Bool) { buf.append(v ? 1 : 0) }
    public mutating func putU16(_ v: UInt16) {
        putLE(2, [UInt8(v & 0xff), UInt8((v >> 8) & 0xff)])
    }
    public mutating func putU32(_ v: UInt32) {
        putLE(4, [UInt8(v & 0xff), UInt8((v >> 8) & 0xff), UInt8((v >> 16) & 0xff), UInt8((v >> 24) & 0xff)])
    }
    public mutating func putU64(_ v: UInt64) {
        var le = [UInt8]()
        for i in 0..<8 { le.append(UInt8((v >> (8 * UInt64(i))) & 0xff)) }
        putLE(4, le)
    }
    public mutating func putF32(_ v: Float) { putU32(v.bitPattern) }
    public mutating func putF64(_ v: Double) { putU64(v.bitPattern) }
    public mutating func putBytes(_ b: [UInt8]) { buf.append(contentsOf: b) }
    public mutating func putString(_ s: String) {
        let bytes = Array(s.utf8)
        putU32(UInt32(bytes.count + 1))
        putBytes(bytes)
        putU8(0)
    }
    public mutating func putSeqU8(_ b: [UInt8]) {
        putU32(UInt32(b.count))
        putBytes(b)
    }
    public func bytes() -> [UInt8] { buf }
}

// --- Reader: a byte cursor ---

public struct Reader {
    let data: [UInt8]
    var pos: Int = 0
    let endian: Endianness
    public init(_ data: [UInt8], _ endian: Endianness) { self.data = data; self.endian = endian }

    mutating func getLE(_ a: Int, _ n: Int) -> [UInt8] {
        let cap = a > 4 ? 4 : a
        while pos % cap != 0 { pos += 1 }
        var out = Array(data[pos..<pos + n])
        pos += n
        if endian == .big { out.reverse() }
        return out
    }
    public mutating func getU32() -> UInt32 {
        let b = getLE(4, 4)
        return UInt32(b[0]) | (UInt32(b[1]) << 8) | (UInt32(b[2]) << 16) | (UInt32(b[3]) << 24)
    }

    // --- primitives beyond getU32 — the byte-exact inverse of the Writer ---

    public mutating func getU8() -> UInt8 {
        let v = data[pos]
        pos += 1
        return v
    }
    public mutating func getU16() -> UInt16 {
        let b = getLE(2, 2)
        return UInt16(b[0]) | (UInt16(b[1]) << 8)
    }
    public mutating func getU64() -> UInt64 {
        let b = getLE(4, 8)
        var v: UInt64 = 0
        for i in 0..<8 { v |= UInt64(b[i]) << (8 * UInt64(i)) }
        return v
    }
    public mutating func getF32() -> Float {
        Float(bitPattern: getU32())
    }
    public mutating func getString() -> String {
        let n = Int(getU32())
        let s = String(decoding: data[pos..<pos + n - 1], as: UTF8.self)
        pos += n
        return s
    }
    public mutating func getSeqU8() -> [UInt8] {
        let n = Int(getU32())
        let b = Array(data[pos..<pos + n])
        pos += n
        return b
    }
}

// --- XRCE WRITE_DATA framing ---

public let xrceSessionNoKey: UInt8 = 0x80
public let xrceStreamBestEffort: UInt8 = 0x01

// The 16-bit submessage_length field cannot describe a sample larger than
// 0xFFFF. Rather than silently truncate the length while appending the full
// payload (which the C SDK rejects), return an empty frame so the caller sees
// the refusal.
public func xrceWriteFrame(session: UInt8, stream: UInt8, seq: UInt16, sample: [UInt8]) -> [UInt8] {
    guard sample.count <= 0xFFFF else { return [] }
    var out: [UInt8] = [
        session, stream,
        UInt8(seq & 0xff), UInt8((seq >> 8) & 0xff),
        0x07, 0x03,
        UInt8(sample.count & 0xff), UInt8((sample.count >> 8) & 0xff),
    ]
    out.append(contentsOf: sample)
    return out
}

// Frame layout: [session, stream, seq_lo, seq_hi, sm_id, flags, len_lo, len_hi]
// then the sample body. The 16-bit little-endian submessage_length (bytes 6..7)
// bounds the body exactly: the returned slice is frame[8 ..< 8+len], never
// frame[8...] to the end, so trailing padding or an appended submessage is not
// folded into the sample.
//
// Accepts WRITE_DATA (0x07, endpoint->hub / loopback) and DATA (0x09,
// hub->endpoint) — the hub pushes samples with DATA. See DDS-XRCE spec 8.3.5.
// Returns nil (never a crash) for a short header, a wrong submessage id, or a
// declared length that runs past the datagram (truncation / wrong length).
public func xrceReadFrame(_ frame: [UInt8]) -> [UInt8]? {
    guard frame.count >= 8, frame[4] == 0x07 || frame[4] == 0x09 else { return nil }
    let smLen = Int(frame[6]) | (Int(frame[7]) << 8)
    guard 8 + smLen <= frame.count else { return nil }
    return Array(frame[8 ..< 8 + smLen])
}

// --- transport ---

public protocol Transport: Sendable {
    func deliver(_ frame: [UInt8])
    func receive() -> [UInt8]?
}

// --- sync client ---

public final class Client {
    let transport: Transport
    let session: UInt8 = xrceSessionNoKey
    let stream: UInt8 = xrceStreamBestEffort
    var seq: UInt16 = 1
    public init(_ transport: Transport) { self.transport = transport }

    public func write(_ sample: [UInt8]) {
        let frame = xrceWriteFrame(session: session, stream: stream, seq: seq, sample: sample)
        seq = seq &+ 1
        transport.deliver(frame)
    }
    // One non-blocking receive: returns the sample body, or nil.
    public func poll() -> [UInt8]? {
        guard let frame = transport.receive() else { return nil }
        return xrceReadFrame(frame)
    }
}

// --- async reader: an AsyncStream (idiomatic Swift concurrency) ---

public final class AsyncReader: Sendable {
    let transport: Transport
    public init(_ transport: Transport) { self.transport = transport }

    public func stream() -> AsyncStream<[UInt8]> {
        let transport = self.transport
        return AsyncStream { continuation in
            let task = Task {
                while !Task.isCancelled {
                    if let frame = transport.receive() {
                        if let body = xrceReadFrame(frame) { continuation.yield(body) }
                    } else {
                        try? await Task.sleep(nanoseconds: 1_000_000)
                    }
                }
                continuation.finish()
            }
            continuation.onTermination = { _ in task.cancel() }
        }
    }
}

// In-memory FIFO transport for tests + example.
public final class MemTransport: Transport, @unchecked Sendable {
    private var q: [[UInt8]] = []
    private let lock = NSLock()
    public init() {}
    public func deliver(_ frame: [UInt8]) {
        lock.lock(); q.append(frame); lock.unlock()
    }
    public func receive() -> [UInt8]? {
        lock.lock(); defer { lock.unlock() }
        return q.isEmpty ? nil : q.removeFirst()
    }
}
