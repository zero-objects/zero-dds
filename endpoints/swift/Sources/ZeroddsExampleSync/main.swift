// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Deep example (sync): a realistic sensor-telemetry flow. A publisher frames
// five typed `Reading { id, value, label }` samples and delivers them; the
// subscriber owns the run-loop and polls, decoding EVERY field byte-for-byte.

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

let total = 5
let t = MemTransport()
let c = Client(t)
for i in 0..<total {
    let r = Reading(id: 0x1000 + UInt32(i), value: 20.0 + Float(i) * 0.5,
                    label: String(format: "bay-%02d", i))
    c.write(r.marshal(.little))
}

var got = 0
while got < total {
    guard let body = c.poll() else { break }
    let r = Reading.decode(body)
    print(String(format: "sync reading %d: id=0x%x value=%.1f label=\"%@\"",
                 got, r.id, r.value, r.label))
    got += 1
}

if got != total {
    FileHandle.standardError.write("incomplete: got \(got) of \(total)\n".data(using: .utf8)!)
    exit(1)
}
print("ALL OK")
