// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Standalone unit-test runner for `Reliable.swift`, mirroring
// `crates/xrce/src/reliable.rs`'s `#[cfg(test)] mod tests`. Not `XCTest`
// (this file is compiled + run via plain `swiftc`/`swift`, matching every
// other endpoint's reliable-stream test harness — no SwiftPM test bundle
// required). Prints `UNIT OK` and exits 0 on success; on any failure it
// prints `FAIL: <name> — <reason>` and exits 1.

import Foundation
import ZeroddsReliable

var failures: [String] = []

func check(_ name: String, _ cond: Bool, _ reason: String = "") {
    if !cond { failures.append(reason.isEmpty ? name : "\(name) — \(reason)") }
}

func run(_ name: String, _ body: () throws -> Void) {
    do {
        try body()
    } catch {
        failures.append("\(name) — threw \(error)")
    }
}

run("submit_assigns_monotonic_seqnrs") {
    let s = ReliableSender()
    let s0 = try s.submit([1, 2])
    let s1 = try s.submit([3, 4])
    check("submit_assigns_monotonic_seqnrs", s0 == 0 && s1 == 1 && s.inFlightCount == 2)
}

run("submit_rejects_payload_too_large") {
    let s = ReliableSender()
    let huge = [UInt8](repeating: 0, count: reliableMaxPayload + 1)
    do {
        _ = try s.submit(huge)
        check("submit_rejects_payload_too_large", false, "did not throw")
    } catch ReliableError.payloadTooLarge {
        check("submit_rejects_payload_too_large", true)
    }
}

run("submit_rejects_when_window_full") {
    let s = ReliableSender()
    for _ in 0..<reliableSenderWindow { _ = try s.submit([0]) }
    do {
        _ = try s.submit([0])
        check("submit_rejects_when_window_full", false, "did not throw")
    } catch ReliableError.windowFull {
        check("submit_rejects_when_window_full", true)
    }
}

run("pending_heartbeat_fires_first_time") {
    let s = ReliableSender()
    _ = try s.submit([1])
    guard let hb = s.pendingHeartbeat(nowMs: 0) else {
        check("pending_heartbeat_fires_first_time", false, "no heartbeat")
        return
    }
    check(
        "pending_heartbeat_fires_first_time",
        hb.firstUnacked == 0 && hb.lastUnacked == 0 && hb.streamId == reliableStreamId
    )
}

run("pending_heartbeat_silenced_until_period_elapsed") {
    let s = ReliableSender()
    _ = try s.submit([1])
    check("pending_heartbeat_silenced_until_period_elapsed/t0", s.pendingHeartbeat(nowMs: 0) != nil)
    check("pending_heartbeat_silenced_until_period_elapsed/t100", s.pendingHeartbeat(nowMs: 100) == nil)
    check("pending_heartbeat_silenced_until_period_elapsed/t600", s.pendingHeartbeat(nowMs: 600) != nil)
}

run("pending_heartbeat_none_when_window_empty") {
    let s = ReliableSender()
    check("pending_heartbeat_none_when_window_empty", s.pendingHeartbeat(nowMs: 0) == nil)
}

run("recv_acknack_clears_acked_seqnrs") {
    let s = ReliableSender()
    _ = try s.submit([0xA0])
    _ = try s.submit([0xA1])
    _ = try s.submit([0xA2])
    check("recv_acknack_clears_acked_seqnrs/pre", s.inFlightCount == 3)
    s.recvAckNack(AckNack(firstUnacked: 2, nackBitmap: 0x0001, streamId: 0x80))
    check(
        "recv_acknack_clears_acked_seqnrs/post",
        s.inFlightCount == 1 && s.getInFlight(2) != nil
    )
}

run("recv_acknack_full_clear_when_no_bits_set") {
    let s = ReliableSender()
    for _ in 0..<5 { _ = try s.submit([0]) }
    s.recvAckNack(AckNack(firstUnacked: 5, nackBitmap: 0, streamId: 0x80))
    check("recv_acknack_full_clear_when_no_bits_set", s.inFlightCount == 0)
}

run("recv_data_buffers_in_order") {
    let r = ReliableReceiver()
    try r.recvData(seq: 0, payload: [10])
    try r.recvData(seq: 1, payload: [11])
    let drained = r.drainInOrder()
    check(
        "recv_data_buffers_in_order",
        drained.count == 2 && drained[0].seq == 0 && drained[1].seq == 1 && r.expectedSeq == 2
    )
}

run("recv_data_reorders_out_of_order") {
    let r = ReliableReceiver()
    try r.recvData(seq: 2, payload: [22])
    try r.recvData(seq: 0, payload: [20])
    let d1 = r.drainInOrder()
    check("recv_data_reorders_out_of_order/d1", d1.count == 1 && d1[0].seq == 0)
    try r.recvData(seq: 1, payload: [21])
    let d2 = r.drainInOrder()
    check(
        "recv_data_reorders_out_of_order/d2",
        d2.count == 2 && d2[0].seq == 1 && d2[1].seq == 2
    )
}

run("recv_data_drops_duplicates") {
    let r = ReliableReceiver()
    try r.recvData(seq: 0, payload: [1])
    _ = r.drainInOrder()
    try r.recvData(seq: 0, payload: [99]) // already delivered -> dropped
    check("recv_data_drops_duplicates", r.outOfOrderCount == 0)
}

run("recv_data_rejects_when_buffer_full") {
    let r = ReliableReceiver()
    for i in 1...UInt16(reliableReceiverBuffer) { try r.recvData(seq: i, payload: [1]) }
    do {
        try r.recvData(seq: UInt16(reliableReceiverBuffer) + 1, payload: [1])
        check("recv_data_rejects_when_buffer_full", false, "did not throw")
    } catch ReliableError.bufferFull {
        check("recv_data_rejects_when_buffer_full", true)
    }
}

run("pending_acknack_marks_missing_slots") {
    let r = ReliableReceiver()
    try r.recvData(seq: 1, payload: [1])
    try r.recvData(seq: 3, payload: [3])
    let an = r.pendingAckNack(hintLastSeen: 3)
    check(
        "pending_acknack_marks_missing_slots",
        (an.nackBitmap & (1 << 0)) != 0 && // seq 0 missing
            (an.nackBitmap & (1 << 2)) != 0 && // seq 2 missing
            (an.nackBitmap & (1 << 1)) == 0 && // seq 1 present
            (an.nackBitmap & (1 << 3)) == 0 // seq 3 present
    )
}

run("reset_clears_state_completely") {
    let r = ReliableReceiver()
    try r.recvData(seq: 0, payload: [3])
    r.reset()
    check("reset_clears_state_completely", r.outOfOrderCount == 0 && r.expectedSeq == 0)
}

run("byte_golden_heartbeat_acknack") {
    // Byte-identical to golden_heartbeat_le.bin / golden_acknack_le.bin
    // (`zerodds-endpoint-golden`, encode_heartbeat_frame / encode_acknack_frame).
    let hb = heartbeatFrame(Heartbeat(firstUnacked: 1, lastUnacked: 3, streamId: 0x80))
    let wantHb: [UInt8] = [0x80, 0x00, 0x01, 0x00, 0x0B, 0x01, 0x05, 0x00, 0x01, 0x00, 0x03, 0x00, 0x80]
    check("byte_golden_heartbeat", hb == wantHb, "got \(hb)")

    let an = acknackFrame(AckNack(firstUnacked: 1, nackBitmap: 0, streamId: 0x80))
    let wantAn: [UInt8] = [0x80, 0x00, 0x01, 0x00, 0x0A, 0x01, 0x05, 0x00, 0x01, 0x00, 0x00, 0x00, 0x80]
    check("byte_golden_acknack", an == wantAn, "got \(an)")
}

run("end_to_end_sender_receiver_with_loss_recovery") {
    // Spec §8.4.14 + §9.2 — sender submits 3, receiver sees only seq 0 + 2
    // (seq 1 lost); ACKNACK recovery delivers all 3 in order.
    let sender = ReliableSender()
    let receiver = ReliableReceiver()
    let s0 = try sender.submit([10])
    let s1 = try sender.submit([11])
    let s2 = try sender.submit([12])
    check("e2e/submit_count", sender.inFlightCount == 3)

    try receiver.recvData(seq: s0, payload: [10])
    try receiver.recvData(seq: s2, payload: [12])

    let drained = receiver.drainInOrder()
    check("e2e/first_drain", drained.count == 1 && drained[0].payload == [10])

    let acknack = receiver.pendingAckNack(hintLastSeen: s2)
    sender.recvAckNack(acknack)
    check("e2e/s1_still_inflight", sender.getInFlight(s1) != nil)

    guard let s1Payload = sender.getInFlight(s1) else {
        check("e2e/retransmit_lookup", false)
        return
    }
    try receiver.recvData(seq: s1, payload: s1Payload)

    let drained2 = receiver.drainInOrder()
    check(
        "e2e/second_drain",
        drained2.count == 2 && drained2[0].payload == [11] && drained2[1].payload == [12]
    )
}

run("heartbeat_and_recovery_across_wrap") {
    // RFC-1982 regression: HEARTBEAT window + loss recovery across the 16-bit
    // wrap (mirrors crates/xrce's wrap regression tests). Seeds sender/receiver
    // up to the wrap using only the public API (submit + full-ack / recvData +
    // drain), then straddles 0x0000.
    let s = ReliableSender()
    var seq: UInt16 = 0 // walk nextSeq to 0xFFFE: submit one, fully-ack it, repeat.
    repeat {
        seq = try s.submit([0])
        s.recvAckNack(AckNack(firstUnacked: seq &+ 1, nackBitmap: 0, streamId: 0x80))
    } while seq != 0xFFFD
    check("wrap/seed_drained", s.inFlightCount == 0)

    let q0 = try s.submit([10]) // 0xFFFE
    let q1 = try s.submit([11]) // 0xFFFF (lost)
    let q2 = try s.submit([12]) // 0x0000
    let q3 = try s.submit([13]) // 0x0001
    check("wrap/seqs", q0 == 0xFFFE && q1 == 0xFFFF && q2 == 0x0000 && q3 == 0x0001)

    let hb = s.pendingHeartbeat(nowMs: 0)
    check(
        "wrap/heartbeat_window",
        hb?.firstUnacked == 0xFFFE && hb?.lastUnacked == 0x0001,
        "want [0xFFFE,0x0001], got \(String(describing: hb))"
    )

    let r = ReliableReceiver() // seed expected to 0xFFFE
    for k in 0...0xFFFD {
        try r.recvData(seq: UInt16(k), payload: [0])
        _ = r.drainInOrder()
    }
    check("wrap/receiver_seed", r.expectedSeq == 0xFFFE)

    try r.recvData(seq: q0, payload: [10]) // 0xFFFF lost
    try r.recvData(seq: q2, payload: [12])
    try r.recvData(seq: q3, payload: [13])
    let dw = r.drainInOrder()
    check("wrap/first_drain", dw.count == 1 && dw[0].payload == [10])
    check("wrap/blocked_at_ffff", r.expectedSeq == 0xFFFF)

    let ack = r.pendingAckNack(hintLastSeen: q3)
    check("wrap/ack_base", ack.firstUnacked == 0xFFFF)
    check("wrap/ack_bits", (ack.nackBitmap & 0b1) != 0 && (ack.nackBitmap & 0b110) == 0)

    s.recvAckNack(ack)
    check("wrap/retransmittable", s.getInFlight(q1) != nil)
    check("wrap/acked", s.getInFlight(q0) == nil && s.getInFlight(q2) == nil && s.getInFlight(q3) == nil)
    check("wrap/inflight_one", s.inFlightCount == 1)

    try r.recvData(seq: q1, payload: s.getInFlight(q1)!)
    let dw2 = r.drainInOrder()
    check(
        "wrap/second_drain",
        dw2.count == 3 && dw2[0].payload == [11] && dw2[1].payload == [12] && dw2[2].payload == [13]
    )
}

// AsyncWriter window-overflow: submit MORE than the 16-sample window before any
// ACKNACK arrives, then let ACKNACKs flow. The pre-fix drain popped a sample off
// the queue *before* submit, so on a full window that sample was gone yet never
// sent — silent data loss. This test keeps the window full while >16 samples are
// pending, then verifies EVERY sample is delivered (none lost) and the writer
// terminates (no hang).
final class LoopPeer {
    let lock = NSLock()
    let rcv = ReliableReceiver()
    var delivered: [UInt8] = []
    var acks: [AckNack] = []
    var writeDataSeen = 0 // distinct fresh WRITE_DATA the peer accepted
    var gateOpen = false

    // Runs on the writer's drain thread (send side). Feeds the receiver, records
    // in-order deliveries, and queues a cumulative ACKNACK. Retransmits of
    // already-delivered seqs are ignored (they would otherwise loop forever).
    func onSend(_ frame: [UInt8]) {
        guard let wd = parseWriteData(frame) else { return } // ignore HEARTBEAT etc.
        lock.lock(); defer { lock.unlock() }
        if seqLt(wd.seq, rcv.expectedSeq) { return } // retransmit of delivered → ignore
        do { try rcv.recvData(seq: wd.seq, payload: wd.sample) } catch { return }
        for item in rcv.drainInOrder() { delivered.append(item.payload.first ?? 0) }
        acks.append(rcv.pendingAckNack(hintLastSeen: wd.seq))
        writeDataSeen += 1
    }

    // Runs on the writer's drain thread (recv side). Releases queued ACKNACKs
    // only once the gate is open, so the window genuinely fills first.
    func onRecv() -> [UInt8]? {
        lock.lock(); defer { lock.unlock() }
        guard gateOpen, !acks.isEmpty else { return nil }
        return acknackFrame(acks.removeFirst())
    }
}

run("async_writer_window_overflow_no_loss") {
    let n = 20 // > reliableSenderWindow (16)
    let peer = LoopPeer()
    let w = AsyncWriter(sendFn: { peer.onSend($0) }, recvFn: { peer.onRecv() })
    w.start()

    for i in 0..<n { _ = w.write([UInt8(i)]) }

    // Wait until the 16-sample window is full (16 distinct WRITE_DATA sent) while
    // the gate stays closed, then give the drain thread a moment to try the
    // remaining 4 — the pre-fix code loses exactly those here.
    var waited = 0
    while true {
        peer.lock.lock(); let seen = peer.writeDataSeen; peer.lock.unlock()
        if seen >= reliableSenderWindow || waited > 5000 { break }
        usleep(1000); waited += 1
    }
    usleep(50_000)

    peer.lock.lock(); peer.gateOpen = true; peer.lock.unlock()

    // Wait for all n to be delivered, with a hard bound so a hang fails the test.
    waited = 0
    while true {
        peer.lock.lock(); let d = peer.delivered.count; peer.lock.unlock()
        if d >= n || waited > 10000 { break }
        usleep(1000); waited += 1
    }
    w.close()

    peer.lock.lock(); let got = peer.delivered; peer.lock.unlock()
    check("async_overflow/no_loss", got.count == n, "delivered \(got.count) of \(n): \(got)")
    check("async_overflow/all_present", got.sorted() == Array(0..<UInt8(n)), "got \(got.sorted())")
}

if failures.isEmpty {
    print("UNIT OK")
    exit(0)
} else {
    for f in failures { FileHandle.standardError.write(("FAIL: " + f + "\n").data(using: .utf8)!) }
    exit(1)
}
