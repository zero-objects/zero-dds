# `zerodds-endpoint-cpp` 1.0 — Spec Coverage

**Source:** `docs/specs/zerodds-endpoint-cpp-1.0.md` (226 lines) — the
normative C++17 endpoint SDK spec (XRCE framing, sync client, async
Reader/Writer, reliable stream). Implemented in `endpoints/cpp/`: XRCE
framing + the sync client over the C89 wire core `zdw` (`endpoints/c`),
async Reader/Writer as a thin C++17 facade (`zerodds_async.hpp`), and the
header-only reliable stream (`zerodds_reliable.hpp`). Complements the
codegen coverage `zerodds-xcdr2-cpp` (`zerodds-xcdr2-cpp-1.0.en.md`) — that
doc covers the generated `topic_type_support<T>` codec, this one the
endpoint that transports it.

## §1 XRCE framing

**Spec:** `zerodds-endpoint-cpp-1.0.en.md` §1 -- DDS-XRCE 1.0 message header
(8 bytes: session, stream, seq LE, submsg id, flags, len LE) + body;
best-effort stream `0x01`.

**Repo:** `endpoints/c/include/zerodds_endpoint.h` — `zdw_xrce_write_frame`
/ `zdw_xrce_read_frame`, `ZDW_XRCE_SESSION_NOKEY`,
`ZDW_XRCE_STREAM_BEST_EFFORT`. The C++ SDK does not re-implement the
framing — it binds the C89 wire core directly.

**Tests:** `crates/endpoint-e2e/tests/cpp.rs::run_sync` / `run_async` frame
and deframe every ping/pong over real UDP.

**Status:** done.

## §2 Sync client

**Spec:** `zerodds-endpoint-cpp-1.0.en.md` §2 -- A blocking transport-poll
client: `zdw_endpoint_send` / `zdw_endpoint_recv` over an
integrator-supplied `zdw_transport` (`deliver`/`receive` callbacks, ADR-0013
frame hook), marshalling via the C++98 facade `zerodds::Writer` /
`zerodds::Reader`.

**Repo:** `endpoints/c/include/zerodds_endpoint.h` (`zdw_transport`,
`zdw_endpoint_send`/`recv`); `endpoints/cpp/include/zerodds_wire.hpp`
(`zerodds::Writer`/`Reader`, a C++98 facade over the C89 core); C++ call
site `crates/endpoint-e2e/tests/cpp.rs::run_sync` (UDP callbacks
`udp_deliver` / `udp_receive`, 100 ms poll loop for up to 100 iterations);
`endpoints/cpp/example_sync.cpp` (sensor-telemetry deep example
`Reading{id,value,label}`, C++98 poll loop over an in-memory FIFO, full
field decode).

**Tests (codepit, g++/gcc gated):** `cpp_endpoint_sync` — live ping-pong E2E
against the shared Rust XRCE peer over real UDP; plus `cpp_raw_udp` (bare
XCDR2, no XRCE framing, a direct codec call through the generated types
`topic_type_support<Ping>`/`<Pong>` from `idl-cpp`) as codec build
integration for the same app compilation. 2/2 passed. Loud skip only when
no compiler is on `PATH`.

**Status:** done.

## §3 Async Reader/Writer

**Spec:** `zerodds-endpoint-cpp-1.0.en.md` §3 -- An event-driven,
non-blocking reader (a callback per received sample) and a fire-and-forget
writer, both thin C++17 facades over the audited C reactor
(`zerodds_async.c`) — additive to the conservative C++98 wire facade
(`zerodds_wire.hpp`).

**Repo:** `endpoints/cpp/include/zerodds_async.hpp` — `zerodds::AsyncReader`
(`poll()` / `run(max)`, RAII, `std::function` trampoline over
`zdw_async_reader_init`) and `zerodds::AsyncWriter` (`write()` over
`zdw_async_writer_init` / `zdw_async_write`); `endpoints/cpp/example_async.cpp`
(sensor-telemetry deep example, C++17 reactor).

**Tests (codepit, g++/gcc gated):** `crates/endpoint-e2e/tests/cpp.rs::run_async`
— `AsyncReader` dispatches the received pong to a lambda, `AsyncWriter`
sends the ping; live ping-pong E2E against the Rust peer over real UDP;
test name `cpp_endpoint_async`. 1/1 passed. Loud skip only when no compiler
is on `PATH`.

**Status:** done.

## §4 Reliable stream — state machine, async writer, loss recovery, byte-golden, latency

**Spec:** `zerodds-endpoint-cpp-1.0.en.md` §4 (normative:
`reliable-endpoint-1.0.en.md`) -- DDS-XRCE reliable stream (`stream_id ≥
128`, §8.4.10/§8.4.11), mirroring the reference `crates/xrce/src/reliable.rs`:
sender `submit`/`pending_heartbeat`/`recv_acknack`/`get_in_flight`; receiver
`recv_data`/`drain_in_order`/`pending_acknack`/`reset`. The async writer
decouples the producer from I/O: `enqueue()` is wait-free into a lock-free
SPSC ring; a dedicated drain thread owns the `Sender` state, batches
`WRITE_DATA` via `sendmmsg`, fires HEARTBEATs periodically, and retransmits
on ACKNACK. The producer must never enter the kernel on the
async-decoupled path — a measurement should show the decoupling against the
inline-`sendto` path (design rationale from `reliable-endpoint-1.0.en.md`
§2).

**Repo:** `endpoints/cpp/include/zerodds_reliable.hpp` (381 lines,
header-only C++17) — `Sender`/`Receiver` state machine, wire codec
(`write_frame`/`unframe`/`acknack_frame`/`heartbeat_frame`/`parse_acknack`/
`parse_heartbeat`), `AsyncWriter` (SPSC ring with `std::atomic<size_t>
head_`/`tail_`, `std::thread` drain, `enqueue`/`finish`/`stop`). Pure C++17,
header-only; no linking against the Rust layer, cross-compile-safe.
`endpoints/cpp/example_reliable.cpp` (in-process demo: sender + receiver,
every 3rd sample dropped in the first round, ACKNACK-driven recovery
rounds). `endpoints/cpp/test/test_reliable_cpp.cpp` (12 unit checks +
byte-golden + latency bench).

**Tests (codepit, g++12):**

- `cpp_reliable_unit_and_golden` — 12 unit checks (sender: monotonic seq,
  payload-too-large, window-full, heartbeat first/silence/period; receiver:
  acknack-clear, acknack-full-clear, in-order, reorder, duplicate-drop,
  buffer-full, pending-acknack bitmap, reset) + byte-golden
  (`acknack_frame(0x80, NONE, 1, 1, 0, 0, 0x80)` == `golden_acknack_le.bin`,
  `heartbeat_frame(0x80, NONE, 1, 1, 3, 0x80)` == `golden_heartbeat_le.bin`,
  13 bytes each, byte-identical to the Rust-generated goldens) + a latency
  bench (median over K=20000 iterations: `AsyncWriter::enqueue`, no-op drain
  hooks, no syscall, vs `write_frame` + inline `send` to a bound loopback
  sink) → `ALL OK`.
- `cpp_reliable_loss_recovery` — the Rust peer drops every 3rd datagram; the
  app retransmits on ACKNACK; 12/12 samples delivered gap-free in order.
- `cpp_reliable_lossless_baseline` — lossless; 12/12.
- `cpp_reliable_example` — in-process demo → `RELIABLE OK N=16`.

4/4 passed. Latency measured: 31–40 ns (`enqueue`) vs 3427 ns (inline
`sendto`), ~85–110× depending on the run (run-to-run variance, not a single
value).

Finding during development: a drain deadlock with no responder — `finish()`
waits until both window and ring are empty, which never happens without an
ACKNACK reply. Fixed by adding `stop()` (unconditional teardown, for
responder-less contexts such as the latency bench).

**Status:** done — sender and receiver state machine unit-verified; the
live E2E against the Rust peer covers the sender role (app sends, Rust peer
receives + ACKNACKs); the receiver role is verified in-process
(`example_reliable.cpp`), not live-network against an external sender;
latency decoupling measured.

## §5 Test and evidence obligations

**Spec:** `zerodds-endpoint-cpp-1.0.en.md` §5 (mirrors
`reliable-endpoint-1.0.en.md` §5) -- unit, byte-golden, E2E loss recovery, a
latency bench, a runnable `example_reliable_*`; no false-green, a loud skip
only when the toolchain is missing.

**Repo:** All five required artifacts present, evidenced in §4:
`test_reliable_cpp.cpp` (unit + byte-golden + latency bench),
`crates/endpoint-e2e/tests/cpp_reliable.rs` (loss recovery + lossless
baseline against the Rust peer), `example_reliable.cpp` (a runnable
example, not a stub). Loud skip only when `g++`/`gcc` is missing from
`PATH`.

**Tests:** see §4 (`cpp_reliable_unit_and_golden`, `cpp_reliable_loss_recovery`,
`cpp_reliable_lossless_baseline`, `cpp_reliable_example`) and §2/§3
(`cpp_raw_udp`, `cpp_endpoint_sync`, `cpp_endpoint_async`).

**Status:** done.

## Audit status

5 done / 0 partial / 0 open / 0 n/a (informative) / 0 n/a (rejected).

Test run (codepit, g++12, verified): `cargo test -p zerodds-endpoint-e2e
--test cpp` → 3/3 (`cpp_raw_udp`, `cpp_endpoint_sync`, `cpp_endpoint_async`);
`--test cpp_reliable` → 4/4 (`cpp_reliable_unit_and_golden` incl.
12-checks+byte-golden+bench, `cpp_reliable_loss_recovery`,
`cpp_reliable_lossless_baseline`, `cpp_reliable_example`); latency bench
enqueue 31–40 ns / inline sendto 3427 ns.

Open items: none.
