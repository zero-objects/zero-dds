# `zerodds-endpoint-swift` 1.0 — Spec Coverage

**Source:** `docs/specs/zerodds-endpoint-swift-1.0.md` — the ZeroDDS Swift
endpoint SDK spec. Complements the codegen coverage `zerodds-xcdr2-swift`
(`docs/spec-coverage/zerodds-xcdr2-swift-1.0.md`) — that doc covers
marshalling, this one covers transport.

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
  `crates/endpoint-e2e/tests/swift_reliable.rs` — reliable-stream E2E, unit
  runner, example, latency bench.

**Toolchain note (applies to every section):** `swiftc` is not available on
`codepit` (the Linux bench host). Every test below is **verified locally on
macOS only** (this run: macOS, `swift-driver 1.148.6`, Swift 6.3.3, arm64).
The E2E harnesses gate on `swiftc_available()` and emit a loud skip when the
toolchain is missing (no false-green) — not re-verified on codepit.

## §1 XRCE framing

**Spec:** §1 — 8-byte XRCE header (session, stream, seq LE, submsg id `0x07`
WRITE_DATA, flags, len LE) + body, byte-identical to `crates/xrce` +
`endpoints/c`.

**Repo:** `endpoints/swift/Sources/Zerodds/Zerodds.swift` —
`xrceWriteFrame`/`xrceReadFrame`, constants `xrceSessionNoKey` (`0x80`) and
`xrceStreamBestEffort` (`0x01`).

**Tests:** framing is exercised live via `swift_endpoint_sync`/
`swift_endpoint_async` (§4); the wire core itself (`Writer`/`Reader`, cap-4
alignment, `f32`/`f64` via `.bitPattern`) via `ZeroddsTests.testByteIdentity`
(`endpoints/swift/Tests/ZeroddsTests/ZeroddsTests.swift`, `swift test`)
against `testdata/golden_le.bin`/`golden_be.bin` — LE **and** BE
byte-identical.

**Status:** done (local, macOS).

## §2 Sync `Client`

**Spec:** §2 — blocking `Client`: `write` frames + delivers synchronously,
`poll` is a single non-blocking receive.

**Repo:** `endpoints/swift/Sources/Zerodds/Zerodds.swift::Transport`
protocol (`deliver`/`receive`, the sole integration point); `Client`
(`init`/`write`/`poll`, monotonic `seq` counter, default session
`xrceSessionNoKey`/`xrceStreamBestEffort`); `MemTransport`, an
`NSLock`-guarded in-memory FIFO.

**Tests:** `ZeroddsTests.testSyncLoopback` (`swift test`, 5 samples over
`MemTransport`); live E2E `swift_endpoint_sync` (§4).

**Status:** done (local, macOS).

## §3 Async `AsyncReader`

**Spec:** §3 — `AsyncReader.stream()` returns an `AsyncStream<[UInt8]>`; an
internal `Task` polls the `Transport` and yields unframed sample bodies; the
consumer iterates with `for await`, `onTermination` cancels the task.

**Repo:** `endpoints/swift/Sources/Zerodds/Zerodds.swift::AsyncReader`
(`init`, `stream()`); the send side of the async path shares `Client.write`
(§2) — no separate `AsyncWriter` type outside the reliable `AsyncWriter`
(§5).

**Tests:** `ZeroddsTests.testAsyncLoopback` (`swift test`, 5 samples over
`MemTransport`, `for await`); live E2E `swift_endpoint_async` (§4).

**Status:** done (local, macOS).

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

**Tests (local, macOS, this run):**
- `swift_endpoint_sync` — full stack via `Client`.
- `swift_endpoint_async` — full stack via `AsyncReader`.

2/2 passed (`cargo test -p zerodds-endpoint-e2e --test swift`, local macOS;
not on codepit — no `swiftc` there).

**Status:** done (local, macOS; codepit not verifiable — no toolchain).

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

**Tests (local, macOS, this run):**
- `swift_reliable_unit` (`crates/endpoint-e2e/tests/swift_reliable.rs`)
  compiles `reliable_tests.swift` against the `ZeroddsReliable` module and
  runs it — 16 `run()` test scenarios: monotonic seq
  (`submit_assigns_monotonic_seqnrs`), payload-too-large
  (`submit_rejects_payload_too_large`), window-full
  (`submit_rejects_when_window_full`), heartbeat first/silence/none
  (`pending_heartbeat_fires_first_time`/
  `pending_heartbeat_silenced_until_period_elapsed`/
  `pending_heartbeat_none_when_window_empty`), ACKNACK partial/full clear
  (`recv_acknack_clears_acked_seqnrs`/
  `recv_acknack_full_clear_when_no_bits_set`), receiver in-order/reorder/
  dedup/buffer-full (`recv_data_buffers_in_order`/
  `recv_data_reorders_out_of_order`/`recv_data_drops_duplicates`/
  `recv_data_rejects_when_buffer_full`), pending-ACKNACK bitmap
  (`pending_acknack_marks_missing_slots`), reset
  (`reset_clears_state_completely`), byte-golden
  (`byte_golden_heartbeat_acknack`: `heartbeatFrame(1,3,0x80)` ==
  `80 00 01 00 0B 01 05 00 01 00 03 00 80`, `acknackFrame(1,0,0x80)` ==
  `80 00 01 00 0A 01 05 00 01 00 00 00 80` — identical to the reference
  goldens), in-process end-to-end loss recovery
  (`end_to_end_sender_receiver_with_loss_recovery`).
- `swift_reliable_loss_recovery` — peer drops every 3rd sample once; the app
  retransmits directly via `ReliableSender`/ACKNACK; all 12 samples
  delivered gap-free in order.
- `swift_reliable_no_loss` — lossless baseline; 12/12.
- `swift_reliable_example` — `example_reliable.swift` runs and reports
  `sequence 0..11 verified in order`.

5/5 passed (`cargo test -p zerodds-endpoint-e2e --test swift_reliable`,
local macOS), 4 of which belong to this section (latency bench in §6). Not
on codepit — no `swiftc` there.

**Status:** done (local, macOS; codepit not verifiable — no toolchain).

## §6 Latency — ring enqueue vs. inline `sendto`

**Spec:** §5.3 — the producer path of `AsyncWriter` (`write` → `NSLock`
ring-buffer push) must be measurably below the inline `sendto` syscall — the
evidence that async write removes syscall latency from the producer path,
not that it waits for ACKNACK.

**Repo:** `endpoints/swift/Reliable.swift::AsyncWriter`; bench harness in
`crates/endpoint-e2e/tests/swift_reliable.rs::SWIFT_RELIABLE_MAIN`
(`modeBench`) — 4000 iterations of inline `sendto` (UDP) vs. 4000 iterations
of `AsyncWriter.write` after 100 warmup iterations, no live peer needed.

**Tests (local, macOS, this run):** `swift_reliable_latency_bench`
(`crates/endpoint-e2e/tests/swift_reliable.rs`) — two measurement runs: ring
enqueue **426–560 ns** vs. inline `sendto` **3829–3871 ns** (~7–9×). The
test asserts `decoupled_ns < inline_ns` (no fixed factor, only the
inequality). The measured value fluctuates run to run (microbenchmark noise
on a developer machine); the direction (decoupled beats inline) is stable.

**Status:** done (local, macOS; codepit not verifiable — no toolchain).

---

## Audit status

5 done / 0 partial / 0 open / 0 n/a (informative) / 0 n/a (rejected).

Test run (local macOS, verified this session):
`cargo test -p zerodds-endpoint-e2e --test swift` 2/2
(`swift_endpoint_sync`/`swift_endpoint_async`); `--test swift_reliable` 5/5
(`swift_reliable_unit` — 16 Swift unit-test scenarios incl. byte-golden,
`swift_reliable_loss_recovery`, `swift_reliable_no_loss`,
`swift_reliable_example`, `swift_reliable_latency_bench`); additionally
`swift test` (SwiftPM XCTest) 3/3 (`testByteIdentity`, `testSyncLoopback`,
`testAsyncLoopback`). Latency bench ring enqueue 426–560 ns / inline
`sendto` 3829–3871 ns (~7–9×, two measurement runs).

Open items: no functional gap. Measured: every test runs locally on macOS
only — `codepit` has no `swiftc`, so there is no Linux/CI evidence for this
SDK (unlike Go/Zig/Nim, which run on codepit). That is a toolchain
boundary, not a spec gap.
