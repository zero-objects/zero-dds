# `zerodds-endpoint-swift` 1.0 — Spec Coverage

**Source:** `docs/specs/zerodds-endpoint-swift-1.0.md` — the ZeroDDS Swift
endpoint SDK spec. The marshalling comes from `crates/lang-rules`, column `Language::Swift`; this document covers the transport.

Implementation:

- `endpoints/swift/Sources/Zerodds/Zerodds.swift` — wire core
  (`Writer`/`Reader`), XRCE framing (`xrceWriteFrame`/`xrceReadFrame`), sync
  `Client`, async `AsyncReader`, `Transport` protocol, `MemTransport`.
- `endpoints/swift/Reliable.swift` — reliable sender/receiver state machine +
  HEARTBEAT/ACKNACK wire codec + `AsyncWriter` (compiled as its own
  `ZeroddsReliable` module).
- `endpoints/swift/Tests/ZeroddsTests/ZeroddsTests.swift` — SwiftPM XCTest
  (byte-golden + sync/async loopback over `MemTransport`).
- `crates/endpoint-e2e/tests/swift.rs` — ping-pong E2E over real UDP;
  `crates/endpoint-e2e/tests/swift_reliable.rs` — reliable-stream E2E,
  latency comparison.

**Toolchain note (applies to every section):** `swiftc` is not available on
`Linux` (the Linux bench host). Every test below is **verified locally on
macOS only** (this run: macOS, `swift-driver 1.148.6`, Swift 6.3.3, arm64).
The E2E harnesses gate on `swiftc_available()` and emit a loud skip when the
toolchain is missing (no false-green) — not re-verified on the Linux test host.

## §1 XRCE framing

**Spec:** §1 — 8-byte XRCE header (session, stream, seq LE, submsg id `0x07`
WRITE_DATA, flags, len LE) + body, byte-identical to `crates/xrce` +
`endpoints/c`.

**Repo:** `endpoints/swift/Sources/Zerodds/Zerodds.swift` —
`xrceWriteFrame`/`xrceReadFrame`, constants `xrceSessionNoKey` (`0x80`) and
`xrceStreamBestEffort` (`0x01`).

**Tests:** framing is exercised live via `xrce_8_3_5_8_swift_sync_app_writes_the_ping_and_reads_the_pong`/
`xrce_8_3_5_8_swift_async_app_writes_the_ping_and_reads_the_pong` (§4); the wire core itself (`Writer`/`Reader`, cap-4
alignment, `f32`/`f64` via `.bitPattern`) via `ZeroddsTests.testByteIdentity`
(`endpoints/swift/Tests/ZeroddsTests/ZeroddsTests.swift`, `swift test`)
against `testdata/golden_le.bin`/`golden_be.bin` — LE **and** BE
byte-identical.

**Status:** done — measured on macOS and on the Linux test host
(swiftc 6.3.3, `swift test`: 39 tests, 0 failures).

## §2 Sync `Client`

**Spec:** §2 — blocking `Client`: `write` frames + delivers synchronously,
`poll` is a single non-blocking receive.

**Repo:** `endpoints/swift/Sources/Zerodds/Zerodds.swift::Transport`
protocol (`deliver`/`receive`, the sole integration point); `Client`
(`init`/`write`/`poll`, monotonic `seq` counter, default session
`xrceSessionNoKey`/`xrceStreamBestEffort`); `MemTransport`, an
`NSLock`-guarded in-memory FIFO.

**Tests:** `ZeroddsTests.testSyncLoopback` (`swift test`, 5 samples over
`MemTransport`); live E2E `xrce_8_3_5_8_swift_sync_app_writes_the_ping_and_reads_the_pong` (§4).

**Status:** done — measured on macOS and on the Linux test host
(swiftc 6.3.3, `swift test`: 39 tests, 0 failures).

## §3 Async `AsyncReader`

**Spec:** §3 — `AsyncReader.stream()` returns an `AsyncStream<[UInt8]>`; an
internal `Task` polls the `Transport` and yields unframed sample bodies; the
consumer iterates with `for await`, `onTermination` cancels the task.

**Repo:** `endpoints/swift/Sources/Zerodds/Zerodds.swift::AsyncReader`
(`init`, `stream()`); the send side of the async path shares `Client.write`
(§2) — no separate `AsyncWriter` type outside the reliable `AsyncWriter`
(§5).

**Tests:** `ZeroddsTests.testAsyncLoopback` (`swift test`, 5 samples over
`MemTransport`, `for await`); live E2E `xrce_8_3_5_8_swift_async_app_writes_the_ping_and_reads_the_pong` (§4).

**Status:** done — measured on macOS and on the Linux test host
(swiftc 6.3.3, `swift test`: 39 tests, 0 failures).

## §4 Ping-pong E2E (live)

**Spec:** §5.1 — a Swift app exchanges a typed sample with the shared Rust
XRCE peer over a real UDP socket: the full stack (generated types + endpoint
SDK), once sync via `Client`, once async via `AsyncReader`. §6 — the SDK and
the generated module are compiled separately (`Zerodds` object +
`.swiftmodule`) so the shared wire-core type names
(`Endianness`/`Writer`/`Reader`) don't collide.

**Repo:** `crates/endpoint-e2e/tests/swift.rs` — `build_swift_app` compiles
`Zerodds.swift` as its own module (`-emit-module`/`-emit-object`), then
`app` (generated `Ping`/`Pong` types + `APP_MAIN`) against it; `UDPTransport`
implements `Transport` over a raw, non-blocking UDP socket.

**Tests:** (local, macOS, this run)
- `xrce_8_3_5_8_swift_sync_app_writes_the_ping_and_reads_the_pong` — full stack via `Client`.
- `xrce_8_3_5_8_swift_async_app_writes_the_ping_and_reads_the_pong` — full stack via `AsyncReader`.

**Status:** done — measured on macOS and on the Linux test host
(swiftc 6.3.3, x86_64-unknown-linux-gnu).

## §5 Reliable stream — state machine, wire, async writer

**Spec:** §4 (references `reliable-endpoint` v1.0 §3/§4) — XRCE reliable
stream (`stream_id 0x80`, §8.4.10/§8.4.11), mirroring the reference
`crates/xrce/src/reliable.rs`: `ReliableSender.submit`/`pendingHeartbeat`/
`recvAckNack`/`getInFlight`/`inFlightSeqs`; `ReliableReceiver.recvData`/
`drainInOrder`/`pendingAckNack`/`reset`. Window 16, receiver buffer 64,
heartbeat 500 ms, payload ≤ 65535, RFC-1982 16-bit sequence numbers.
Alongside it, the async-decoupled `AsyncWriter`: the producer enqueues
wait-free (`NSLock`-guarded ring buffer, backpressure via a `false` return),
a dedicated drain `Thread` holds the `ReliableSender` state and does all the
I/O (send, heartbeat, ACKNACK-driven retransmit) — the producer never enters
the kernel.

**Repo:** `endpoints/swift/Reliable.swift` —
`writeDataFrame`/`parseWriteData`, `heartbeatFrame`/`parseHeartbeat`,
`acknackFrame`/`parseAckNack`; `ReliableSender`, `ReliableReceiver`;
`AsyncWriter` (ring buffer `queue`, `NSLock`, `init(sendFn:recvFn:)`/`start`/
`write`/`close`, `DispatchSemaphore` for shutdown rendezvous);
`endpoints/swift/example_reliable.swift` (runnable in-process demo, no
socket); `endpoints/swift/reliable_tests.swift` (standalone unit runner,
mirroring `crates/xrce/src/reliable.rs`'s `#[cfg(test)]`, not `XCTest` —
plain `swiftc`/`swift`, prints `UNIT OK`).

**Tests:** (local, macOS, this run)
- `endpoints/swift/reliable_tests.swift` in the `swift` column of `tools/endpoints-gate`
  (job `integration-bindings-swift`)
  compiles `reliable_tests.swift` against the `ZeroddsReliable` module and
  runs it — 16 `run()` test scenarios: monotonic seq
  (`endpointswift_5_submit_assigns_monotonic_seqnrs`), payload-too-large
  (`endpointswift_5_submit_rejects_payload_too_large`), window-full
  (`endpointswift_5_submit_rejects_when_window_full`), heartbeat first/silence/none
  (`endpointswift_5_pending_heartbeat_fires_first_time`/
  `endpointswift_5_pending_heartbeat_silenced_until_period_elapsed`/
  `endpointswift_5_pending_heartbeat_none_when_window_empty`), ACKNACK partial/full clear
  (`endpointswift_5_recv_acknack_clears_acked_seqnrs`/
  `endpointswift_5_recv_acknack_full_clear_when_no_bits_set`), receiver in-order/reorder/
  dedup/buffer-full (`endpointswift_5_recv_data_buffers_in_order`/
  `endpointswift_5_recv_data_reorders_out_of_order`/`endpointswift_5_recv_data_drops_duplicates`/
  `endpointswift_5_recv_data_rejects_when_buffer_full`), pending-ACKNACK bitmap
  (`endpointswift_5_pending_acknack_marks_missing_slots`), reset
  (`endpointswift_5_reset_clears_state_completely`), byte-golden
  (`endpointswift_5_byte_golden_heartbeat_acknack`: `heartbeatFrame(1,3,0x80)` ==
  `80 00 01 00 0B 01 05 00 01 00 03 00 80`, `acknackFrame(1,0,0x80)` ==
  `80 00 01 00 0A 01 05 00 01 00 00 00 80` — identical to the reference
  goldens), in-process end-to-end loss recovery
  (`endpointswift_5_end_to_end_sender_receiver_with_loss_recovery`).
- `reliable_5_3_swift_samples_arrive_in_order_despite_dropped_datagrams` — peer drops every 3rd sample once; the app
  retransmits directly via `ReliableSender`/ACKNACK; all 12 samples
  delivered gap-free in order.
- `endpoints/swift/example_reliable.swift` in the `swift` column of `tools/endpoints-gate`
  (job `integration-bindings-swift`) — `example_reliable.swift` runs and reports
  `sequence 0..11 verified in order`.

**Status:** done — measured on macOS and on the Linux test host
(swiftc 6.3.3, x86_64-unknown-linux-gnu).

## §6 Latency — ring enqueue vs. inline `sendto`

**Spec:** §5.3 — the producer path of `AsyncWriter` (`write` → `NSLock`
ring-buffer push) must be measurably below the inline `sendto` syscall — the
evidence that async write removes syscall latency from the producer path,
not that it waits for ACKNACK.

**Repo:** `endpoints/swift/Reliable.swift::AsyncWriter`; bench harness in
`crates/endpoint-e2e/tests/swift_reliable.rs::SWIFT_RELIABLE_MAIN`
(`modeBench`) — 4000 iterations of inline `sendto` (UDP) vs. 4000 iterations
of `AsyncWriter.write` after 100 warmup iterations, no live peer needed.

**Tests:** (local, macOS, this run) `reliable_5_4_swift_the_producer_enqueue_costs_less_than_the_inline_send`
(`crates/endpoint-e2e/tests/swift_reliable.rs`) — two measurement runs: ring
enqueue **426–560 ns** vs. inline `sendto` **3829–3871 ns** (~7–9×). The
test asserts `decoupled_ns < inline_ns` (no fixed factor, only the
inequality). The measured value fluctuates run to run (microbenchmark noise
on a developer machine); the direction (decoupled beats inline) is stable.

**Status:** done — measured on macOS and on the Linux test host
(swiftc 6.3.3, x86_64-unknown-linux-gnu).

---

## Audit status

6 done / 0 partial / 0 open / 0 blocked / 0 n/a (informative).

Test run: in CI, crate tier and pillar jobs; the cited tests with the latest result of each job in the pipelines 8501 to 9053 of `main` (`python3 ci/coverage_results.py --document zerodds-endpoint-swift-1.0.md`):

* `python3 ci/run_integration.py integration-bindings-swift` (job `integration-bindings-swift`) — 20 tests green.

20 cited tests green, 0 not green.

