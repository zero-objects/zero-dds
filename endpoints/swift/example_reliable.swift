// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Runnable reliable-stream demo (in-process, no sockets): an aggregator
// submits N samples on a lossy channel (every 3rd first-delivery dropped);
// the receiver reorders + ACKNACKs, the sender retransmits, and the reader
// prints the recovered contiguous sequence. Mirrors
// `endpoints/zig/example_reliable.zig`.
//
// Run: swiftc -swift-version 5 -emit-module -emit-module-path /tmp/ZeroddsReliable.swiftmodule \
//        -emit-object -o /tmp/ZeroddsReliable.o Reliable.swift
//      swiftc -swift-version 5 -I /tmp example_reliable.swift /tmp/ZeroddsReliable.o -o /tmp/example_reliable
//      /tmp/example_reliable

import Foundation
import ZeroddsReliable

func u32le(_ v: UInt32) -> [UInt8] {
    [UInt8(v & 0xff), UInt8((v >> 8) & 0xff), UInt8((v >> 16) & 0xff), UInt8((v >> 24) & 0xff)]
}

func u32Decode(_ b: [UInt8]) -> UInt32 {
    UInt32(b[0]) | (UInt32(b[1]) << 8) | (UInt32(b[2]) << 16) | (UInt32(b[3]) << 24)
}

let n: UInt32 = 12
let sender = ReliableSender()
let receiver = ReliableReceiver()
var out: [[UInt8]] = []

// Aggregator submits N typed samples (payload = the sample index, u32 LE).
var submittedSeqs: [UInt16] = []
for i in 0..<n {
    let seq = try! sender.submit(u32le(i))
    submittedSeqs.append(seq)
}

// Lossy first delivery: drop every 3rd sample once.
var losses = 0
for (j, seq) in submittedSeqs.enumerated() {
    if (j + 1) % 3 == 0 {
        losses += 1
        continue // dropped in flight
    }
    try! receiver.recvData(seq: seq, payload: sender.getInFlight(seq)!)
}
out.append(contentsOf: receiver.drainInOrder().map { $0.payload })

// Recovery: ACKNACK -> prune + retransmit until the window drains.
var round = 0
while sender.inFlightCount > 0 && round < 100 {
    let an = receiver.pendingAckNack(hintLastSeen: nil)
    sender.recvAckNack(an)
    for seq in sender.inFlightSeqs() {
        if let data = sender.getInFlight(seq) {
            try! receiver.recvData(seq: seq, payload: data)
        }
    }
    out.append(contentsOf: receiver.drainInOrder().map { $0.payload })
    round += 1
}

print("reliable: delivered \(out.count)/\(n) gap-free (recovered \(losses) losses in \(round) rounds)")

for (i, payload) in out.enumerated() {
    let v = u32Decode(payload)
    if v != UInt32(i) {
        print("GAP: slot \(i) = \(v)")
        exit(0)
    }
}
print("sequence 0..\(n - 1) verified in order")
