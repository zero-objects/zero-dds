# `zerodds-endpoint-go` 1.0 — Spec-Coverage

**Quelle:** `docs/specs/zerodds-endpoint-go-1.0.md` — ZeroDDS Go Endpoint-SDK-Spec.
Ergänzt die Codegen-Coverage `zerodds-xcdr2-go`
(`docs/spec-coverage/zerodds-xcdr2-go-1.0.md`) — dort das Marshalling, hier der
Transport.

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

**Tests:** `crates/endpoint-e2e/tests/go.rs::go_raw_udp` (rohes XCDR2 ohne
XRCE-Frame — eigener Mini-Harness); Framing selbst wird über `go_endpoint_sync`
und `go_endpoint_async` (§4) live geübt.

**Status:** done.

## §2 Sync `Client`

**Spec:** §2 — blockierender `Client`: `Write` framet + liefert synchron,
`Poll` ist ein nicht-blockierender Einzel-Receive, `Receive` blockiert bis zu
einem Timeout (pollend).

**Repo:** `endpoints/go/async.go::Transport`-Interface (`Deliver`/`Receive`,
der einzige Integrationspunkt); `endpoints/go/sync.go::Client`
(`NewClient`/`Write`/`Poll`/`Receive`, monotoner `seq`-Zähler, Default-Session
`XrceSessionNoKey`/`XrceStreamBestEffort`).

**Tests:** `endpoints/go/sync_test.go::TestSyncLoopback`; Live-E2E
`go_endpoint_sync` (§4).

**Status:** done.

## §3 Async `Reader`/`Writer`

**Spec:** §3 — Goroutine pollt den `Transport` und schiebt entrahmte
Sample-Bodies auf einen Channel (push); `AsyncWriter` als Sendeseiten-Gegenstück
über dasselbe Framing.

**Repo:** `endpoints/go/async.go::AsyncReader` (`NewAsyncReader`, `Samples`-Channel,
Hintergrund-`loop`, `Close`), `AsyncWriter` (`NewAsyncWriter`, `Write`).

**Tests:** `endpoints/go/async_test.go::TestAsyncLoopback` +
`TestAsyncUDP`; Live-E2E `go_endpoint_async` (§4).

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

**Tests (codepit):**
- `go_raw_udp` — generierter `Ping`/`Pong`-Codec direkt über einen rohen
  UDP-Socket, ohne XRCE-Framing.
- `go_endpoint_sync` — voller Stack über `zerodds.Client`.
- `go_endpoint_async` — voller Stack über `zerodds.AsyncReader`/`AsyncWriter`.

3/3 grün (codepit).

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
`endpoints/go/reliable_app` (live UDP-Sender-App für das E2E).

**Tests (codepit):**
- `go_reliable_unit_and_golden` (`crates/endpoint-e2e/tests/go_reliable.rs`)
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
- `go_reliable_loss_recovery` — Peer dropt jedes 3. Sample einmalig; die App
  (`reliable_app`) retransmittet auf ACKNACK; alle 12 Samples lückenlos in
  Reihenfolge geliefert.
- `go_reliable_no_loss` — lossless Baseline; 12/12.
- `go_reliable_example` — `example_reliable` läuft und meldet
  `sequence 0..11 verified in order` + `RELIABLE OK`.

5/5 grün (codepit), davon 4 in diesem Abschnitt (Latenz-Bench in §6).

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

**Tests (codepit):** `go_reliable_latency_bench`
(`crates/endpoint-e2e/tests/go_reliable.rs`) — Ring-Enqueue **20–25 ns** vs.
inline `sendto` **4360 ns** (~175–220×).

**Status:** done.

---

## Audit-Status

6 done / 0 partial / 0 open / 0 n/a (informativ) / 0 n/a (rejected).

Test-Lauf (codepit, verifiziert): `cargo test -p zerodds-endpoint-e2e --test go`
3/3 (Ping-Pong: `go_raw_udp`/`go_endpoint_sync`/`go_endpoint_async`);
`--test go_reliable` 5/5 (`go_reliable_unit_and_golden` — 21 Go-Unit-Tests inkl.
Byte-Golden, `go_reliable_loss_recovery`, `go_reliable_no_loss`,
`go_reliable_example`, `go_reliable_latency_bench`); Latenz-Bench Ring-Enqueue
20–25 ns / inline `sendto` 4360 ns (~175–220×).

Offene Punkte: keine.
