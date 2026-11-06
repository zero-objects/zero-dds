# `zerodds-endpoint-go` 1.0 — Spec-Coverage

**Quelle:** `docs/specs/zerodds-endpoint-go-1.0.md` — ZeroDDS Go Endpoint-SDK-Spec.
Das Marshalling kommt aus `crates/lang-rules`, Spalte `Language::Go`; hier steht der Transport.

Implementation:

- `endpoints/go/endpoint.go` — XRCE-Framing (`XrceWriteFrame`/`XrceReadFrame`).
- `endpoints/go/sync.go` — sync `Client`.
- `endpoints/go/async.go` — `Transport`-Interface, async `AsyncReader`/`AsyncWriter`.
- `endpoints/go/reliable.go` — reliable Sender/Receiver-State-Machine +
  HEARTBEAT/ACKNACK-Wire-Codec + `ReliableAsyncWriter`.
- `crates/endpoint-e2e/tests/go.rs` — Ping-Pong-E2E; `crates/endpoint-e2e/tests/go_reliable.rs` —
  reliable-Stream-E2E.

## §1 XRCE-Framing

**Spec:** §1 — 8-Byte-XRCE-Header (session, stream, seq LE, submsg id `0x07`
WRITE_DATA, flags, len LE) + Body, byte-identisch zu `crates/xrce` +
`endpoints/c`.

**Repo:** `endpoints/go/endpoint.go` — `XrceWriteFrame`/`XrceReadFrame`,
Konstanten `XrceSessionNoKey` (`0x80`) und `XrceStreamBestEffort` (`0x01`).

**Tests:** `crates/endpoint-e2e/tests/go.rs::endpoint_go_5_1_the_generated_codec_pings_without_the_xrce_frame` (rohes XCDR2 ohne
XRCE-Frame — eigener Mini-Harness); Framing selbst wird über `xrce_8_3_5_8_go_sync_app_writes_the_ping_and_reads_the_pong`
und `xrce_8_3_5_8_go_async_app_writes_the_ping_and_reads_the_pong` (§4) live geübt.

**Status:** done.

## §2 Sync `Client`

**Spec:** §2 — blockierender `Client`: `Write` framet + liefert synchron,
`Poll` ist ein nicht-blockierender Einzel-Receive, `Receive` blockiert bis zu
einem Timeout (pollend).

**Repo:** `endpoints/go/async.go::Transport`-Interface (`Deliver`/`Receive`,
der einzige Integrationspunkt); `endpoints/go/sync.go::Client`
(`NewClient`/`Write`/`Poll`/`Receive`, monotoner `seq`-Zähler, Default-Session
`XrceSessionNoKey`/`XrceStreamBestEffort`).

**Tests:** `endpoints/go/sync_test.go::Test_endpointgo_2_sync_loopback`; Live-E2E
`xrce_8_3_5_8_go_sync_app_writes_the_ping_and_reads_the_pong` (§4).

**Status:** done.

## §3 Async `Reader`/`Writer`

**Spec:** §3 — Goroutine pollt den `Transport` und schiebt entrahmte
Sample-Bodies auf einen Channel (push); `AsyncWriter` als Sendeseiten-Gegenstück
über dasselbe Framing.

**Repo:** `endpoints/go/async.go::AsyncReader` (`NewAsyncReader`, `Samples`-Channel,
Hintergrund-`loop`, `Close`), `AsyncWriter` (`NewAsyncWriter`, `Write`).

**Tests:** `endpoints/go/async_test.go::Test_endpointgo_3_async_loopback` +
`TestAsyncUDP`; Live-E2E `xrce_8_3_5_8_go_async_app_writes_the_ping_and_reads_the_pong` (§4).

**Status:** done.

## §4 Ping-Pong-E2E (live)

**Spec:** §5.1/§5.2 — eine Go-App tauscht mit dem geteilten Rust-XRCE-Peer über
einen echten UDP-Socket ein typisiertes Sample aus: einmal roher generierter
Codec ohne XRCE-Frame, einmal voller Stack (generierte Typen + Endpoint-SDK)
sync und async.

**Repo:** `crates/endpoint-e2e/tests/go.rs` — `GO_RAW_MAIN` (rohes UDP, kein
XRCE-Frame, nutzt nur den generierten `MarshalXCDR`/`UnmarshalXCDRPong`),
`GO_ENDPOINT_MAIN` (`udpTransport` über `zerodds.Transport` +
`zeroddsendpoint`-Modul, Modus `sync`/`async` per CLI-Argument).

**Tests:** (Linux)
- `endpoint_go_5_1_the_generated_codec_pings_without_the_xrce_frame` — generierter `Ping`/`Pong`-Codec direkt über einen rohen
  UDP-Socket, ohne XRCE-Framing.
- `xrce_8_3_5_8_go_sync_app_writes_the_ping_and_reads_the_pong` — voller Stack über `zerodds.Client`.
- `xrce_8_3_5_8_go_async_app_writes_the_ping_and_reads_the_pong` — voller Stack über `zerodds.AsyncReader`/`AsyncWriter`.

**Status:** done.

## §5 Reliable Stream — State-Machine, Wire, Async-Writer

**Spec:** §4 (verweist auf `reliable-endpoint` v1.0 §3/§4) — XRCE reliable
Stream (`stream_id 0x80`, §8.4.10/§8.4.11), spiegelt die Referenz
`crates/xrce/src/reliable.rs`: `ReliableSender.Submit`/`PendingHeartbeat`/
`RecvAcknack`/`GetInFlight`; `ReliableReceiver.RecvData`/`DrainInOrder`/
`PendingAcknack`/`Reset`. Window 16, Receiver-Buffer 64, Heartbeat 500 ms,
Payload ≤ 65535, RFC-1982 16-bit Sequenznummern. Dazu der async-entkoppelte
`ReliableAsyncWriter`: der Producer enqueued wait-free in einen gepufferten
Channel, eine dedizierte Drain-Goroutine hält den `ReliableSender`-State und
macht die gesamte I/O (senden, Heartbeat, ACKNACK-getriebenes Retransmit) —
der Producer geht nie in den Kernel.

**Repo:** `endpoints/go/reliable.go` — `ReliableWriteFrame`/`ReliableReadFrame`,
`HeartbeatFrame`/`ParseHeartbeat`, `AckNackFrame`/`ParseAckNack`;
`ReliableSender`, `ReliableReceiver`; `ReliableAsyncWriter`
(gepufferter Channel `in`, `NewReliableAsyncWriter`/`Enqueue`/`Close`/`drain`);
`endpoints/go/example_reliable` (lauffähige In-Process-Demo, kein Socket);
`endpoints/go/reliable_app` (UDP-Sender-App mit Peer-Port und Anzahl).

**Tests:** (Linux)
- `endpoints/go/reliable_test.go` in der Spalte `go` von `tools/endpoints-gate`
  (Job `integration-endpoints`)
  läuft `go test` mit `-run` auf `endpoints/go/reliable_test.go` — 21
  Testfunktionen: monotone seq (`TestSubmitAssignsMonotonicSeqnrs`),
  Payload-zu-groß (`TestSubmitRejectsPayloadTooLarge`), Window-full
  (`TestSubmitRejectsWhenWindowFull`), Heartbeat first/silence/leer
  (`TestPendingHeartbeatFiresFirstTime`/`...SilencedUntilPeriodElapsed`/
  `...NoneWhenWindowEmpty`), ACKNACK Teil-/Voll-Clear
  (`TestRecvAcknackClearsAckedSeqnrs`/`...FullClearWhenNoBitsSet`), Receiver
  Reorder/Dedup/Buffer-full (`TestRecvDataBuffersInOrder`/
  `...ReordersOutOfOrder`/`...DropsDuplicates`/`...RejectsWhenBufferFull`),
  Pending-ACKNACK-Bitmap (`TestPendingAcknackMarksMissingSlots`), Reset
  (`TestResetClearsStateCompletely`), In-Process-End-to-End-Loss-Recovery
  (`TestEndToEndSenderReceiverWithLossRecovery`), In-Order-Delivery über den
  reliable Stream (`TestConfigSubmessagesDeliveredInOrderViaReliableStream`),
  RFC-1982-Wraparound (`TestSeqLtGtRfc1982Wraparound`), Byte-Golden
  (`TestByteGoldenHeartbeat`/`TestByteGoldenAckNack`: `HeartbeatFrame(1,3)` ==
  `80 00 01 00 0B 01 05 00 01 00 03 00 80`, `AckNackFrame(1,0)` ==
  `80 00 01 00 0A 01 05 00 01 00 00 00 80` — identisch zu den Referenz-Goldens),
  Frame-Roundtrip-Parsing (`TestFrameRoundTripParsing`),
  `ReliableAsyncWriter`-Loss-Recovery (`TestReliableAsyncWriterLossRecovery`).
- `reliable_5_3_go_samples_arrive_in_order_despite_dropped_datagrams` — Peer dropt jedes 3. Sample einmalig; die App,
  die der Test aus dem `Ping`, den der Generator der Sprache aus der IDL von `crates/endpoint-e2e/src/codegen.rs` schreibt, und `ReliableAsyncWriter` baut, retransmittet auf
  ACKNACK; alle 12 Samples lückenlos in Reihenfolge geliefert, der Test liest jedes Sample nach dem XCDR2-Layout der IDL (`assert_reliable_pings`).
- `reliable_5_5_go_example_recovers_its_sequence_without_gaps` — `example_reliable` läuft und meldet
  `sequence 0..11 verified in order` + `RELIABLE OK`.

**Status:** done.

## §6 Latenz — Ring-Enqueue vs. inline `sendto`

**Spec:** §5.4 — der Producer-Pfad des `ReliableAsyncWriter`
(`Enqueue` → Channel-Push) muss messbar unter dem inline `sendto`-Syscall
liegen — der Beleg, dass Async-Write die Syscall-Latenz aus dem Producer-Pfad
nimmt, nicht das Warten auf ACKNACK.

**Repo:** `endpoints/go/reliable_bench` — 20000 Iterationen inline `conn.Write`
(UDP-`sendto`) vs. 20000 Iterationen `ReliableAsyncWriter.Enqueue`, kein Live-
Peer nötig (beliebiger Loopback-Port, nur lokale Dispatch-Kosten unter
Messung).

**Tests:** (Linux) `reliable_5_4_go_the_producer_enqueue_costs_less_than_the_inline_send`
(`crates/endpoint-e2e/tests/go_reliable.rs`) — Ring-Enqueue **20–25 ns** vs.
inline `sendto` **4360 ns** (~175–220×).

**Status:** done.

---

## Audit-Status

6 done / 0 partial / 0 open / 0 blocked / 0 n/a (informative).

Test-Lauf: in CI, Krate-Ebene und Säulen-Jobs; die zitierten Tests mit dem letzten Ergebnis jedes Jobs in den Pipelines 8501 bis 9053 von `main` (`python3 ci/coverage_results.py --document zerodds-endpoint-go-1.0.md`):

* `python3 ci/run_integration.py integration-bindings` (Job `integration-bindings`) — 6 Tests grün.
* `python3 ci/run_integration.py integration-endpoints` (Job `integration-endpoints`) — 2 Tests grün.

8 zitierte Tests grün, 0 nicht grün.

Offene Punkte: keine.
