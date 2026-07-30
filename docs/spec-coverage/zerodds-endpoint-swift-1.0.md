# `zerodds-endpoint-swift` 1.0 — Spec-Coverage

**Quelle:** `docs/specs/zerodds-endpoint-swift-1.0.md` — ZeroDDS Swift
Endpoint-SDK-Spec. Ergänzt die Codegen-Coverage `zerodds-xcdr2-swift`
(`docs/spec-coverage/zerodds-xcdr2-swift-1.0.md`) — dort das Marshalling,
hier der Transport.

Implementation:

- `endpoints/swift/Sources/Zerodds/Zerodds.swift` — Wire-Core
  (`Writer`/`Reader`), XRCE-Framing (`xrceWriteFrame`/`xrceReadFrame`), sync
  `Client`, async `AsyncReader`, `Transport`-Protokoll, `MemTransport`.
- `endpoints/swift/Reliable.swift` — reliable Sender/Receiver-State-Machine +
  HEARTBEAT/ACKNACK-Wire-Codec + `AsyncWriter` (als eigenes
  `ZeroddsReliable`-Modul kompiliert).
- `endpoints/swift/Tests/ZeroddsTests/ZeroddsTests.swift` — SwiftPM-XCTest
  (Byte-Golden + sync/async Loopback über `MemTransport`).
- `crates/endpoint-e2e/tests/swift.rs` — Ping-Pong-E2E über echtes UDP;
  `crates/endpoint-e2e/tests/swift_reliable.rs` — reliable-Stream-E2E, Unit-
  Runner, Example, Latenz-Bench.

**Toolchain-Hinweis (gilt für alle Abschnitte):** `swiftc` steht auf
`codepit` (Linux-Bench-Host) nicht zur Verfügung. Alle Tests unten sind
**ausschließlich lokal auf macOS verifiziert** (dieser Durchlauf: macOS,
`swift-driver 1.148.6`, Swift 6.3.3, arm64). Die E2E-Harnesse gaten auf
`swiftc_available()` und melden bei fehlender Toolchain einen lauten Skip
(kein false-green) — nicht auf codepit re-verifiziert.

## §1 XRCE-Framing

**Spec:** §1 — 8-Byte-XRCE-Header (session, stream, seq LE, submsg id `0x07`
WRITE_DATA, flags, len LE) + Body, byte-identisch zu `crates/xrce` +
`endpoints/c`.

**Repo:** `endpoints/swift/Sources/Zerodds/Zerodds.swift` —
`xrceWriteFrame`/`xrceReadFrame`, Konstanten `xrceSessionNoKey` (`0x80`) und
`xrceStreamBestEffort` (`0x01`).

**Tests:** Framing wird über `swift_endpoint_sync`/`swift_endpoint_async`
(§4) live geübt; der Wire-Core selbst (`Writer`/`Reader`, cap-4-Alignment,
`f32`/`f64` via `.bitPattern`) über `ZeroddsTests.testByteIdentity`
(`endpoints/swift/Tests/ZeroddsTests/ZeroddsTests.swift`, `swift test`) gegen
`testdata/golden_le.bin`/`golden_be.bin` — LE **und** BE byte-identisch.

**Status:** done (lokal, macOS).

## §2 Sync `Client`

**Spec:** §2 — blockierender `Client`: `write` framet + liefert synchron,
`poll` ist ein nicht-blockierender Einzel-Receive.

**Repo:** `endpoints/swift/Sources/Zerodds/Zerodds.swift::Transport`-Protokoll
(`deliver`/`receive`, der einzige Integrationspunkt); `Client`
(`init`/`write`/`poll`, monotoner `seq`-Zähler, Default-Session
`xrceSessionNoKey`/`xrceStreamBestEffort`); `MemTransport` als
`NSLock`-geschützte In-Memory-FIFO.

**Tests:** `ZeroddsTests.testSyncLoopback` (`swift test`, 5 Samples über
`MemTransport`); Live-E2E `swift_endpoint_sync` (§4).

**Status:** done (lokal, macOS).

## §3 Async `AsyncReader`

**Spec:** §3 — `AsyncReader.stream()` liefert einen `AsyncStream<[UInt8]>`;
eine interne `Task` pollt den `Transport` und yielded entrahmte
Sample-Bodies; `for await` auf Consumer-Seite, `onTermination` cancelt die
Task.

**Repo:** `endpoints/swift/Sources/Zerodds/Zerodds.swift::AsyncReader`
(`init`, `stream()`); Sendeseitig teilt sich der async Pfad `Client.write`
(§2) — kein separater `AsyncWriter`-Typ jenseits des reliable `AsyncWriter`
(§5).

**Tests:** `ZeroddsTests.testAsyncLoopback` (`swift test`, 5 Samples über
`MemTransport`, `for await`); Live-E2E `swift_endpoint_async` (§4).

**Status:** done (lokal, macOS).

## §4 Ping-Pong-E2E (live)

**Spec:** §5.1 — eine Swift-App tauscht mit dem geteilten Rust-XRCE-Peer über
einen echten UDP-Socket ein typisiertes Sample aus: voller Stack (generierte
Typen + Endpoint-SDK), einmal sync über `Client`, einmal async über
`AsyncReader`. §6 — SDK und generiertes Modul werden getrennt kompiliert
(`Zerodds`-Objekt + `.swiftmodule`), damit die geteilten Wire-Core-Typnamen
(`Endianness`/`Writer`/`Reader`) nicht kollidieren.

**Repo:** `crates/endpoint-e2e/tests/swift.rs` — `build_swift_app` kompiliert
`Zerodds.swift` als eigenes Modul (`-emit-module`/`-emit-object`), dann `app`
(generierte `Ping`/`Pong`-Typen + `APP_MAIN`) dagegen; `UDPTransport`
implementiert `Transport` über einen rohen, nicht-blockierenden UDP-Socket.

**Tests (lokal, macOS, dieser Durchlauf):**
- `swift_endpoint_sync` — voller Stack über `Client`.
- `swift_endpoint_async` — voller Stack über `AsyncReader`.

2/2 grün (`cargo test -p zerodds-endpoint-e2e --test swift`, macOS lokal;
nicht auf codepit — kein `swiftc` dort).

**Status:** done (lokal, macOS; codepit nicht verifizierbar mangels
Toolchain).

## §5 Reliable Stream — State-Machine, Wire, Async-Writer

**Spec:** §4 (verweist auf `reliable-endpoint` v1.0 §3/§4) — XRCE reliable
Stream (`stream_id 0x80`, §8.4.10/§8.4.11), spiegelt die Referenz
`crates/xrce/src/reliable.rs`: `ReliableSender.submit`/`pendingHeartbeat`/
`recvAckNack`/`getInFlight`/`inFlightSeqs`; `ReliableReceiver.recvData`/
`drainInOrder`/`pendingAckNack`/`reset`. Window 16, Receiver-Buffer 64,
Heartbeat 500 ms, Payload ≤ 65535, RFC-1982 16-bit Sequenznummern. Dazu der
async-entkoppelte `AsyncWriter`: der Producer enqueued wait-free
(`NSLock`-geschützter Ringpuffer, back-pressure via `false`-Rückgabe), ein
dedizierter Drain-`Thread` hält den `ReliableSender`-State und macht die
gesamte I/O (senden, Heartbeat, ACKNACK-getriebenes Retransmit) — der
Producer geht nie in den Kernel.

**Repo:** `endpoints/swift/Reliable.swift` —
`writeDataFrame`/`parseWriteData`, `heartbeatFrame`/`parseHeartbeat`,
`acknackFrame`/`parseAckNack`; `ReliableSender`, `ReliableReceiver`;
`AsyncWriter` (Ringpuffer `queue`, `NSLock`, `init(sendFn:recvFn:)`/`start`/
`write`/`close`, `DispatchSemaphore` fürs Shutdown-Rendezvous);
`endpoints/swift/example_reliable.swift` (lauffähige In-Process-Demo, kein
Socket); `endpoints/swift/reliable_tests.swift` (Standalone-Unit-Runner,
mirrort `crates/xrce/src/reliable.rs`'s `#[cfg(test)]`, kein `XCTest` —
plain `swiftc`/`swift`, druckt `UNIT OK`).

**Tests (lokal, macOS, dieser Durchlauf):**
- `swift_reliable_unit` (`crates/endpoint-e2e/tests/swift_reliable.rs`)
  kompiliert `reliable_tests.swift` gegen das `ZeroddsReliable`-Modul und
  führt es aus — 16 `run()`-Testszenarien: monotone seq
  (`submit_assigns_monotonic_seqnrs`), Payload-zu-groß
  (`submit_rejects_payload_too_large`), Window-full
  (`submit_rejects_when_window_full`), Heartbeat first/silence/leer
  (`pending_heartbeat_fires_first_time`/
  `pending_heartbeat_silenced_until_period_elapsed`/
  `pending_heartbeat_none_when_window_empty`), ACKNACK Teil-/Voll-Clear
  (`recv_acknack_clears_acked_seqnrs`/
  `recv_acknack_full_clear_when_no_bits_set`), Receiver In-Order/Reorder/
  Dedup/Buffer-full (`recv_data_buffers_in_order`/
  `recv_data_reorders_out_of_order`/`recv_data_drops_duplicates`/
  `recv_data_rejects_when_buffer_full`), Pending-ACKNACK-Bitmap
  (`pending_acknack_marks_missing_slots`), Reset
  (`reset_clears_state_completely`), Byte-Golden
  (`byte_golden_heartbeat_acknack`: `heartbeatFrame(1,3,0x80)` ==
  `80 00 01 00 0B 01 05 00 01 00 03 00 80`, `acknackFrame(1,0,0x80)` ==
  `80 00 01 00 0A 01 05 00 01 00 00 00 80` — identisch zu den
  Referenz-Goldens), In-Process-End-to-End-Loss-Recovery
  (`end_to_end_sender_receiver_with_loss_recovery`).
- `swift_reliable_loss_recovery` — Peer dropt jedes 3. Sample einmalig; die
  App retransmittet direkt über `ReliableSender`/ACKNACK; alle 12 Samples
  lückenlos in Reihenfolge geliefert.
- `swift_reliable_no_loss` — lossless Baseline; 12/12.
- `swift_reliable_example` — `example_reliable.swift` läuft und meldet
  `sequence 0..11 verified in order`.

5/5 grün (`cargo test -p zerodds-endpoint-e2e --test swift_reliable`, macOS
lokal), davon 4 in diesem Abschnitt (Latenz-Bench in §6). Nicht auf codepit —
kein `swiftc` dort.

**Status:** done (lokal, macOS; codepit nicht verifizierbar mangels
Toolchain).

## §6 Latenz — Ring-Enqueue vs. inline `sendto`

**Spec:** §5.3 — der Producer-Pfad des `AsyncWriter` (`write` →
`NSLock`-Ringpuffer-Push) muss messbar unter dem inline `sendto`-Syscall
liegen — der Beleg, dass Async-Write die Syscall-Latenz aus dem
Producer-Pfad nimmt, nicht das Warten auf ACKNACK.

**Repo:** `endpoints/swift/Reliable.swift::AsyncWriter`; Bench-Harness in
`crates/endpoint-e2e/tests/swift_reliable.rs::SWIFT_RELIABLE_MAIN`
(`modeBench`) — 4000 Iterationen inline `sendto` (UDP) vs. 4000 Iterationen
`AsyncWriter.write` nach 100 Warmup-Iterationen, kein Live-Peer nötig.

**Tests (lokal, macOS, dieser Durchlauf):** `swift_reliable_latency_bench`
(`crates/endpoint-e2e/tests/swift_reliable.rs`) — zwei Messläufe:
Ring-Enqueue **426–560 ns** vs. inline `sendto` **3829–3871 ns**
(~7–9×). Assert im Test: `decoupled_ns < inline_ns` (kein fixer Faktor,
nur die Ungleichung). Messwert schwankt lauf-zu-lauf (Mikrobenchmark-Rauschen
auf einer Entwicklermaschine); die Richtung (entkoppelt schneller als
inline) ist stabil.

**Status:** done (lokal, macOS; codepit nicht verifizierbar mangels
Toolchain).

---

## Audit-Status

5 done / 0 partial / 0 open / 0 n/a (informativ) / 0 n/a (rejected).

Test-Lauf (macOS lokal, verifiziert dieser Durchlauf):
`cargo test -p zerodds-endpoint-e2e --test swift` 2/2
(`swift_endpoint_sync`/`swift_endpoint_async`); `--test swift_reliable` 5/5
(`swift_reliable_unit` — 16 Swift-Unit-Testszenarien inkl. Byte-Golden,
`swift_reliable_loss_recovery`, `swift_reliable_no_loss`,
`swift_reliable_example`, `swift_reliable_latency_bench`); zusätzlich
`swift test` (SwiftPM XCTest) 3/3 (`testByteIdentity`, `testSyncLoopback`,
`testAsyncLoopback`). Latenz-Bench Ring-Enqueue 426–560 ns / inline `sendto`
3829–3871 ns (~7–9×, zwei Messläufe).

Offene Punkte: keine funktionale Lücke. Gemessen: alle Tests laufen
ausschließlich lokal auf macOS — `codepit` hat keinen `swiftc`, daher kein
Linux/CI-Nachweis für dieses SDK (im Gegensatz zu Go/Zig/Nim, die auf
codepit laufen). Das ist eine Toolchain-Grenze, kein Spec-Gap.
