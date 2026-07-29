// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Native pure-Swift reliable XRCE stream layer (DDS-XRCE 1.0 §8.4.10/§8.4.11),
// byte-identical to `crates/xrce` and the other endpoint SDKs (`endpoints/zig`,
// `endpoints/c`, …). Self-contained (only `Foundation`, for `Date`/`Thread`/
// `NSLock`/`DispatchSemaphore`) so it compiles as its own module — the same
// recipe the ping-pong test uses for `Zerodds.swift` — and never collides with
// a generated IDL module's own wire-core types (`Endianness`/`Writer`/`Reader`).
//
// Two roles:
//   ReliableSender   — submit / pendingHeartbeat / recvAckNack / getInFlight
//                       (history + retransmit), plus `AsyncWriter`: a
//                       lock-guarded producer queue drained by a dedicated
//                       `Thread` that owns the `ReliableSender` state
//                       exclusively and performs every syscall — the
//                       producer's `write` never touches the network.
//   ReliableReceiver — recvData / drainInOrder / pendingAckNack (reorder +
//                       de-duplication).
//
// RFC-1982 16-bit sequence numbers; sender window 16, receiver buffer 64,
// heartbeat 500 ms, payload <= 65535.

import Foundation

public let reliableSenderWindow = 16
public let reliableReceiverBuffer = 64
public let reliableMaxPayload = 65_535
public let reliableHeartbeatPeriodMs: Int64 = 500
public let reliableStreamId: UInt8 = 0x80

private let halfWindow: UInt16 = 0x8000

/// RFC-1982: is `a` strictly before `b`?
public func seqLt(_ a: UInt16, _ b: UInt16) -> Bool {
    let d = b &- a
    return d != 0 && d < halfWindow
}

/// RFC-1982: is `a` strictly after `b`?
public func seqGt(_ a: UInt16, _ b: UInt16) -> Bool { seqLt(b, a) }

public enum ReliableError: Error, Equatable {
    case payloadTooLarge
    case windowFull
    case bufferFull
}

public struct Heartbeat: Equatable {
    public var firstUnacked: UInt16
    public var lastUnacked: UInt16
    public var streamId: UInt8
    public init(firstUnacked: UInt16, lastUnacked: UInt16, streamId: UInt8) {
        self.firstUnacked = firstUnacked
        self.lastUnacked = lastUnacked
        self.streamId = streamId
    }
}

public struct AckNack: Equatable {
    public var firstUnacked: UInt16
    public var nackBitmap: UInt16
    public var streamId: UInt8
    public init(firstUnacked: UInt16, nackBitmap: UInt16, streamId: UInt8) {
        self.firstUnacked = firstUnacked
        self.nackBitmap = nackBitmap
        self.streamId = streamId
    }
}

// ------------------------------------------------------------------
// Wire framing — 8-byte header + body, all little-endian:
//   [0]=session [1]=stream [2..4]=seq LE [4]=submsg id [5]=flags
//   [6..8]=body-len LE [8..]=body
// WRITE_DATA keeps the sample's RFC-1982 seq in the header seq field
// ([2..4]) with stream byte 0x80 (reliable), matching the Rust peer's
// `reliable_unframe`. HEARTBEAT/ACKNACK use the byte-golden layout (session
// 0x80, stream NONE 0x00, msg-seq=1) — `golden_heartbeat_le.bin` /
// `golden_acknack_le.bin`; every peer's parser ignores the header stream/seq
// bytes and only checks the submessage id at offset 4, so this is wire-safe.
// ------------------------------------------------------------------

/// Reliable WRITE_DATA frame; the 2-byte stream-header seq carries the sample seq.
public func writeDataFrame(seq: UInt16, sample: [UInt8]) -> [UInt8] {
    let n = UInt16(sample.count)
    var out: [UInt8] = [
        0x80, reliableStreamId,
        UInt8(seq & 0xff), UInt8(seq >> 8),
        0x07, 0x03,
        UInt8(n & 0xff), UInt8(n >> 8),
    ]
    out.append(contentsOf: sample)
    return out
}

/// Parses a reliable WRITE_DATA frame into `(seq, sample)`.
public func parseWriteData(_ frame: [UInt8]) -> (seq: UInt16, sample: [UInt8])? {
    guard frame.count >= 8, frame[4] == 0x07 else { return nil }
    let seq = UInt16(frame[2]) | (UInt16(frame[3]) << 8)
    return (seq, Array(frame[8...]))
}

/// HEARTBEAT frame (golden layout, 13 bytes).
public func heartbeatFrame(_ hb: Heartbeat) -> [UInt8] {
    [
        0x80, 0x00, 0x01, 0x00,
        0x0B, 0x01, 0x05, 0x00,
        UInt8(hb.firstUnacked & 0xff), UInt8(hb.firstUnacked >> 8),
        UInt8(hb.lastUnacked & 0xff), UInt8(hb.lastUnacked >> 8),
        hb.streamId,
    ]
}

/// Parses a HEARTBEAT frame. Ignores header stream/seq — matches every peer.
public func parseHeartbeat(_ frame: [UInt8]) -> Heartbeat? {
    guard frame.count >= 13, frame[4] == 0x0B else { return nil }
    return Heartbeat(
        firstUnacked: UInt16(frame[8]) | (UInt16(frame[9]) << 8),
        lastUnacked: UInt16(frame[10]) | (UInt16(frame[11]) << 8),
        streamId: frame[12]
    )
}

/// ACKNACK frame (golden layout, 13 bytes).
public func acknackFrame(_ an: AckNack) -> [UInt8] {
    [
        0x80, 0x00, 0x01, 0x00,
        0x0A, 0x01, 0x05, 0x00,
        UInt8(an.firstUnacked & 0xff), UInt8(an.firstUnacked >> 8),
        UInt8(an.nackBitmap & 0xff), UInt8(an.nackBitmap >> 8),
        an.streamId,
    ]
}

/// Parses an ACKNACK frame. Ignores header stream/seq — matches every peer.
public func parseAckNack(_ frame: [UInt8]) -> AckNack? {
    guard frame.count >= 13, frame[4] == 0x0A else { return nil }
    return AckNack(
        firstUnacked: UInt16(frame[8]) | (UInt16(frame[9]) << 8),
        nackBitmap: UInt16(frame[10]) | (UInt16(frame[11]) << 8),
        streamId: frame[12]
    )
}

// ------------------------------------------------------------------
// Sender
// ------------------------------------------------------------------

/// Sender-side state machine: buffers in-flight samples (history for
/// retransmit), assigns monotonic RFC-1982 seqnrs, tracks heartbeat cadence,
/// prunes on ACKNACK. Not thread-safe by itself — `AsyncWriter` below owns one
/// instance exclusively on its drain thread.
public final class ReliableSender {
    private var nextSeq: UInt16 = 0
    private var inFlight: [UInt16: [UInt8]] = [:]
    private var lastHeartbeatMs: Int64 = -1

    public init() {}

    public var inFlightCount: Int { inFlight.count }

    public func getInFlight(_ seq: UInt16) -> [UInt8]? { inFlight[seq] }

    /// In-flight seqnrs, ascending — a deterministic retransmit order.
    public func inFlightSeqs() -> [UInt16] { inFlight.keys.sorted() }

    /// Buffers a new sample, returns its assigned seqnr.
    public func submit(_ payload: [UInt8]) throws -> UInt16 {
        if payload.count > reliableMaxPayload { throw ReliableError.payloadTooLarge }
        if inFlight.count >= reliableSenderWindow { throw ReliableError.windowFull }
        let seq = nextSeq
        inFlight[seq] = payload
        nextSeq = nextSeq &+ 1
        return seq
    }

    /// Emits a HEARTBEAT if the window is non-empty and the period elapsed.
    /// `nowMs` is a monotonic millisecond clock supplied by the caller.
    public func pendingHeartbeat(nowMs: Int64) -> Heartbeat? {
        guard !inFlight.isEmpty else { return nil }
        let due = lastHeartbeatMs < 0 || (nowMs - lastHeartbeatMs) >= reliableHeartbeatPeriodMs
        guard due else { return nil }
        lastHeartbeatMs = nowMs
        let keys = inFlight.keys
        guard let first = keys.min(), let last = keys.max() else { return nil }
        return Heartbeat(firstUnacked: first, lastUnacked: last, streamId: reliableStreamId)
    }

    /// Processes an ACKNACK: everything strictly before `base` is acked
    /// (RFC-1982); within `[base, base+16)` a clear bit is acked (dropped), a
    /// set bit stays in-flight (retransmit candidate).
    public func recvAckNack(_ an: AckNack) {
        let base = an.firstUnacked
        let acked = inFlight.keys.filter { seqLt($0, base) }
        for seq in acked { inFlight.removeValue(forKey: seq) }
        for i: UInt16 in 0..<16 {
            let seq = base &+ i
            let bit = (an.nackBitmap >> i) & 1
            if bit == 0 { inFlight.removeValue(forKey: seq) }
        }
    }
}

// ------------------------------------------------------------------
// Receiver
// ------------------------------------------------------------------

/// Receiver-side state machine: reorders + de-duplicates incoming samples,
/// delivers gap-free runs starting at `expectedSeq`, computes the
/// missing-slot ACKNACK bitmap. Not thread-safe — one instance per stream.
public final class ReliableReceiver {
    private var expectedSeqStore: UInt16 = 0
    private var received: [UInt16: [UInt8]] = [:]

    public init() {}

    public var expectedSeq: UInt16 { expectedSeqStore }
    public var outOfOrderCount: Int { received.count }

    /// Buffers an incoming sample (dedup + DoS bound).
    public func recvData(seq: UInt16, payload: [UInt8]) throws {
        if seqLt(seq, expectedSeqStore) { return } // duplicate before window -> drop
        if received[seq] != nil { return } // already buffered
        if received.count >= reliableReceiverBuffer { throw ReliableError.bufferFull }
        received[seq] = payload
    }

    /// Delivers all contiguous samples from `expectedSeq`, advances it.
    public func drainInOrder() -> [(seq: UInt16, payload: [UInt8])] {
        var out: [(seq: UInt16, payload: [UInt8])] = []
        while let payload = received.removeValue(forKey: expectedSeqStore) {
            out.append((expectedSeqStore, payload))
            expectedSeqStore = expectedSeqStore &+ 1
        }
        return out
    }

    /// Computes the ACKNACK marking missing slots in `[expected, expected+16)`.
    /// `hintLastSeen` (typically a HEARTBEAT's `lastUnacked`) bounds the
    /// window so slots beyond it are not falsely marked missing.
    public func pendingAckNack(hintLastSeen: UInt16?) -> AckNack {
        let base = expectedSeqStore
        var bitmap: UInt16 = 0
        for i: UInt16 in 0..<16 {
            let seq = base &+ i
            if let h = hintLastSeen, seqGt(seq, h) { continue }
            if received[seq] == nil { bitmap |= (1 << i) }
        }
        return AckNack(firstUnacked: base, nackBitmap: bitmap, streamId: reliableStreamId)
    }

    public func reset() {
        expectedSeqStore = 0
        received.removeAll()
    }
}

// ------------------------------------------------------------------
// Async-decoupled writer: the producer's `write` only appends to a
// lock-guarded bounded queue and returns (back-pressure via a `false`
// result, never a block); a dedicated drain `Thread` owns the
// `ReliableSender` state exclusively and performs every syscall (submit ->
// history -> WRITE_DATA send; periodic HEARTBEAT; on ACKNACK retransmit +
// prune). The producer never enters the kernel.
// ------------------------------------------------------------------

public final class AsyncWriter {
    public static let queueCapacity = 1024

    private let sender = ReliableSender()
    private var queue: [[UInt8]] = []
    private let lock = NSLock()
    private let stopped = DispatchSemaphore(value: 0)
    private var thread: Thread?
    private let sendFn: ([UInt8]) -> Void
    private let recvFn: () -> [UInt8]?
    private let clock = Date()

    /// `sendFn` transmits one frame (e.g. `sendto` on a UDP socket); `recvFn`
    /// returns the next available received frame (e.g. non-blocking
    /// `recvfrom`), or `nil` when nothing is pending.
    public init(sendFn: @escaping ([UInt8]) -> Void, recvFn: @escaping () -> [UInt8]?) {
        self.sendFn = sendFn
        self.recvFn = recvFn
    }

    /// Starts the drain thread. Call once before `write`.
    public func start() {
        let t = Thread { [weak self] in self?.drainLoop() }
        t.name = "zerodds-reliable-writer"
        thread = t
        t.start()
    }

    /// Enqueues a sample for reliable delivery. Returns `false`
    /// (backpressure) when the queue is full — never blocks, never touches
    /// the network.
    @discardableResult
    public func write(_ sample: [UInt8]) -> Bool {
        lock.lock()
        defer { lock.unlock() }
        guard queue.count < Self.queueCapacity else { return false }
        queue.append(sample)
        return true
    }

    private func pop() -> [UInt8]? {
        lock.lock()
        defer { lock.unlock() }
        return queue.isEmpty ? nil : queue.removeFirst()
    }

    private func drainLoop() {
        while !Thread.current.isCancelled {
            var idle = true
            if let sample = pop() {
                idle = false
                do {
                    let seq = try sender.submit(sample)
                    sendFn(writeDataFrame(seq: seq, sample: sample))
                } catch {
                    // Window full: drain pending ACKNACKs to free a slot, retry next tick.
                    _ = drainAckNacks()
                    continue
                }
            }
            if drainAckNacks() { idle = false }
            let nowMs = Int64(Date().timeIntervalSince(clock) * 1000)
            if let hb = sender.pendingHeartbeat(nowMs: nowMs) {
                sendFn(heartbeatFrame(hb))
            }
            if idle { usleep(200) }
        }
        stopped.signal()
    }

    @discardableResult
    private func drainAckNacks() -> Bool {
        var any = false
        while let frame = recvFn() {
            guard let an = parseAckNack(frame) else { break }
            sender.recvAckNack(an)
            for seq in sender.inFlightSeqs() {
                if let data = sender.getInFlight(seq) {
                    sendFn(writeDataFrame(seq: seq, sample: data))
                }
            }
            any = true
        }
        return any
    }

    /// Stops the drain thread and waits for it to exit.
    public func close() {
        thread?.cancel()
        stopped.wait()
        thread = nil
    }
}
