# `zerodds-endpoint-go` 1.0 — Spec Coverage

**Source:** `docs/specs/zerodds-endpoint-go-1.0.md` — the ZeroDDS Go endpoint
SDK spec. The marshalling comes from `crates/lang-rules`, column `Language::Go`; this document covers the transport.

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

**Tests:** `crates/endpoint-e2e/tests/go.rs::endpoint_go_5_1_the_generated_codec_pings_without_the_xrce_frame` (raw XCDR2 with no
XRCE frame — its own minimal harness); the framing itself is exercised live
via `xrce_8_3_5_8_go_sync_app_writes_the_ping_and_reads_the_pong` and `xrce_8_3_5_8_go_async_app_writes_the_ping_and_reads_the_pong` (§4).

**Status:** done.

## §2 Sync `Client`

**Spec:** §2 — blocking `Client`: `Write` frames + delivers synchronously,
`Poll` is a single non-blocking receive, `Receive` blocks up to a timeout
(polling).

**Repo:** `endpoints/go/async.go::Transport` interface (`Deliver`/`Receive`,
the sole integration point); `endpoints/go/sync.go::Client`
(`NewClient`/`Write`/`Poll`/`Receive`, monotonic `seq` counter, default
session `XrceSessionNoKey`/`XrceStreamBestEffort`).

**Tests:** `endpoints/go/sync_test.go::Test_endpointgo_2_sync_loopback`; live E2E
`xrce_8_3_5_8_go_sync_app_writes_the_ping_and_reads_the_pong` (§4).

**Status:** done.

## §3 Async `Reader`/`Writer`

**Spec:** §3 — a goroutine polls the `Transport` and pushes unframed sample
bodies onto a channel (push); `AsyncWriter` is the send-side counterpart over
the same framing.

**Repo:** `endpoints/go/async.go::AsyncReader` (`NewAsyncReader`, `Samples`
channel, background `loop`, `Close`), `AsyncWriter` (`NewAsyncWriter`,
`Write`).

**Tests:** `endpoints/go/async_test.go::Test_endpointgo_3_async_loopback` +
`TestAsyncUDP`; live E2E `xrce_8_3_5_8_go_async_app_writes_the_ping_and_reads_the_pong` (§4).

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

**Tests:** (Linux)
- `endpoint_go_5_1_the_generated_codec_pings_without_the_xrce_frame` — generated `Ping`/`Pong` codec directly over a raw UDP
  socket, no XRCE framing.
- `xrce_8_3_5_8_go_sync_app_writes_the_ping_and_reads_the_pong` — full stack via `zerodds.Client`.
- `xrce_8_3_5_8_go_async_app_writes_the_ping_and_reads_the_pong` — full stack via `zerodds.AsyncReader`/`AsyncWriter`.

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
`endpoints/go/reliable_app` (UDP sender app with a peer port and a count).

**Tests:** (Linux)
- `endpoints/go/reliable_test.go` in the `go` column of `tools/endpoints-gate`
  (job `integration-endpoints`)
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
- `reliable_5_3_go_samples_arrive_in_order_despite_dropped_datagrams` — peer drops every 3rd sample once; the app
  the test builds from the `Ping` the language's generator writes from the IDL of `crates/endpoint-e2e/src/codegen.rs`, and `ReliableAsyncWriter` retransmits on ACKNACK;
  all 12 samples delivered gap-free in order, the test reads every sample by the IDL's XCDR2 layout (`assert_reliable_pings`).
- `reliable_5_5_go_example_recovers_its_sequence_without_gaps` — `example_reliable` runs and reports
  `sequence 0..11 verified in order` + `RELIABLE OK`.

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

**Tests:** (Linux) `reliable_5_4_go_the_producer_enqueue_costs_less_than_the_inline_send`
(`crates/endpoint-e2e/tests/go_reliable.rs`) — ring enqueue **20–25 ns** vs.
inline `sendto` **4360 ns** (~175–220×).

**Status:** done.

---

## Audit-Status

6 done / 0 partial / 0 open / 0 blocked / 0 n/a (informative).

Test run: in CI, crate tier and pillar jobs; the cited tests with the latest result of each job in the pipelines 8501 to 9053 of `main` (`python3 ci/coverage_results.py --document zerodds-endpoint-go-1.0.md`):

* `python3 ci/run_integration.py integration-bindings` (job `integration-bindings`) — 6 tests green.
* `python3 ci/run_integration.py integration-endpoints` (job `integration-endpoints`) — 2 tests green.

8 cited tests green, 0 not green.

Open items: none.
