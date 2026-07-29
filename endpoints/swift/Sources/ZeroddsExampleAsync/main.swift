// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Deep example (async): the same sensor-telemetry flow, but the subscriber does
// not own the run-loop. `AsyncReader.stream()` is an `AsyncStream`; the consumer
// iterates it with `for await` — the idiomatic Swift concurrency model. Every
// field is decoded.

import Foundation
import Zerodds

struct Reading {
    var id: UInt32
    var value: Float
    var label: String

    func marshal(_ endian: Endianness) -> [UInt8] {
        var w = Writer(endian)
        w.putU32(id)
        w.putF32(value)
        w.putString(label)
        return w.bytes()
    }

    static func decode(_ body: [UInt8]) -> Reading {
        var r = Reader(body, .little)
        let id = r.getU32()
        let value = r.getF32()
        let label = r.getString()
        return Reading(id: id, value: value, label: label)
    }
}

func run() async {
    let total = 5
    let t = MemTransport()
    let c = Client(t)
    for i in 0..<total {
        let r = Reading(id: 0x2000 + UInt32(i), value: 100.0 - Float(i),
                        label: String(format: "sensor-%02d", i))
        c.write(r.marshal(.little))
    }

    let reader = AsyncReader(t)
    var got = 0
    for await body in reader.stream() {
        let r = Reading.decode(body)
        print(String(format: "async reading %d: id=0x%x value=%.1f label=\"%@\"",
                     got, r.id, r.value, r.label))
        got += 1
        if got == total { break }
    }
    print("ALL OK")
}

await run()
