# `zerodds-endpoint-go` 1.0 — Spec Coverage

**Source:** `docs/specs/zerodds-endpoint-go-1.0.md` — the ZeroDDS Go endpoint
SDK spec. Complements the codegen coverage `zerodds-xcdr2-go`
(`docs/spec-coverage/zerodds-xcdr2-go-1.0.md`) — that doc covers marshalling,
this one covers transport.

Implementation:

- `endpoints/go/endpoint.go` — XRCE framing (`XrceWriteFrame`/`XrceReadFrame`).
- `endpoints/go/sync.go` — sync `Client`.
- `endpoints/go/async.go` — `Transport` interface, async `AsyncReader`/`AsyncWriter`.
- `endpoints/go/reliable.go` — reliable sender/receiver state machine +
  HEARTBEAT/ACKNACK wire codec + `ReliableAsyncWriter`.
- `crates/endpoint-e2e/tests/go.rs` — ping-pong E2E; `crates/endpoint-e2e/tests/go_reliable.rs` —
  reliable-stream E2E.

## §1 XRCE framing

**Spec:** §1 — 8-byte XRCE header (session, stream, seq LE, submsg id `0x07`
WRITE_DATA, flags, len LE) + body, byte-identical to `crates/xrce` +
`endpoints/c`.

**Repo:** `endpoints/go/endpoint.go` — `XrceWriteFrame`/`XrceReadFrame`,
constants `XrceSessionNoKey` (`0x80`) and `XrceStreamBestEffort` (`0x01`).

**Tests:** `crates/endpoint-e2e/tests/go.rs::go_raw_udp` (raw XCDR2 with no
XRCE frame — its own minimal harness); the framing itself is exercised live
via `go_endpoint_sync` and `go_endpoint_async` (§4).

**Status:** done.

## §2 Sync `Client`

**Spec:** §2 — blocking `Client`: `Write` frames + delivers synchronously,
`Poll` is a single non-blocking receive, `Receive` blocks up to a timeout
(polling).

**Repo:** `endpoints/go/async.go::Transport` interface (`Deliver`/`Receive`,
the sole integration point); `endpoints/go/sync.go::Client`
(`NewClient`/`Write`/`Poll`/`Receive`, monotonic `seq` counter, default
session `XrceSessionNoKey`/`XrceStreamBestEffort`).

**Tests:** `endpoints/go/sync_test.go::TestSyncLoopback`; live E2E
`go_endpoint_sync` (§4).

**Status:** done.

## §3 Async `Reader`/`Writer`

**Spec:** §3 — a goroutine polls the `Transport` and pushes unframed sample
bodies onto a channel (push); `AsyncWriter` is the send-side counterpart over
the same framing.

**Repo:** `endpoints/go/async.go::AsyncReader` (`NewAsyncReader`, `Samples`
channel, background `loop`, `Close`), `AsyncWriter` (`NewAsyncWriter`,
`Write`).

**Tests:** `endpoints/go/async_test.go::TestAsyncLoopback` +
`TestAsyncUDP`; live E2E `go_endpoint_async` (§4).

**Status:** done.

## §4 Ping-pong E2E (live)

**Spec:** §5.1/§5.2 — a Go app exchanges a typed sample with the shared Rust
XRCE peer over a real UDP socket: once as raw generated codec with no XRCE
frame, once as the full stack (generated types + endpoint SDK) sync and
async.

**Repo:** `crates/endpoint-e2e/tests/go.rs` — `GO_RAW_MAIN` (raw UDP, no XRCE
frame, uses only the generated `MarshalXCDR`/`UnmarshalXCDRPong`),
`GO_ENDPOINT_MAIN` (`udpTransport` implementing `zerodds.Transport` + the
`zeroddsendpoint` module, mode `sync`/`async` via CLI argument).

**Tests (codepit):**
- `go_raw_udp` — generated `Ping`/`Pong` codec directly over a raw UDP
  socket, no XRCE framing.
- `go_endpoint_sync` — full stack via `zerodds.Client`.
- `go_endpoint_async` — full stack via `zerodds.AsyncReader`/`AsyncWriter`.

3/3 passed (codepit).

**Status:** done.

## §5 Reliable stream — state machine, wire, async writer

**Spec:** §4 (references `reliable-endpoint` v1.0 §3/§4) — XRCE reliable
stream (`stream_id 0x80`, §8.4.10/§8.4.11), mirroring the reference
`crates/xrce/src/reliable.rs`: `ReliableSender.Submit`/`PendingHeartbeat`/
`RecvAcknack`/`GetInFlight`; `ReliableReceiver.RecvData`/`DrainInOrder`/
`PendingAcknack`/`Reset`. Window 16, receiver buffer 64, heartbeat 500 ms,
payload ≤ 65535, RFC-1982 16-bit sequence numbers. Alongside it, the
async-decoupled `ReliableAsyncWriter`: the producer enqueues wait-free onto a
buffered channel, a dedicated drain goroutine holds the `ReliableSender`
state and does all the I/O (send, heartbeat, ACKNACK-driven retransmit) — the
producer never enters the kernel.

**Repo:** `endpoints/go/reliable.go` — `ReliableWriteFrame`/`ReliableReadFrame`,
`HeartbeatFrame`/`ParseHeartbeat`, `AckNackFrame`/`ParseAckNack`;
`ReliableSender`, `ReliableReceiver`; `ReliableAsyncWriter` (buffered channel
`in`, `NewReliableAsyncWriter`/`Enqueue`/`Close`/`drain`);
`endpoints/go/example_reliable` (runnable in-process demo, no socket);
`endpoints/go/reliable_app` (live UDP sender app for the E2E).

**Tests (codepit):**
- `go_reliable_unit_and_golden` (`crates/endpoint-e2e/tests/go_reliable.rs`)
  runs `go test` with `-run` against `endpoints/go/reliable_test.go` — 21 test
  functions: monotonic seq (`TestSubmitAssignsMonotonicSeqnrs`),
  payload-too-large (`TestSubmitRejectsPayloadTooLarge`), window-full
  (`TestSubmitRejectsWhenWindowFull`), heartbeat first/silence/none
  (`TestPendingHeartbeatFiresFirstTime`/`...SilencedUntilPeriodElapsed`/
  `...NoneWhenWindowEmpty`), ACKNACK partial/full clear
  (`TestRecvAcknackClearsAckedSeqnrs`/`...FullClearWhenNoBitsSet`), receiver
  reorder/dedup/buffer-full (`TestRecvDataBuffersInOrder`/
  `...ReordersOutOfOrder`/`...DropsDuplicates`/`...RejectsWhenBufferFull`),
  pending-ACKNACK bitmap (`TestPendingAcknackMarksMissingSlots`), reset
  (`TestResetClearsStateCompletely`), in-process end-to-end loss recovery
  (`TestEndToEndSenderReceiverWithLossRecovery`), in-order delivery over the
  reliable stream (`TestConfigSubmessagesDeliveredInOrderViaReliableStream`),
  RFC-1982 wraparound (`TestSeqLtGtRfc1982Wraparound`), byte-golden
  (`TestByteGoldenHeartbeat`/`TestByteGoldenAckNack`: `HeartbeatFrame(1,3)` ==
  `80 00 01 00 0B 01 05 00 01 00 03 00 80`, `AckNackFrame(1,0)` ==
  `80 00 01 00 0A 01 05 00 01 00 00 00 80` — identical to the reference
  goldens), frame round-trip parsing (`TestFrameRoundTripParsing`),
  `ReliableAsyncWriter` loss recovery (`TestReliableAsyncWriterLossRecovery`).
- `go_reliable_loss_recovery` — peer drops every 3rd sample once; the app
  (`reliable_app`) retransmits on ACKNACK; all 12 samples delivered gap-free
  in order.
- `go_reliable_no_loss` — lossless baseline; 12/12.
- `go_reliable_example` — `example_reliable` runs and reports
  `sequence 0..11 verified in order` + `RELIABLE OK`.

5/5 passed (codepit), 4 of which belong to this section (latency bench in §6).

**Status:** done.

## §6 Latency — ring enqueue vs. inline `sendto`

**Spec:** §5.4 — the producer path of `ReliableAsyncWriter` (`Enqueue` →
channel push) must be measurably below the inline `sendto` syscall — the
evidence that async write removes syscall latency from the producer path,
not that it waits for ACKNACK.

**Repo:** `endpoints/go/reliable_bench` — 20000 iterations of inline
`conn.Write` (UDP `sendto`) vs. 20000 iterations of
`ReliableAsyncWriter.Enqueue`, no live peer needed (an arbitrary loopback
port, only local dispatch cost under measurement).

**Tests (codepit):** `go_reliable_latency_bench`
(`crates/endpoint-e2e/tests/go_reliable.rs`) — ring enqueue **20–25 ns** vs.
inline `sendto` **4360 ns** (~175–220×).

**Status:** done.

---

## Audit-Status

6 done / 0 partial / 0 open / 0 n/a (informative) / 0 n/a (rejected).

Test run (codepit, verified): `cargo test -p zerodds-endpoint-e2e --test go`
3/3 (ping-pong: `go_raw_udp`/`go_endpoint_sync`/`go_endpoint_async`);
`--test go_reliable` 5/5 (`go_reliable_unit_and_golden` — 21 Go unit tests
incl. byte-golden, `go_reliable_loss_recovery`, `go_reliable_no_loss`,
`go_reliable_example`, `go_reliable_latency_bench`); latency bench ring
enqueue 20–25 ns / inline `sendto` 4360 ns (~175–220×).

Open items: none.
