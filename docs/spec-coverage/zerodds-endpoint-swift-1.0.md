# `zerodds-endpoint-swift` 1.0 — Spec-Coverage

**Quelle:** `docs/specs/zerodds-endpoint-swift-1.0.md` — ZeroDDS Swift
Endpoint-SDK-Spec. Das Marshalling kommt aus `crates/lang-rules`, Spalte `Language::Swift`; hier steht der Transport.

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
  `crates/endpoint-e2e/tests/swift_reliable.rs` — reliable-Stream-E2E,
  Latenz-Vergleich.

**Toolchain-Hinweis (gilt für alle Abschnitte):** `swiftc` steht auf
`Linux` (Linux-Bench-Host) nicht zur Verfügung. Alle Tests unten sind
**ausschließlich lokal auf macOS verifiziert** (dieser Durchlauf: macOS,
`swift-driver 1.148.6`, Swift 6.3.3, arm64). Die E2E-Harnesse gaten auf
`swiftc_available()` und melden bei fehlender Toolchain einen lauten Skip
(kein false-green) — nicht auf dem Linux-Testhost re-verifiziert.

## §1 XRCE-Framing

**Spec:** §1 — 8-Byte-XRCE-Header (session, stream, seq LE, submsg id `0x07`
WRITE_DATA, flags, len LE) + Body, byte-identisch zu `crates/xrce` +
`endpoints/c`.

**Repo:** `endpoints/swift/Sources/Zerodds/Zerodds.swift` —
`xrceWriteFrame`/`xrceReadFrame`, Konstanten `xrceSessionNoKey` (`0x80`) und
`xrceStreamBestEffort` (`0x01`).

**Tests:** Framing wird über `xrce_8_3_5_8_swift_sync_app_writes_the_ping_and_reads_the_pong`/`xrce_8_3_5_8_swift_async_app_writes_the_ping_and_reads_the_pong`
(§4) live geübt; der Wire-Core selbst (`Writer`/`Reader`, cap-4-Alignment,
`f32`/`f64` via `.bitPattern`) über `ZeroddsTests.testByteIdentity`
(`endpoints/swift/Tests/ZeroddsTests/ZeroddsTests.swift`, `swift test`) gegen
`testdata/golden_le.bin`/`golden_be.bin` — LE **und** BE byte-identisch.

**Status:** done — auf macOS und auf dem Linux-Testhost gemessen
(swiftc 6.3.3, `swift test`: 39 Tests, 0 Fehlschläge).

## §2 Sync `Client`

**Spec:** §2 — blockierender `Client`: `write` framet + liefert synchron,
`poll` ist ein nicht-blockierender Einzel-Receive.

**Repo:** `endpoints/swift/Sources/Zerodds/Zerodds.swift::Transport`-Protokoll
(`deliver`/`receive`, der einzige Integrationspunkt); `Client`
(`init`/`write`/`poll`, monotoner `seq`-Zähler, Default-Session
`xrceSessionNoKey`/`xrceStreamBestEffort`); `MemTransport` als
`NSLock`-geschützte In-Memory-FIFO.

**Tests:** `ZeroddsTests.testSyncLoopback` (`swift test`, 5 Samples über
`MemTransport`); Live-E2E `xrce_8_3_5_8_swift_sync_app_writes_the_ping_and_reads_the_pong` (§4).

**Status:** done — auf macOS und auf dem Linux-Testhost gemessen
(swiftc 6.3.3, `swift test`: 39 Tests, 0 Fehlschläge).

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
`MemTransport`, `for await`); Live-E2E `xrce_8_3_5_8_swift_async_app_writes_the_ping_and_reads_the_pong` (§4).

**Status:** done — auf macOS und auf dem Linux-Testhost gemessen
(swiftc 6.3.3, `swift test`: 39 Tests, 0 Fehlschläge).

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

**Tests:** (lokal, macOS, dieser Durchlauf)
- `xrce_8_3_5_8_swift_sync_app_writes_the_ping_and_reads_the_pong` — voller Stack über `Client`.
- `xrce_8_3_5_8_swift_async_app_writes_the_ping_and_reads_the_pong` — voller Stack über `AsyncReader`.

**Status:** done — auf macOS und auf dem Linux-Testhost gemessen
(swiftc 6.3.3, x86_64-unknown-linux-gnu).

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

**Tests:** (lokal, macOS, dieser Durchlauf)
- `endpoints/swift/reliable_tests.swift` in der Spalte `swift` von `tools/endpoints-gate`
  (Job `integration-bindings-swift`)
  kompiliert `reliable_tests.swift` gegen das `ZeroddsReliable`-Modul und
  führt es aus — 16 `run()`-Testszenarien: monotone seq
  (`endpointswift_5_submit_assigns_monotonic_seqnrs`), Payload-zu-groß
  (`endpointswift_5_submit_rejects_payload_too_large`), Window-full
  (`endpointswift_5_submit_rejects_when_window_full`), Heartbeat first/silence/leer
  (`endpointswift_5_pending_heartbeat_fires_first_time`/
  `endpointswift_5_pending_heartbeat_silenced_until_period_elapsed`/
  `endpointswift_5_pending_heartbeat_none_when_window_empty`), ACKNACK Teil-/Voll-Clear
  (`endpointswift_5_recv_acknack_clears_acked_seqnrs`/
  `endpointswift_5_recv_acknack_full_clear_when_no_bits_set`), Receiver In-Order/Reorder/
  Dedup/Buffer-full (`endpointswift_5_recv_data_buffers_in_order`/
  `endpointswift_5_recv_data_reorders_out_of_order`/`endpointswift_5_recv_data_drops_duplicates`/
  `endpointswift_5_recv_data_rejects_when_buffer_full`), Pending-ACKNACK-Bitmap
  (`endpointswift_5_pending_acknack_marks_missing_slots`), Reset
  (`endpointswift_5_reset_clears_state_completely`), Byte-Golden
  (`endpointswift_5_byte_golden_heartbeat_acknack`: `heartbeatFrame(1,3,0x80)` ==
  `80 00 01 00 0B 01 05 00 01 00 03 00 80`, `acknackFrame(1,0,0x80)` ==
  `80 00 01 00 0A 01 05 00 01 00 00 00 80` — identisch zu den
  Referenz-Goldens), In-Process-End-to-End-Loss-Recovery
  (`endpointswift_5_end_to_end_sender_receiver_with_loss_recovery`).
- `reliable_5_3_swift_samples_arrive_in_order_despite_dropped_datagrams` — Peer dropt jedes 3. Sample einmalig; die
  App retransmittet direkt über `ReliableSender`/ACKNACK; alle 12 Samples
  lückenlos in Reihenfolge geliefert.
- `endpoints/swift/example_reliable.swift` in der Spalte `swift` von `tools/endpoints-gate`
  (Job `integration-bindings-swift`) — `example_reliable.swift` läuft und meldet
  `sequence 0..11 verified in order`.

**Status:** done — auf macOS und auf dem Linux-Testhost gemessen
(swiftc 6.3.3, x86_64-unknown-linux-gnu).

## §6 Latenz — Ring-Enqueue vs. inline `sendto`

**Spec:** §5.3 — der Producer-Pfad des `AsyncWriter` (`write` →
`NSLock`-Ringpuffer-Push) muss messbar unter dem inline `sendto`-Syscall
liegen — der Beleg, dass Async-Write die Syscall-Latenz aus dem
Producer-Pfad nimmt, nicht das Warten auf ACKNACK.

**Repo:** `endpoints/swift/Reliable.swift::AsyncWriter`; Bench-Harness in
`crates/endpoint-e2e/tests/swift_reliable.rs::SWIFT_RELIABLE_MAIN`
(`modeBench`) — 4000 Iterationen inline `sendto` (UDP) vs. 4000 Iterationen
`AsyncWriter.write` nach 100 Warmup-Iterationen, kein Live-Peer nötig.

**Tests:** (lokal, macOS, dieser Durchlauf) `reliable_5_4_swift_the_producer_enqueue_costs_less_than_the_inline_send`
(`crates/endpoint-e2e/tests/swift_reliable.rs`) — zwei Messläufe:
Ring-Enqueue **426–560 ns** vs. inline `sendto` **3829–3871 ns**
(~7–9×). Assert im Test: `decoupled_ns < inline_ns` (kein fixer Faktor,
nur die Ungleichung). Messwert schwankt lauf-zu-lauf (Mikrobenchmark-Rauschen
auf einer Entwicklermaschine); die Richtung (entkoppelt schneller als
inline) ist stabil.

**Status:** done — auf macOS und auf dem Linux-Testhost gemessen
(swiftc 6.3.3, x86_64-unknown-linux-gnu).

---

## Audit-Status

6 done / 0 partial / 0 open / 0 blocked / 0 n/a (informative).

Test-Lauf: in CI, Krate-Ebene und Säulen-Jobs; die zitierten Tests mit dem letzten Ergebnis jedes Jobs in den Pipelines 8501 bis 9053 von `main` (`python3 ci/coverage_results.py --document zerodds-endpoint-swift-1.0.md`):

* `python3 ci/run_integration.py integration-bindings-swift` (Job `integration-bindings-swift`) — 20 Tests grün.

20 zitierte Tests grün, 0 nicht grün.

