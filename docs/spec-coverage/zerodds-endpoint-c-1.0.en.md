# `zerodds-endpoint-c` 1.0 — Spec Coverage

**Source:** `docs/specs/zerodds-endpoint-c-1.0.md` — the native C endpoint SDK
(XRCE framing, sync/async, reliable stream). The reliable-stream contract
details live in `docs/spec-coverage/reliable-endpoint-1.0.en.md`.

Covers the C **ENDPOINT** stack (separate from the C **codegen** spec
`matrix/zerodds-c/`): XRCE framing, sync send/recv, async reader/writer
(including the live ping-pong E2E obligation) and the reliable stream.

Implementation:

- `endpoints/c/include/zerodds_endpoint.h` / `endpoints/c/src/zerodds_endpoint.c`
  — frame hook, XRCE framing, serial framing after Annex C, reliable state machine (C89).
- `endpoints/c/include/zerodds_async.h` / `endpoints/c/src/zerodds_async.c` —
  async reactor (C11).
- `endpoints/c/include/zerodds_reliable_async.h` /
  `endpoints/c/src/zerodds_reliable_async.c` — SPSC ring plus a pthread drain
  thread for the decoupled reliable writer (C11).
- `crates/endpoint-e2e/tests/c.rs` and `crates/endpoint-e2e/tests/c_reliable.rs`
  — live UDP E2E against the Rust reference peer.

## How the evidence runs

Two separate runs, both executed in this audit:

1. **C side.** The files under `endpoints/c/test/` and
   `endpoints/c/examples/` are C programs. They are **not** part of
   `cargo test`; a pure Rust run executes none of them. They are built and
   run through `endpoints/c/Makefile`:
   `make -C endpoints/c test GOLDEN_DIR=<dir>` and
   `make -C endpoints/c examples`, where `<dir>` holds the byte goldens from
   `cargo run -p zerodds-endpoint-golden -- <dir>`. In the pipeline the job
   `endpoints-native` in `.gitlab-ci.yml` runs exactly those two commands.
2. **Rust side.** `crates/endpoint-e2e/tests/c.rs` and
   `crates/endpoint-e2e/tests/c_reliable.rs` run in the default
   `cargo test -p zerodds-endpoint-e2e` — no feature gate. Every case first
   checks whether a C compiler is on the `PATH`, and otherwise returns with a
   `SKIP` line on stderr; the case then counts as passed without having
   checked anything. The pipeline job `endpoint-e2e` in `.gitlab-ci.yml`
   greps for those lines and fails the job when a language other than Swift
   skips. On this host `cc` and `gcc` were present; none of the six cases
   skipped.

Every item below names which of the two runs carries its evidence.

Open items: none. Decision records: none.

## §1 XRCE framing

### §1.1 WRITE_DATA frame (session/stream/sequence + sample body)

**Spec:** §1.1 (`docs/specs/zerodds-endpoint-c-1.0.md`, DDS-XRCE 1.0
§8.3.2.3/§8.3.4) — message header (`session_id`, `stream_id`, `sequence_nr`
little-endian) plus the WRITE_DATA submessage header (id = 7, flags, length
little-endian) plus the XCDR sample body. Best-effort, no ClientKey
(`session_id >= 128`).

**Repo:** `endpoints/c/include/zerodds_endpoint.h` /
`endpoints/c/src/zerodds_endpoint.c` (functions `zdw_xrce_write_frame` and
`zdw_xrce_read_frame`, constants `ZDW_XRCE_SESSION_NOKEY` and
`ZDW_XRCE_STREAM_BEST_EFFORT`).

**Tests:** C side — `endpoints/c/test/test_xrce_frame.c`, run through the
Makefile (`make -C endpoints/c test`); builds the frame and compares it byte
for byte against `golden_xrce_le.bin`, then a round trip through the frame
hook (encode, frame, transport, receive, unwrap, decode). Output measured in
this audit: XRCE WRITE_DATA frame 48 bytes byte-identical to crates/xrce.
Rust side — `crates/endpoint-e2e/tests/c.rs::c_endpoint_sync` (the Rust peer
unframes the WRITE_DATA the C program sent live over UDP and checks the 23
octets of the `Ping`). The reference the golden is generated from:
`crates/xrce/src/submessages/write_data.rs::tests::write_data_flags_are_e_le_and_format_data`,
`crates/xrce/src/submessages/write_data.rs::tests::write_data_base_object_request_precedes_the_sample`,
`crates/xrce/src/submessages/write_data.rs::tests::write_data_matches_the_annex_b_example`.

**Status:** done

### §1.2 Receive path (DATA message from the agent)

**Spec:** §1.2 (`docs/specs/zerodds-endpoint-c-1.0.md`, DDS-XRCE 1.0 §8.3.4)
— the DATA submessage (id = 9) is the agent → client direction. The endpoint
MUST accept it; WRITE_DATA in that direction would be wrong.

**Repo:** `endpoints/c/src/zerodds_endpoint.c` (`zdw_xrce_read_frame`, the
shared unwrap function for WRITE_DATA and DATA).

**Tests:** C side — `endpoints/c/test/test_receive.c` against
`golden_data_le.bin` (Makefile run `make -C endpoints/c test`). Output
measured in this audit: received DATA frame, 40-byte sample body / agent DATA
decoded. Rust side — `crates/endpoint-e2e/tests/c.rs::c_endpoint_sync` and
`crates/endpoint-e2e/tests/c.rs::c_endpoint_async`: the peer answers with
id = 9 only, so the C program has to read the pong out of it. The reference
for the golden: `crates/xrce/src/submessages/data.rs::tests::data_uses_id_9`,
`crates/xrce/src/submessages/data.rs::tests::data_base_object_request_precedes_the_sample`.

**Status:** done

### §1.3 Serial framing (Annex C)

**Spec:** §1.3 (`docs/specs/zerodds-endpoint-c-1.0.md`, DDS-XRCE 1.0 Annex C.1)
— `7E | sourceAddress | remoteAddress | payloadLength (LE) | payload |
frameCheck (LE)`; stuffing of `0x7E` and `0x7D` after the flag; `frameCheck`
the FCS-16 of RFC 1662 over addresses, length and payload; refusal of a wrong
`frameCheck` and of `0x7E` before the end of the frame (C.1.3).

**Repo:** `endpoints/c/src/zerodds_endpoint.c` (`zdw_fcs16`,
`zdw_serial_frame`, `zdw_serial_deframe`).

**Tests:** C side — `endpoints/c/test/test_serial_frame.c` against
`golden_serial_le.bin` (`tools/endpoints-gate`, run on the Linux test host at
`2fa0800f55a`). Measured output: byte-stuffing ok / fcs16 0x906E, check and
flag refusals ok / XRCE serial frame 59 bytes byte-identical to crates/xrce /
serial round-trip ok. There is no live E2E case for this clause: the Rust peer
speaks UDP, not a serial link. The reference the golden stands against byte
for byte:
`crates/xrce/src/transport_serial.rs::tests::the_frame_check_is_the_fcs16_of_rfc_1662`,
`crates/xrce/src/transport_serial.rs::tests::a_frame_carries_the_addresses_the_length_and_a_little_endian_check`,
`crates/xrce/src/transport_serial.rs::tests::flag_and_escape_octets_are_stuffed_everywhere_after_the_flag`,
`crates/xrce/src/transport_serial.rs::tests::a_frame_with_a_wrong_check_is_refused`,
`crates/xrce/src/transport_serial.rs::tests::a_begin_flag_inside_a_frame_drops_it_and_begins_the_next`;
`endpoints/golden-gen/src/main.rs` produces `golden_serial_le.bin` with
`encode_frame(1, 0, …)` from that implementation.

**Status:** done

## §2 sync send/recv

### §2.1 Frame-hook contract (`zdw_transport`)

**Spec:** §2.1 (`docs/specs/zerodds-endpoint-c-1.0.md`, ADR 0013 invariant 5)
— the endpoint is transport-opaque: it hands a fully framed, encoded message
to the transport the integrator fills in (`ctx` plus function pointers for
deliver and receive) and gets complete frames back. The endpoint opens no
socket of its own.

**Repo:** `endpoints/c/include/zerodds_endpoint.h` (the `zdw_transport`
struct, `zdw_endpoint_send`, `zdw_endpoint_recv`).

**Tests:** C side — `endpoints/c/test/test_endpoint_loopback.c` with an
in-memory loopback transport (encode, send, receive, decode, field compare;
Makefile run `make -C endpoints/c test`). Output measured in this audit:
frame-hook 40-byte frame delivered + received + decoded ok. Rust side —
`crates/endpoint-e2e/tests/c.rs::c_endpoint_sync` plugs a real POSIX UDP
socket into that same frame hook and thereby shows the contract holds beyond
the test stub.

**Status:** done

### §2.2 Poll-loop idiom (C89)

**Spec:** §2.2 (`docs/specs/zerodds-endpoint-c-1.0.md`) — sync is the C89
baseline use of the frame hook: the integrator owns the run loop and calls
`zdw_endpoint_recv` itself. There MUST be a runnable example, not a stub.

**Repo:** `endpoints/c/examples/example_sync.c` (a C89 poll loop over the
transport, compiled with `-std=c89 -pedantic -Wall -Wextra`).

**Tests:** C side — the example run (`make -C endpoints/c examples`, target
`example_sync`). Output measured in this audit: five decoded `Reading`
samples (id 0x1000 through 0x1004) and ALL OK, 0 compiler warnings. Rust side
— `crates/endpoint-e2e/tests/c.rs::c_endpoint_sync` runs the same idiom in a
purpose-built program live against the Rust peer.

**Status:** done

## §3 async reader/writer

### §3.1 Event-driven reactor (`zdw_async_reader` / `zdw_async_writer`)

**Spec:** §3.1 (`docs/specs/zerodds-endpoint-c-1.0.md`, ADR 0013) — additive
to the C89 core, a latency-decoupled, callback-driven alternative to the poll
loop (C11, no `malloc`).

**Repo:** `endpoints/c/include/zerodds_async.h` /
`endpoints/c/src/zerodds_async.c` (`zdw_async_reader_init` binds an
`on_sample` callback, `zdw_async_run` drains the transport and dispatches
every decoded sample; plus `zdw_async_writer_init` and `zdw_async_write`).

**Tests:** C side — `endpoints/c/test/test_async_loopback.c` over an
in-memory FIFO transport (Makefile run `make -C endpoints/c test`). Output
measured in this audit: async loopback 5 samples dispatched + decoded in
order. Rust side — `crates/endpoint-e2e/tests/c.rs::c_endpoint_async`
(`zdw_async_write` frames and delivers, `zdw_async_poll` unwraps and calls
the callback, live over UDP).

**Status:** done

### §3.2 Live UDP reactor

**Spec:** §3.2 (`docs/specs/zerodds-endpoint-c-1.0.md`) — evidence that the
reactor drives real datagram I/O, not only an in-memory queue.

**Repo:** the same reactor functions from `endpoints/c/src/zerodds_async.c`,
wired to a non-blocking POSIX UDP socket in
`endpoints/c/test/test_async_udp.c`.

**Tests:** C side — `endpoints/c/test/test_async_udp.c` (Makefile run
`make -C endpoints/c test`). Output measured in this audit: async UDP 5/5
samples received + decoded via reactor. Rust side —
`crates/endpoint-e2e/tests/c.rs::c_endpoint_async` (the same reactor against
a foreign process rather than a second socket in the same program),
`crates/endpoint-e2e/tests/c.rs::c_raw_udp` (the counter-check without the
XRCE envelope).

**Status:** done

### §3.3 Async deep example

**Spec:** §3.3 (`docs/specs/zerodds-endpoint-c-1.0.md`) — a runnable example
for the reactor, not a stub.

**Repo:** `endpoints/c/examples/example_async.c` (a C11 reactor plus
callback, compiled with `-std=c11 -pedantic -Wall -Wextra`).

**Tests:** C side — the example run (`make -C endpoints/c examples`, target
`example_async`). Output measured in this audit: five decoded readings (id
0x2000 through 0x2004) and ALL OK, 0 compiler warnings. Rust side —
`crates/endpoint-e2e/tests/c.rs::c_endpoint_async` shows the same callback
path against a real peer.

**Status:** done

### §3.4 Test obligation: live ping-pong E2E

**Spec:** §3.4 (`docs/specs/zerodds-endpoint-c-1.0.md`) — in addition to the
isolated unit and loopback tests of §1 through §3, every language MUST
provide a live E2E file following the `crates/endpoint-e2e/tests/<lang>.rs`
pattern (reference: `crates/endpoint-e2e/tests/cpp.rs`), so that framing
(§1), sync (§2) and async (§3) are shown working together and not only
individually.

**Repo:** `crates/endpoint-e2e/tests/c.rs` — three cases over the C endpoint
surface: the generated codec straight on a POSIX UDP socket without the XRCE
envelope; the same probe inside an XRCE WRITE_DATA via `zdw_endpoint_send`
and `zdw_endpoint_recv`; the event-driven surface via `zdw_async_write` and
`zdw_async_poll`. The wire runtime `runtime/c` is built into a static library
through its own Makefile because its `CFLAGS` are a precondition; the include
path puts `runtime/c/include` before `endpoints/c/include`, because
`runtime/c/include/zerodds_wire.h` and `endpoints/c/include/zerodds_wire.h`
carry the same include guard.

**Tests:** Rust side, default run — `crates/endpoint-e2e/tests/c.rs::c_raw_udp`,
`crates/endpoint-e2e/tests/c.rs::c_endpoint_sync`,
`crates/endpoint-e2e/tests/c.rs::c_endpoint_async`. Measured (command
`cargo test -p zerodds-endpoint-e2e --test c`): 3 passed, 0 failed, no SKIP
line. The three cases sit behind no feature gate but return with a `SKIP`
line when no C compiler is on the `PATH`, and then count as passed without
having checked anything; the pipeline job `endpoint-e2e` fails on such a
line. The Rust peer checks the exact 23 octets of the `Ping` and the program
checks the answer line carrying `reply=pong:hello from app`.

**Status:** done

## §4 reliable stream

### §4.1 C89 state machine (`zdw_reliable`)

**Spec:** §4 (`docs/specs/zerodds-endpoint-c-1.0.md`) together with
`docs/spec-coverage/reliable-endpoint-1.0.en.md` §3 — a reliable sender and
receiver, mirrored from `crates/xrce/src/reliable.rs`; fixed storage, no
`malloc`: window 16, receive buffer 64, RFC 1982 16-bit sequence numbers.

**Repo:** `endpoints/c/include/zerodds_endpoint.h` (declaration) and
`endpoints/c/src/zerodds_endpoint.c` (C89 implementation) — sender side
`zdw_reliable_submit`, `zdw_reliable_pending_heartbeat`,
`zdw_reliable_recv_acknack`, `zdw_reliable_get_in_flight`; receiver side
`zdw_reliable_recv_data`, `zdw_reliable_drain`,
`zdw_reliable_pending_acknack`; plus `zdw_reliable_reset`.

**Tests:** C side — `endpoints/c/test/test_reliable_sm.c` (Makefile run
`make -C endpoints/c test`): 14 check functions, 13 of them mirroring the
reference tests and one a byte-golden check. Output measured in this audit:
test_reliable_sm ALL OK. The mirrored reference is
`crates/xrce/src/reliable.rs::tests::submit_assigns_monotonic_seqnrs`,
`crates/xrce/src/reliable.rs::tests::submit_rejects_when_window_full`,
`crates/xrce/src/reliable.rs::tests::pending_heartbeat_fires_first_time`,
`crates/xrce/src/reliable.rs::tests::pending_heartbeat_silenced_until_period_elapsed`,
`crates/xrce/src/reliable.rs::tests::recv_acknack_clears_acked_seqnrs`,
`crates/xrce/src/reliable.rs::tests::recv_acknack_full_clear_when_no_bits_set`,
`crates/xrce/src/reliable.rs::tests::recv_data_buffers_in_order`,
`crates/xrce/src/reliable.rs::tests::recv_data_reorders_out_of_order`,
`crates/xrce/src/reliable.rs::tests::recv_data_drops_duplicates`,
`crates/xrce/src/reliable.rs::tests::recv_data_rejects_when_buffer_full`,
`crates/xrce/src/reliable.rs::tests::pending_acknack_marks_missing_slots`,
`crates/xrce/src/reliable.rs::tests::reset_clears_state_completely`,
`crates/xrce/src/reliable.rs::tests::end_to_end_sender_receiver_with_loss_recovery`.

**Status:** done

### §4.2 Byte-golden HEARTBEAT and ACKNACK

**Spec:** §4 (`docs/specs/zerodds-endpoint-c-1.0.md`) together with
`docs/spec-coverage/reliable-endpoint-1.0.en.md` §4 — ACKNACK carries
`first_unacked_seq_num` as an `i16`, `nack_bitmap` as two little-endian bytes
and `stream_id` as one byte; HEARTBEAT carries `first_unacked_seq_nr` and
`last_unacked_seq_nr` as `i16` plus `stream_id`.

**Repo:** `endpoints/c/src/zerodds_endpoint.c` (the codec halves of
`zdw_reliable_pending_heartbeat` and `zdw_reliable_pending_acknack`), checked
by the byte-golden function in `endpoints/c/test/test_reliable_sm.c` and by
the pure wire test `endpoints/c/test/test_reliable.c`.

**Tests:** C side — `endpoints/c/test/test_reliable.c` and
`endpoints/c/test/test_reliable_sm.c` against `golden_heartbeat_le.bin` and
`golden_acknack_le.bin` (Makefile run `make -C endpoints/c test`). Output
measured in this audit: HEARTBEAT parsed first=1 last=3 stream=0x80 / ACKNACK
13 bytes byte-identical to crates/xrce. The reference both goldens come from:
`crates/xrce/src/submessages/heartbeat.rs::tests::heartbeat_le_roundtrip`,
`crates/xrce/src/submessages/heartbeat.rs::tests::heartbeat_roundtrip_via_submessage`,
`crates/xrce/src/submessages/acknack.rs::tests::acknack_le_roundtrip`,
`crates/xrce/src/submessages/acknack.rs::tests::acknack_roundtrip_via_submessage`.

**Status:** done

### §4.3 pthread SPSC ring as the async writer (latency decoupling)

**Spec:** §4 (`docs/specs/zerodds-endpoint-c-1.0.md`) together with
`docs/spec-coverage/reliable-endpoint-1.0.en.md` §2 — the producer must never
enter the kernel; the component carries the reliable sender state at the same
time.

**Repo:** `endpoints/c/include/zerodds_reliable_async.h` /
`endpoints/c/src/zerodds_reliable_async.c` (C11 plus pthread):
`zdw_async_ring_start` starts the drain thread, `zdw_async_ring_enqueue` is a
wait-free enqueue without a syscall, and the drain thread holds the
`zdw_reliable` state and does the framing and the I/O.

**Tests:** C side — `endpoints/c/test/test_reliable_async_overflow.c`
(Makefile run `make -C endpoints/c test`): checks the behaviour at a full
send window and that the drain thread does not acknowledge itself. Output
measured in this audit: ALL OK, delivered 24/24, no self-ack, no hang. Plus
`endpoints/c/test/bench_reliable_async.c` as the latency comparison (see
§4.6). Rust side —
`crates/endpoint-e2e/tests/c_reliable.rs::c_reliable_latency_bench` builds and
runs the same ring out of the default run.

**Status:** done

### §4.4 Loss-recovery example

**Spec:** §4 (`docs/specs/zerodds-endpoint-c-1.0.md`) together with
`docs/spec-coverage/reliable-endpoint-1.0.en.md` §5 point 5 — a runnable
example, not a stub: N samples, injected loss, gap-free delivery after
retransmit.

**Repo:** `endpoints/c/examples/example_reliable.c` — 12 samples submitted,
every fourth dropped on the first pass, recovery rounds until all 12 have
arrived in the sequence 0 through 11.

**Tests:** C side — the example run (`make -C endpoints/c examples`, target
`example_reliable`; also in the Makefile target `test`). Output measured in
this audit: 3/12 delivered before recovery, delivered contiguous 0..11
expected=12, ALL 12 samples recovered gap-free. Rust side —
`crates/endpoint-e2e/tests/c_reliable.rs::c_reliable_loss_recovery` shows the
same sequence live over UDP against a foreign process instead of in-process.

**Status:** done

### §4.5 Live E2E against the Rust reference peer

**Spec:** §4 (`docs/specs/zerodds-endpoint-c-1.0.md`) together with
`docs/spec-coverage/reliable-endpoint-1.0.en.md` §5 point 3 — live against
the Rust reliable peer, `stream_id >= 128`, with an injected drop; assert:
every sample delivered gap-free and in order despite the loss.

**Repo:** `endpoints/c/test/reliable_udp_app.c` — the C app plays the
reliable sender (submit, WRITE_DATA, HEARTBEAT, ACKNACK-driven retransmit)
against the shared Rust peer in `crates/endpoint-e2e/src/lib.rs`, which
injects the loss. The file declares `_POSIX_C_SOURCE 200809L` and
additionally `_DARWIN_C_SOURCE 1`, because `INADDR_LOOPBACK` is a BSD socket
extension under Apple's libc and would otherwise be invisible; glibc does not
have that restriction.

**Tests:** Rust side, default run —
`crates/endpoint-e2e/tests/c_reliable.rs::c_reliable_loss_recovery`
(12 samples, injected drop, assert on gap-freeness) and
`crates/endpoint-e2e/tests/c_reliable.rs::c_reliable_no_loss` (12 samples
without a drop). Measured (command `cargo test -p zerodds-endpoint-e2e --test
c_reliable`): 3 passed, 0 failed, no SKIP line. The compiler pre-check from
the head of this document applies here too — without a compiler on the search
path the cases skip loudly and the pipeline job fails on it.

**Status:** done

### §4.6 Latency: inline send versus decoupled ring enqueue

**Spec:** §4 (`docs/specs/zerodds-endpoint-c-1.0.md`) together with
`docs/spec-coverage/reliable-endpoint-1.0.en.md` §5 point 4 — the producer
latency from `write` to `return` in the decoupled path MUST be measured
against the inline deliver; one figure that shows the decoupling.

**Repo:** `endpoints/c/test/bench_reliable_async.c` — measures
`inline_ns_per_op` (framing plus the `send()` syscall per sample against a
bound but unread loopback socket) against `decoupled_enqueue_ns_per_op`
(a bare enqueue into the ring, no kernel). The file carried the same
`INADDR_LOOPBACK` cause as §4.5 and therefore declares `_DARWIN_C_SOURCE 1`
as well.

**Tests:** Rust side, default run —
`crates/endpoint-e2e/tests/c_reliable.rs::c_reliable_latency_bench` (builds
the bench program with `gcc -O2 -std=c11`, runs it and fails the case when
the latency line is missing or the decoupled enqueue is not below the inline
send). C side — the same program in the Makefile target (`test`). Measured in
this audit on macOS 15 (Apple Clang): inline_ns_per_op=2585 against
decoupled_enqueue_ns_per_op=13 over 5000 delivered samples. The absolute
inline figure varies with host load; the assert checks the ordering, not the
magnitude.

**Status:** done

---

## Audit-Status

15 done / 0 partial / 0 open / 0 n/a (informative) / 0 n/a (rejected).

Test run, C side: `cargo run -p zerodds-endpoint-golden -- <dir>` followed by
`make -C endpoints/c test GOLDEN_DIR=<dir>` and
`make -C endpoints/c examples` — all 20 binaries build with
`-std=c89 -pedantic -Wall -Wextra` or `-std=c11 -pedantic -Wall -Wextra`
without a warning and run through, 0 failed.

Test run, Rust side: `cargo test -p zerodds-endpoint-e2e --test c` — 3
passed, 0 failed; `cargo test -p zerodds-endpoint-e2e --test c_reliable` — 3
passed, 0 failed. None of the six cases skipped.

Both runs measured on macOS 15, where `cc` and `gcc` are Apple Clang.

Open items: none. Decision records: none.
