// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Runnable example for the native Swift endpoint: sync (poll) and async
// (AsyncStream). Run with `swift run ZeroddsExample`.

import Zerodds

func sample(_ id: UInt32, _ label: String) -> [UInt8] {
    var w = Writer(.little)
    w.putU32(id)
    w.putString(label)
    return w.bytes()
}

// --- sync ---
let t = MemTransport()
let c = Client(t)
c.write(sample(0x42, "sync-hello"))
if let body = c.poll() {
    var r = Reader(body, .little)
    print("sync: received id=0x" + String(r.getU32(), radix: 16))
}

// --- async (AsyncStream) ---
let t2 = MemTransport()
let w = Client(t2)
for i in 0..<3 { w.write(sample(0x100 + UInt32(i), "async")) }
let reader = AsyncReader(t2)
var n = 0
for await body in reader.stream() {
    var r = Reader(body, .little)
    print("async: received id=0x" + String(r.getU32(), radix: 16))
    n += 1
    if n == 3 { break }
}

print("ALL OK")
