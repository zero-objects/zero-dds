# `zerodds-endpoint-nim` 1.0 — Spec-Coverage

**Source:** `docs/specs/zerodds-endpoint-nim-1.0.md` — the ZeroDDS Nim
endpoint SDK spec (XRCE framing, sync `Client`, async `AsyncReader`, reliable
stream).

**Implementation:**

- `endpoints/nim/zerodds.nim` — `Writer`/`Reader` (XCDR2, alignment cap 4),
  `writeFrame`/`readFrame` (XRCE best-effort stream 0x01), `Client` (sync),
  `AsyncReader` (async).
- `endpoints/nim/reliable.nim` — sender/receiver state machine + HEARTBEAT/
  ACKNACK frame codec (reliable stream 0x80).
- `endpoints/nim/reliable_app.nim` — Channel-based drain thread (producer
  enqueues, a drain thread owns the socket + sender state).
- `endpoints/nim/example_reliable.nim` — in-process demo of loss recovery.
- `crates/endpoint-e2e/tests/nim.rs`, `crates/endpoint-e2e/tests/nim_reliable.rs`
  — live E2E against the Rust XRCE peer.

## §1 XRCE framing

**Spec:** §1 — XRCE `WRITE_DATA` frame: an 8-byte header (session, stream, seq
LE, submessage id, flags, length LE) + payload, byte-identical to
`crates/xrce` and every other endpoint SDK.

**Repo:** `endpoints/nim/zerodds.nim::writeFrame`/`readFrame` (best-effort,
session `0x80`, stream `0x01`); `endpoints/nim/reliable.nim::reliableWriteFrame`
(reliable, stream `0x80`) — identical byte layout, different stream id.

**Tests:** `endpoints/nim/test.nim` (byte identity of the wire core, CI job
`endpoints-nim`); live frame round-trip in §2/§3/§4.

**Status:** done.

## §2 Sync Client

**Spec:** §2 — A non-blocking poll client: `write` frames+sends, `poll` reads
non-blocking and returns `Option[seq[byte]]`.

**Repo:** `endpoints/nim/zerodds.nim::Client`/`newClient`/`write`/`poll`
(monotonic `seqNo`, wraps at `0xffff`).

**Tests:** `nim_endpoint_sync` (`crates/endpoint-e2e/tests/nim.rs`) — a live
poll loop against the Rust peer: an app built from generated `idlc` types
(`gen.nim`) plus the endpoint SDK exchanges a typed sample with the shared
Rust XRCE peer over a real UDP socket
(`crates/endpoint-e2e/tests/nim.rs::build_nim_endpoint` compiles `gen.nim` + a
copy of `zerodds.nim` + `main.nim`, spawns the app).

**Status:** done.

## §3 Async Reader

**Spec:** §3 — The idiomatic Nim `async`/`await` path: `recv` returns a
`Future[seq[byte]]` that resolves only once the deframed sample is ready.

**Repo:** `endpoints/nim/zerodds.nim::AsyncReader`/`newAsyncReader`/`recv`
(an `{.async.}` proc, polls via `sleepAsync(1)` until a frame is pending).

**Tests:** `nim_endpoint_async` (`crates/endpoint-e2e/tests/nim.rs`) —
`waitFor fut.withTimeout(10000)` against the same generated app; `Client.write`
sends in both modes (the SDK only has a sync writer), the async path exercises
purely the `AsyncReader` `Future`.

**Tests (codepit):** `nim_endpoint_sync`, `nim_endpoint_async` — both green.

**Status:** done.

## §4 Reliable stream

**Spec:** §4 — XRCE reliable stream (`stream_id >= 128`, §8.4.10/§8.4.11),
mirrored from the reference `crates/xrce/src/reliable.rs`: sender `submit`/
`pendingHeartbeat`/`recvAcknack`/`getInFlight`; receiver `recvData`/
`drainInOrder`/`pendingAcknack`/`reset`. Window 16, receiver buffer 64,
heartbeat 500 ms, payload <= 65535, RFC-1982 16-bit sequence numbers. The
async writer is built as a Channel-drain thread: the producer enqueues
lock-based (Nim stdlib `Channel`, not a wait-free ring), a dedicated drain
thread owns the socket and the `Sender` state (sends WRITE_DATA, periodic
HEARTBEAT, receives ACKNACK, retransmits from `inFlight`).

**Repo:** `endpoints/nim/reliable.nim` (state machine + frame codec);
`endpoints/nim/reliable_app.nim::drain` (drain thread over `gChan.tryRecv`);
`endpoints/nim/reliable_test.nim` (unit + byte-golden);
`endpoints/nim/example_reliable.nim` (in-process demo, no socket).

**Tests (codepit):**

- `nim_reliable_loss_recovery` (`crates/endpoint-e2e/tests/nim_reliable.rs`) —
  12 samples, the peer drops every 3rd datagram; the app retransmits on
  ACKNACK; all 12 delivered gap-free in order.
- `nim_reliable_no_loss` — the same run without injected loss (baseline).
- `nim_reliable_unit_and_golden` — compiles and runs `reliable_test.nim`;
  22 mirrored state-machine checks (sender: monotonic seq, payload-too-large,
  window-full, heartbeat first/silence/after-period/empty, ACKNACK clear
  partial/full; receiver: in-order, reorder block+delivery, duplicate-drop,
  buffer-full, ACKNACK bitmap, reset; plus 3 end-to-end loss-recovery checks)
  + 4 byte-golden checks (`byte_golden_heartbeat`, `byte_golden_acknack`
  against `golden_heartbeat_le.bin`/`golden_acknack_le.bin`, plus a parse
  round-trip of both goldens) — `ALL OK`.
- `nim_reliable_producer_latency` — the bench mode of `reliable_app.nim`:
  producer `enqueue`->return via Channel vs. inline `sendto` on the same
  thread (measurement in §5).

**Status:** done.

**Honestly noted:** Nim's stdlib `Channel` is lock-based (mutex + copy), not a
wait-free SPSC ring; the decoupling from the send syscall is real, a true ring
would push the enqueue figure lower still (comment in `reliable_app.nim`). The
reference spec (§4) does not mandate the concrete queue implementation.

## §5 Latency

**Spec:** §4 — A measurement that shows the producer decoupling from the
socket syscall (`reliable-endpoint` §5 item 4, latency bench).

**Repo:** `endpoints/nim/reliable_app.nim::runBench` — 20000 iterations per
path; `inline`: `sock.send` directly on the producer thread; `decoupled`:
`gChan.send` into a Channel drained by a separate thread.

**Tests (codepit):** `nim_reliable_producer_latency` — enqueue **192 ns** vs.
inline `sendto` **3926 ns** (~20x).

**Status:** done (measurement present); further reduction via a wait-free
ring remains open (see §4, honestly noted — not a separate tracking item,
since it is not a spec requirement).

---

## Audit-Status

6 done / 0 partial / 0 open / 0 n/a.

Test-run (codepit, verified): `cargo test -p zerodds-endpoint-e2e --test
nim` -> `nim_endpoint_sync` + `nim_endpoint_async` 2/2 green; `--test
nim_reliable` -> `nim_reliable_loss_recovery`, `nim_reliable_no_loss`,
`nim_reliable_unit_and_golden`, `nim_reliable_producer_latency` 4/4 green;
latency measurement 192 ns (enqueue) vs. 3926 ns (inline sendto), ~20x. No
GitLab CI job runs `zerodds-endpoint-e2e` — these are manual codepit runs,
not wired into CI (the `endpoints-nim` job covers only the wire core/examples,
not reliable/E2E).

Open items: the `zerodds-endpoint-e2e` tests (ping-pong + reliable) are not
wired into GitLab CI — they run manually on codepit only. A true wait-free
SPSC ring instead of the lock-based `Channel` remains a possible further
latency reduction (not an open spec item, see §4).
