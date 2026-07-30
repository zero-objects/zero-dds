# `zerodds-endpoint-c` 1.0 — Spec-Coverage

**Source:** `docs/specs/zerodds-endpoint-c-1.0.md` -- Native C Endpoint SDK
spec (XRCE framing, sync/async, reliable stream). Reliable-stream contract
detail in `docs/spec-coverage/reliable-endpoint-1.0.en.md`.

Covers the C **ENDPOINT** stack (distinct from the C **codegen** spec
`docs/spec-coverage/zerodds-xcdr2-c-1.0.md`): XRCE framing, sync send/recv,
async reader/writer (incl. the live ping-pong E2E duty), and the reliable
stream.

Implementation:

- `endpoints/c/include/zerodds_endpoint.h` / `src/zerodds_endpoint.c` — frame
  hook, XRCE framing, serial HDLC framing, reliable state machine (C89).
- `endpoints/c/include/zerodds_async.h` / `src/zerodds_async.c` — async
  reactor (C11), `zdw_async_reader` / `zdw_async_writer`.
- `endpoints/c/include/zerodds_reliable_async.h` / `src/zerodds_reliable_async.c`
  — SPSC ring + pthread drain thread for the decoupled reliable writer (C11).
- `crates/endpoint-e2e/tests/c_reliable.rs` — live-UDP E2E against the Rust
  reference peer.

## §1 XRCE framing

### §1.1 WRITE_DATA frame (session/stream/sequence + sample body)

**Spec:** §1.1 (`zerodds-endpoint-c-1.0.en.md`, DDS-XRCE 1.0 §8.3.2.3/§8.3.4)
— message header (`session_id`, `stream_id`, `sequence_nr` LE) + WRITE_DATA
submessage header (id=7, flags, length LE) + XCDR sample body. Best-effort,
no ClientKey (`session_id ≥ 128`).

**Repo:** `endpoints/c/include/zerodds_endpoint.h::zdw_xrce_write_frame` /
`zdw_xrce_read_frame`; `ZDW_XRCE_SESSION_NOKEY` / `ZDW_XRCE_STREAM_BEST_EFFORT`.

**Tests:** `endpoints/c/test/test_xrce_frame.c` — builds the frame, compares it
byte-exact against `golden_xrce_le.bin` (a real `crates/xrce` message), then
runs it through the frame hook (encode → frame → transport → receive → unwrap
→ decode). Reproduced locally: `XRCE WRITE_DATA frame 48 bytes byte-identical
to crates/xrce`, `frame-hook round-trip: unwrapped + decoded ok`, `ALL OK`.

**Status:** done

### §1.2 Receive path (DATA message from the agent)

**Spec:** §1.2 (`zerodds-endpoint-c-1.0.en.md`, DDS-XRCE 1.0 §8.3.4) — DATA
submessage (id=9), pushed by the agent to the client.

**Repo:** `zdw_xrce_read_frame` (shared unwrap function for WRITE_DATA and
DATA).

**Tests:** `endpoints/c/test/test_receive.c` against `golden_data_le.bin` (a
real `zerodds-xrce` DATA message). Reproduced locally: `received DATA frame,
40-byte sample body`, `agent DATA decoded: id=0xA1B2C3D4 label=bay-12`,
`ALL OK`.

**Status:** done

### §1.3 Serial HDLC framing (Annex C, RFC 1662)

**Spec:** §1.3 (`zerodds-endpoint-c-1.0.en.md`, DDS-XRCE 1.0 Annex C) —
`7E [byte-stuffed(payload) byte-stuffed(crc16-BE)] 7E`; stuffing of
`0x7E`/`0x7D`; CRC-16-CCITT-FALSE (init `0xFFFF`, poly `0x1021`) over the raw
payload.

**Repo:** `zdw_crc16_ccitt_false`, `zdw_serial_frame`, `zdw_serial_deframe`.

**Tests:** `endpoints/c/test/test_serial_frame.c` against
`golden_serial_le.bin`. Reproduced locally: `byte-stuffing: 0x7E->7D5E,
0x7D->7D5D ok`, `XRCE serial frame 52 bytes byte-identical to crates/xrce`,
`serial round-trip: deframe + crc + unwrap + decode ok`, `ALL OK`.

**Status:** done

## §2 sync send/recv

### §2.1 Frame-hook contract (`zdw_transport`)

**Spec:** §2.1 (`zerodds-endpoint-c-1.0.en.md`, ADR 0013 invariant 5) — the
endpoint is transport-opaque: it hands a fully-framed, encoded message to the
transport the integrator fills (`ctx` + `deliver`/`receive` function
pointers) and receives complete frames back.

**Repo:** `endpoints/c/include/zerodds_endpoint.h::zdw_transport`,
`zdw_endpoint_send`, `zdw_endpoint_recv`.

**Tests:** `endpoints/c/test/test_endpoint_loopback.c` — in-memory loopback
transport, encode → send → receive → decode, field comparison. Reproduced
locally: `frame-hook: 40-byte frame delivered + received + decoded ok`,
`ALL OK`.

**Status:** done

### §2.2 Poll-loop idiom (C89)

**Spec:** §2.2 (`zerodds-endpoint-c-1.0.en.md`) — sync is the C89 baseline
usage of the frame hook: the integrator owns the run loop and calls
`zdw_endpoint_recv` itself.

**Repo:** `endpoints/c/examples/example_sync.c` (C89 poll loop over the
transport).

**Tests:** `make -C endpoints/c examples` (target `example_sync`); prints five
decoded `Reading` samples + `ALL OK`. CI job `endpoints-native`.

**Status:** done

## §3 async reader/writer

### §3.1 Event-driven reactor (`zdw_async_reader` / `zdw_async_writer`)

**Spec:** §3.1 (`zerodds-endpoint-c-1.0.en.md`, ADR 0013) — additive to the
C89 core, a latency-decoupled, callback-driven alternative to the poll loop
(C11, no malloc).

**Repo:** `endpoints/c/include/zerodds_async.h` / `src/zerodds_async.c` —
`zdw_async_reader_init` binds an `on_sample` callback,
`zdw_async_run` drains the transport and dispatches each decoded sample;
`zdw_async_writer_init` + `zdw_async_write`.

**Tests:** `endpoints/c/test/test_async_loopback.c` — in-memory FIFO
transport, 5 samples written, the reactor dispatches all 5 in order.
Reproduced locally: `async loopback: 5 samples dispatched + decoded in order`,
`ALL OK`.

**Status:** done

### §3.2 Live-UDP reactor

**Spec:** §3.2 (`zerodds-endpoint-c-1.0.en.md`) — proof that the reactor
drives real datagram I/O, not just an in-memory queue.

**Repo:** the same reactor functions over a non-blocking POSIX UDP socket
(`endpoints/c/test/test_async_udp.c`).

**Tests:** `endpoints/c/test/test_async_udp.c` — 5 samples over UDP loopback.
Reproduced locally: `async UDP: 5/5 samples received + decoded via reactor`,
`ALL OK`.

**Status:** done

### §3.3 Async deep example

**Spec:** §3.3 (`zerodds-endpoint-c-1.0.en.md`) — a runnable example, not a
stub.

**Repo:** `endpoints/c/examples/example_async.c` (C11 reactor + callback).

**Tests:** `make -C endpoints/c examples` (target `example_async`); five
decoded readings + `ALL OK`. CI job `endpoints-native`; codepit-verified (5/5
field decode, byte-identical via `zdw`, see
`docs/spec-coverage/zerodds-xcdr2-c-1.0.md` §8).

**Status:** done

### §3.4 Test duty: live ping-pong E2E

**Spec:** §3.4 (`zerodds-endpoint-c-1.0.en.md`) — in addition to the isolated
unit/loopback tests from §1–§3, every language MUST provide a live E2E file
following the pattern `crates/endpoint-e2e/tests/<lang>.rs` (reference:
`cpp.rs` for C++ — raw/sync/async modes against the `idl-cpp`-generated
`Ping`/`Pong` codec), so that framing (§1), sync (§2), and async (§3) are
proven working together.

**Repo:** for **C**, `crates/endpoint-e2e/tests/` has no dedicated `c.rs`
ping-pong file (unlike `cpp`, `ada`, `d`, `nim`, `zig`, `csharp`, `go`, `java`,
`julia`, `lua`, `ocaml`, `python`, `swift` — see
`crates/endpoint-e2e/tests/`). C's endpoint capabilities (framing §1, sync §2,
async §3) are instead exercised via the C SDK's own test suite
(`endpoints/c/test/`) and, for the reliable stream, via
`crates/endpoint-e2e/tests/c_reliable.rs` (§4). A dedicated ping-pong E2E file
for C is missing.

**Tests:** —

**Status:** open — a ping-pong E2E (`c.rs` following the `cpp.rs` pattern) is
not set up; no invented test is cited here.

## §4 reliable stream

### §4.1 C89 state machine (`zdw_reliable`)

**Spec:** §4 (`zerodds-endpoint-c-1.0.en.md`) together with
`docs/spec-coverage/reliable-endpoint-1.0.en.md` §3 — reliable sender +
receiver, mirroring `crates/xrce::ReliableStreamState`; fixed storage, no
malloc: `ZDW_REL_WINDOW`=16, `ZDW_REL_RECV_BUF`=64, RFC-1982 16-bit sequence
numbers.

**Repo:** `endpoints/c/include/zerodds_endpoint.h` (declaration) /
`src/zerodds_endpoint.c` (implementation, C89) — `zdw_reliable_submit`,
`zdw_reliable_pending_heartbeat`, `zdw_reliable_recv_acknack`,
`zdw_reliable_get_in_flight` (sender); `zdw_reliable_recv_data`,
`zdw_reliable_drain`, `zdw_reliable_pending_acknack` (receiver);
`zdw_reliable_reset`.

**Tests:** `endpoints/c/test/test_reliable_sm.c` — 14 check functions: 13
mirror the `crates/xrce::reliable` reference tests (monotonic `seq`,
window-full, heartbeat first/silence/again, acknack-clear/full-clear,
in-order drain, reorder, duplicate-drop, buffer-full, pending-acknack bitmap,
reset, end-to-end loss-recovery in-process) plus 1 byte-golden check.
Reproduced locally (`make build/test_reliable_sm && ./build/test_reliable_sm
golden_heartbeat_le.bin golden_acknack_le.bin`): `test_reliable_sm: ALL OK`.

**Status:** done

### §4.2 Byte-golden HEARTBEAT/ACKNACK

**Spec:** §4 (`zerodds-endpoint-c-1.0.en.md`) together with
`docs/spec-coverage/reliable-endpoint-1.0.en.md` §4 — `AckNack{first_unacked_seq_num
i16, nack_bitmap [u8;2] LE, stream_id u8}`, `Heartbeat{first_unacked_seq_nr i16,
last_unacked_seq_nr i16, stream_id u8}`.

**Repo:** `t_byte_golden` in `test_reliable_sm.c` parses
`golden_heartbeat_le.bin` / `golden_acknack_le.bin`, rebuilds them, compares
byte-exact; plus `endpoints/c/test/test_reliable.c` (pure wire test, no state
machine).

**Tests:** same as §4.1 plus `endpoints/c/test/test_reliable.c`. Reproduced
locally: `HEARTBEAT parsed: first=1 last=3 stream=0x80`, `ACKNACK 13 bytes
byte-identical to crates/xrce`, `ALL OK`.

**Status:** done

### §4.3 pthread SPSC-ring async writer (latency decoupling)

**Spec:** §4 (`zerodds-endpoint-c-1.0.en.md`) together with
`docs/spec-coverage/reliable-endpoint-1.0.en.md` §2 — the producer must never
enter the kernel; the component simultaneously carries the reliable sender
state.

**Repo:** `endpoints/c/include/zerodds_reliable_async.h` /
`src/zerodds_reliable_async.c` (C11 + pthread) — `zdw_async_ring_start` starts
the drain thread, `zdw_async_ring_enqueue` is a wait-free enqueue (no
syscall), the drain thread holds `zdw_reliable rel` and performs framing +
`sendmmsg`-style I/O.

**Tests:** `endpoints/c/test/bench_reliable_async.c` (latency comparison, see
§4.6) and `endpoints/c/examples/example_reliable.c` (loss-recovery demo,
in-process, no network).

**Status:** done

### §4.4 Loss-recovery example

**Spec:** §4 (`zerodds-endpoint-c-1.0.en.md`) together with
`docs/spec-coverage/reliable-endpoint-1.0.en.md` §5 item 5 — a runnable
example, not a stub; N samples, injected loss, gap-free delivery after
retransmit.

**Repo:** `endpoints/c/examples/example_reliable.c` — 12 samples submitted,
every 4th dropped on the first pass, recovery rounds until all 12 arrive in
sequence 0..11.

**Tests:** `make -C endpoints/c examples` (target `example_reliable`).
Reproduced locally: `reliable: 3/12 delivered before recovery (3 lost)`,
`reliable: delivered contiguous 0..11, expected=12`, `reliable: ALL 12 samples
recovered gap-free`.

**Status:** done

### §4.5 Live E2E against the Rust reference peer

**Spec:** §4 (`zerodds-endpoint-c-1.0.en.md`) together with
`docs/spec-coverage/reliable-endpoint-1.0.en.md` §5 item 3 — live against the
Rust reliable peer (`zerodds-endpoint-e2e`), `stream_id ≥ 128`, with injected
drop; assert all samples delivered gap-free in order despite loss.

**Repo:** `endpoints/c/test/reliable_udp_app.c` — the C app plays the
reliable **sender** (submit + WRITE_DATA + HEARTBEAT + ACKNACK-driven
retransmit) against the shared Rust `ReliablePeer`, which injects loss.

**Tests:** `crates/endpoint-e2e/tests/c_reliable.rs::c_reliable_loss_recovery`
(12 samples, injected drop, assert gap-free) and `::c_reliable_no_loss` (12
samples without drop). On codepit (Linux) per prior verification: `cargo test
--test c_reliable` → 3 passed. **Reproduced locally on this host
(macOS/Apple Clang): the build fails** — `endpoints/c/test/reliable_udp_app.c`
defines `_POSIX_C_SOURCE 200809L`, which under Apple's libc hides
`INADDR_LOOPBACK` (a BSD socket extension, not a POSIX constant) (`error: use
of undeclared identifier 'INADDR_LOOPBACK'`); Linux/glibc does not impose this
restriction. A host-specific build difference, not a test run verified on
this host — the file was not modified (out of scope for this document).

**Status:** done (codepit/Linux, per prior verification) — **not buildable on
macOS locally** (finding above); not independently re-run on Linux as part of
writing this document.

### §4.6 Latency: producer latency inline send vs. decoupled ring enqueue

**Spec:** §4 (`zerodds-endpoint-c-1.0.en.md`) together with
`docs/spec-coverage/reliable-endpoint-1.0.en.md` §5 item 4 — producer
`write→return` in the async-decoupled path vs. inline deliver; a measured
figure showing the decoupling.

**Repo:** `endpoints/c/test/bench_reliable_async.c` — measures
`inline_ns_per_op` (frame + `send()` syscall per sample, against a
bound-but-unread loopback socket) against `decoupled_enqueue_ns_per_op` (pure
`zdw_async_ring_enqueue`, no kernel).

**Tests:** `endpoints/c/test/bench_reliable_async.c` (target
`bench_reliable_async`) and
`crates/endpoint-e2e/tests/c_reliable.rs::c_reliable_latency_bench`. Per the
task brief/prior verification (codepit): inline ~3.5 µs vs. decoupled ring
enqueue ~5–7 ns (~600x). **On this host (macOS)**: the build fails with the
same `INADDR_LOOPBACK` root cause as §4.5 (`bench_reliable_async.c` uses
`-std=c11 -pedantic` without `_DEFAULT_SOURCE`, unlike `test_async_udp.c`,
which sets `_DEFAULT_SOURCE 1`) — no local measurement taken.

**Status:** done (codepit/Linux, per prior verification) — **not buildable on
macOS locally**, root cause identical to §4.5.

---

## Audit-Status

15 done / 0 partial / 1 open / 0 n/a.

Test run (this host, macOS/Apple Clang, `cc` = clang):
`make -C endpoints/c build/test_xrce_frame build/test_receive
build/test_endpoint_loopback build/test_reliable_sm build/test_reliable
build/example_reliable build/test_async_loopback build/test_async_udp
build/test_serial_frame GOLDEN_DIR=<goldens from zerodds-endpoint-golden>` →
all 9 binaries build cleanly (`-std=c89 -pedantic -Wall -Wextra` resp.
`-std=c11 -pedantic -Wall -Wextra`, 0 warnings) and run `ALL OK`
(`example_reliable` does not print an `ALL OK` line but reports `ALL 12
samples recovered gap-free` with exit 0). `endpoints/c/test/bench_reliable_async.c`
and `endpoints/c/test/reliable_udp_app.c` (the latter only via `cargo test -p
zerodds-endpoint-e2e --test c_reliable`) do **not** build on this host —
`INADDR_LOOPBACK` is not visible under Apple's libc (§4.5/§4.6 finding). The
results cited for §4.5/§4.6 (3 passed; ~3.5 µs vs. ~5–7 ns) come from prior
codepit (Linux) verification, not from a Linux run performed within this
session.

Open items:
- §3.4 live ping-pong E2E for C (`crates/endpoint-e2e/tests/c.rs` following the
  `cpp.rs` pattern) is not set up.
- §4.5/§4.6 not buildable/reproducible on this host (macOS); only the prior
  codepit (Linux) evidence is cited, no fresh Linux verification within this
  session.
