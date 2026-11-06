<!-- SPDX-License-Identifier: Apache-2.0 -->
# ZeroDDS — Swift (native endpoint)

A native **pure-Swift** endpoint SDK (ADR 0013) — a from-scratch XCDR wire-core,
no C shim, byte-identical to the Rust core and the other SDKs. **Sync** (`poll`)
and **async** — an `AsyncStream`, the idiomatic Swift concurrency model.

Sources: [`endpoints/swift`](../../endpoints/swift) (`Sources/Zerodds/Zerodds.swift`) ·
example: [`endpoints/swift/Sources/ZeroddsExample/main.swift`](../../endpoints/swift/Sources/ZeroddsExample/main.swift)
(`swift run ZeroddsExample`).

## Sync

```swift
let c = Client(transport)              // transport: Transport (deliver / receive)
c.write(sample)                        // frame as XRCE WRITE_DATA + deliver
if let body = c.poll() { ... }         // one non-blocking receive, or nil
```

## Async (AsyncStream)

```swift
let reader = AsyncReader(transport)
for await body in reader.stream() {    // decoded bodies as they arrive
    var r = Reader(body, .little)
    let id = r.getU32()
}
```

`stream()` returns an `AsyncStream<[UInt8]>` fed by a `Task` that polls the
transport (`Task.sleep` between empty reads); cancelling the consumer cancels
the task via `onTermination`.

## Wire-core

`Writer`/`Reader` cover the XCDR primitives with alignment relative to the
buffer start (cap 4). `f32`/`f64` go through `Float.bitPattern`/`Double.bitPattern`
— byte-identical to the Rust core.

## Tests (CI job `integration-bindings-swift`)

- byte-identity: the `@final` sample LE + BE, byte-identical to the Rust goldens
- sync loopback + async loopback (`swift test`)
- the three runnable examples (`ZeroddsExample`, `ZeroddsExampleSync`,
  `ZeroddsExampleAsync`), started as the programs `swift build` built

The weekly job installs Swift 6.0.3 into the Rust CI image
(`tests/integration/provision.sh`). The canonical goldens are committed under
`endpoints/swift/testdata/`. Verified locally on macOS (Swift 6.3).
