<!-- SPDX-License-Identifier: Apache-2.0 -->
# `zerodds-endpoint-swift` v1.0 — Swift Endpoint-SDK

**Status:** normative · ZeroDDS Vendor-Spec. Implementiert in `endpoints/swift/`.

Analog zu [`zerodds-xcdr2-swift`](zerodds-xcdr2-swift-1.0.md) (dort das
Marshalling) und den Endpoint-SDKs anderer Sprachen (`endpoints/go`,
`endpoints/zig`, `endpoints/nim`, `endpoints/c`, ...): das native
Swift-Endpoint über XRCE-Framing, sync `Client`, async `AsyncReader` (ein
`AsyncStream`, das idiomatische Swift-Concurrency-Modell) und den reliable
Stream, so dass eine Swift-App ein XCDR2-Sample byte-identisch zu
`crates/xrce` + `endpoints/c` mit dem geteilten Rust-Peer austauscht.

Kein C-Shim: `endpoints/swift/Sources/Zerodds/Zerodds.swift` ist ein
eigenständiger, reiner Swift-Wire-Core (nur `Foundation`, für `NSLock`), der
mit dem generierten IDL-Modul in einer App koexistiert, indem er als eigenes
`Zerodds`-Modul kompiliert wird (siehe §6).

## §1 XRCE-Framing

Ein 8-Byte-XRCE-Header (`session`, `stream`, `seq` LE, Submessage-ID `0x07`
WRITE_DATA, `flags`, `len` LE) gefolgt vom XCDR2-Sample-Body — byte-identisch
zu `crates/xrce` und `endpoints/c`.

`endpoints/swift` MUSS bereitstellen:

- `func xrceWriteFrame(session: UInt8, stream: UInt8, seq: UInt16, sample: [UInt8]) -> [UInt8]`
  — framet ein Sample.
- `func xrceReadFrame(_ frame: [UInt8]) -> [UInt8]?` — entrahmt; `nil` bei zu
  kurzem Frame oder falscher Submessage-ID.
- Konstanten `xrceSessionNoKey` (`0x80`, best-effort, ohne ClientKey) und
  `xrceStreamBestEffort` (`0x01`).

## §2 Sync `Client`

Ein blockierender Client — Gegenstück zum `AsyncStream`-basierten
`AsyncReader`: `write` framet + liefert synchron über den `Transport`;
`poll` ist ein nicht-blockierender Einzel-Receive.

`endpoints/swift` MUSS bereitstellen:

- `protocol Transport: Sendable { func deliver(_ frame: [UInt8]); func receive() -> [UInt8]? }`
  — der einzige Integrationspunkt; der Integrator implementiert ihn für
  seinen Link (z. B. UDP, siehe `UDPTransport` in `crates/endpoint-e2e/tests/swift.rs`).
- `final class Client` mit `init(_ transport: Transport)`,
  `func write(_ sample: [UInt8])`, `func poll() -> [UInt8]?`.
- Eine monoton wachsende Sequenznummer pro `write`; `session`/`stream` per
  Default `xrceSessionNoKey`/`xrceStreamBestEffort`.

## §3 Async `AsyncReader`

Das idiomatische Swift-Concurrency-Modell: `AsyncReader.stream()` liefert
einen `AsyncStream<[UInt8]>`, den der Consumer mit `for await` iteriert; eine
interne `Task` pollt den `Transport` (Backoff via `Task.sleep` bei leerem
Poll) und yielded entrahmte Sample-Bodies; `continuation.onTermination`
cancelt die Task.

`endpoints/swift` MUSS bereitstellen:

- `final class AsyncReader: Sendable` mit `init(_ transport: Transport)` und
  `func stream() -> AsyncStream<[UInt8]>`.
- Sendeseitig übernimmt derselbe `Client.write` (§2) das Framing für den
  async-lesenden Peer — kein separater `AsyncWriter`-Typ jenseits des
  reliable `AsyncWriter` (§4); beide teilen sich das XRCE-Framing aus §1 und
  denselben `Transport`-Vertrag.
- `MemTransport` — eine `NSLock`-geschützte In-Memory-FIFO-`Transport`-
  Implementierung für Tests + Beispiele (`final class MemTransport: Transport, @unchecked Sendable`).

## §4 Reliable Stream

`endpoints/swift` implementiert den reliable Stream als Endpoint-Fähigkeit
gemäß [`reliable-endpoint` v1.0](reliable-endpoint-1.0.md) — Sender-/
Receiver-State-Machine, HEARTBEAT/ACKNACK-Wire-Codec sowie den
async-entkoppelten `AsyncWriter`, dessen dedizierter Drain-`Thread` den
`Transport` und den reliable Sender-State besitzt, während der Producer nur
wait-free (`NSLock`-geschützter Ringpuffer-`write`, back-pressure via
`false`-Rückgabe) enqueued und nie in den Kernel geht.

Die Konstanten (`reliableSenderWindow=16`, `reliableReceiverBuffer=64`,
`reliableHeartbeatPeriodMs=500`, `reliableMaxPayload=65535`, reliable
Stream-ID `reliableStreamId=0x80`), der State-Machine-Kontrakt (`submit`/
`pendingHeartbeat`/`recvAckNack`/`getInFlight`/`inFlightSeqs` auf
`ReliableSender`; `recvData`/`drainInOrder`/`pendingAckNack`/`reset` auf
`ReliableReceiver`) und das Wire-Format (HEARTBEAT `0x0B`, ACKNACK `0x0A`,
RFC-1982 16-bit Sequenznummern, `seqLt`/`seqGt`) sind dort normativ
definiert; `endpoints/swift/Reliable.swift` ist die Swift-Bindung dieses
Kontrakts, byte-identisch zu `crates/xrce/src/reliable.rs` und jedem anderen
Endpoint-SDK.

`Reliable.swift` ist self-contained (nur `Foundation`, für
`Date`/`Thread`/`NSLock`/`DispatchSemaphore`) und wird als eigenes
`ZeroddsReliable`-Modul kompiliert, damit es nie mit den Wire-Core-Typen
(`Endianness`/`Writer`/`Reader`) eines generierten IDL-Moduls kollidiert —
dasselbe Verfahren wie für `Zerodds` in §6.

## §5 Conformance

Eine Swift-Endpoint-Implementierung ist konform, wenn:

1. Der volle Stack (generierte Typen + `endpoints/swift`) ein typisiertes
   Sample sowohl über den sync `Client` als auch über `AsyncReader` mit dem
   geteilten Rust-XRCE-Peer austauscht.
2. HEARTBEAT- und ACKNACK-Frames byte-identisch zu den Referenz-Goldens sind
   und der reliable Stream Datagramm-Verlust lückenlos in-order aufholt
   (§4, `reliable-endpoint` v1.0 §5).
3. Ein Latenz-Messwert zeigt, dass `AsyncWriter.write` (Ringpuffer-Push)
   messbar unter einem inline `sendto` liegt — der Beleg, dass Async-Write
   die Syscall-Latenz aus dem Producer-Pfad nimmt.
4. Der Wire-Core selbst (`Writer`/`Reader`, §1 der `zerodds-xcdr2-swift`
   Spec) byte-identisch zur Rust-Referenz ist, sowohl big- als auch
   little-endian.

## §6 Modul-Layout + Beispiele

Ein generiertes IDL-Modul und die SDKs (`Zerodds`, `ZeroddsReliable`)
definieren jeweils eigene Wire-Core-Typen (`Endianness`/`Writer`/`Reader`),
die in einem gemeinsamen Modul kollidieren würden. Deshalb wird jedes SDK
separat als Objekt + `.swiftmodule` kompiliert (`-emit-module` +
`-emit-object`) und die App linkt dagegen (`swiftc -I <dir> app.swift
sdk.o`); der generierte Code bleibt im App-Modul. Dieses Verfahren nutzen
sowohl der Ping-Pong- als auch der Reliable-E2E-Harness
(`crates/endpoint-e2e/tests/swift.rs`, `crates/endpoint-e2e/tests/swift_reliable.rs`).

- Sync: `endpoints/swift/Sources/ZeroddsExampleSync` — `Client.poll()` in
  einer Schleife.
- Async: `endpoints/swift/Sources/ZeroddsExampleAsync` — `for await` über
  `AsyncReader.stream()`.
- Reliable: `endpoints/swift/example_reliable.swift` — In-Process-Demo (kein
  Socket) der Loss-Recovery.
- Quickstart: `endpoints/swift/QUICKSTART.md`.

## §7 Errata + Open-Questions

Der Swift-Toolchain (`swiftc`) steht auf `codepit` (Linux-Bench-Host) nicht
zur Verfügung; alle Swift-Endpoint-Tests laufen ausschließlich lokal auf
macOS (`swiftc_available()`-Gate in den E2E-Harnessen — loud-skip, kein
false-green). Ansonsten sind sync, async und reliable vollständig
implementiert und byte-verifiziert (siehe
`docs/spec-coverage/zerodds-endpoint-swift-1.0.md`).
