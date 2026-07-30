# `zerodds-endpoint-rust` 1.0 — Spec Coverage

**Source:** `docs/specs/zerodds-endpoint-rust-1.0.md` — the ZeroDDS Rust endpoint SDK
spec (ADR 0013 + DDS-XRCE 1.0 §8.3/§8.4.10/§8.4.11 + `reliable-endpoint-1.0`).

Implementation:

- `endpoints/rust/` — the native Rust endpoint SDK (ADR 0013): `src/lib.rs` (XRCE framing,
  `Client`, `AsyncReader`), `src/reliable.rs` (reliable state machine + async-decoupled
  writer), `examples/{example_sync,example_async,example_reliable}.rs`.
- `crates/endpoint-e2e/` — the shared Rust XRCE peer (`src/lib.rs`) + the reliable E2E
  (`tests/rust_reliable.rs`).

## §1 XRCE framing

### §1 WRITE_DATA frame (8-byte header + body)

**Spec:** §1 -- "A WRITE_DATA frame is an 8-byte header plus an XCDR2 sample body:
`[session, stream, seq_lo, seq_hi, submsg_id, flags, len_lo, len_hi] + body`. `session=0x80`,
`stream=0x01` (best-effort), `submsg_id=0x07`, `flags=0x03`."

**Repo:** `endpoints/rust/src/lib.rs::xrce_write_frame`/`xrce_read_frame` (line 62–85,
best-effort stream `0x01`, session `0x80`). The shared peer has its own, independently
written framing implementation `crates/endpoint-e2e/src/lib.rs::xrce_frame`/`xrce_unframe`
(line 49–72) — the same 8-byte layout, the same constants (`0x80`/`0x01`/`0x07`/`0x03`),
byte-identical despite the separate implementation.

**Tests:** indirect, via `example_sync`/`example_async` (CI job `endpoints-rust`); no
isolated byte-golden unit test for the best-effort frame (the reliable frame has its own,
see §4).

**Status:** done

### §1 Peer role in the ping-pong (no standalone Rust ping-pong test)

**Spec:** §1 -- "Because Rust plays the peer role here (the other 13 languages talk to
it), there is no standalone Rust-vs-Rust ping-pong test — framing conformance is checked
transitively by each of the 13 language E2E tests in `crates/endpoint-e2e/tests/`."

**Repo:** `crates/endpoint-e2e/src/lib.rs::ping_pong` (line 97–128) is the peer itself.
There is no `crates/endpoint-e2e/tests/rust.rs`, because Rust is not "the app" here — it
is the peer every other language talks to; a Rust ping-pong test against itself would not
check anything additional. `endpoints/rust`'s own Client/Reader pair is instead exercised
through the MemTransport examples (§2/§3), not through the UDP peer path.

**Tests:** transitive — every one of the 14 language E2E tests (`ada.rs`, `c.rs`,
`cpp.rs`, …) exercises `ping_pong`; no direct Rust-own test.

**Status:** n/a (informative) — structural, not open: Rust is the peer implementation,
not a tested client of this peer.

## §2 sync Client

### §2 `Client::write`/`poll`

**Spec:** §2 -- "A polling, non-blocking receive path — the idiom for callers that own
their own run loop. `write` frames an XCDR2 sample body with the current `seq`
(`wrapping_add`), `poll` returns a decoded body or `None`."

**Repo:** `endpoints/rust/src/lib.rs::Client` (line 113–138): `write` frames an XCDR2
sample body and delivers it via `MemTransport`; `poll` returns a decoded body or `None`
(non-blocking).

**Tests:** `endpoints/rust/examples/example_sync.rs` — 5 `Reading{id,value,label}`
samples, decoded field by field (`id`, `value`, `label`), `assert!(got == total)` +
`"ALL OK"`; CI job `endpoints-rust`.

**Status:** done

## §3 async Reader/Writer

### §3 `AsyncReader` (thread + `mpsc`)

**Spec:** §3 -- "A background reader that delivers decoded samples over a channel — no
async runtime, but the idiomatic std concurrency model (thread + channel). `start` spawns
a thread, `recv` blocks, `stop` signals via an `AtomicBool`."

**Repo:** `endpoints/rust/src/lib.rs::AsyncReader` (line 142–180): `start` spawns a
thread that polls the transport and forwards decoded bodies to `recv` via
`mpsc::channel`; `stop` signals the thread via an `AtomicBool`.

**Tests:** `endpoints/rust/examples/example_async.rs` — 5 samples, `reader.recv()`
blocking, every field decoded, `"ALL OK"`; CI job `endpoints-rust`.

**Status:** done

## §4 reliable stream

### §4 State machine (sender + receiver)

**Spec:** §4 -- "XRCE reliable stream (`stream_id >= 128`, §8.4.10/§8.4.11), window 16
(matches the 16-bit ACKNACK bitmap), receiver buffer 64, heartbeat 500 ms, payload
≤ 65535, RFC-1982 16-bit sequence numbers — mirrors `crates/xrce/src/reliable.rs`."

**Repo:** `endpoints/rust/src/reliable.rs` — `ReliableSender` (`submit`/
`pending_heartbeat`/`recv_acknack`/`get_in_flight`/`in_flight_seqs`, line 201–292),
`ReliableReceiver` (`recv_data`/`drain_in_order`/`pending_acknack`/`reset`, line
300–376). Self-contained, no `zerodds-xrce` runtime dependency (deliberately duplicated
rather than imported, see the module doc comment, line 1–21).

**Tests:** `cargo test -p zerodds-endpoint-rust --lib` — 14 state-machine tests
(`submit_assigns_monotonic_seqnrs`, `submit_rejects_payload_too_large`,
`submit_rejects_when_window_full`, `heartbeat_fires_first_then_silences_until_period`,
`heartbeat_none_when_window_empty`, `recv_acknack_clears_acked_keeps_missing`,
`recv_acknack_full_clear_when_no_bits`, `recv_data_delivers_in_order`,
`recv_data_reorders_out_of_order`, `recv_data_drops_duplicates`,
`recv_data_rejects_when_buffer_full`, `pending_acknack_marks_missing_slots`,
`reset_clears_receiver`, `end_to_end_loss_recovery_in_process`) + 2 wire/ring tests
(`write_frame_round_trips`, `spsc_ring_fifo_and_backpressure`) + 2 byte-golden (next
item) = 18 of 18 in the crate. Reproduced locally (this run): 18/18 passed, 0 failed.

**Status:** done

### §4 Wire (byte-golden HEARTBEAT/ACKNACK)

**Spec:** §4 -- "HEARTBEAT (`0x0B`) and ACKNACK (`0x0A`) byte-identical to the C SDK's
reference goldens (`golden_heartbeat_le.bin`/`golden_acknack_le.bin`); XRCE control
convention: header stream = NONE (`0x00`) plus control-message seq, target stream id in
the body's last byte."

**Repo:** `endpoints/rust/src/reliable.rs::heartbeat_frame`/`acknack_frame`/
`parse_heartbeat`/`parse_acknack` (line 125–192).

**Tests:** `heartbeat_frame_byte_golden` asserts
`heartbeat_frame(Heartbeat{first:1,last:3,stream_id:0x80},1)` ==
`[128,0,1,0,11,1,5,0,1,0,3,0,128]`; `acknack_frame_byte_golden` asserts
`acknack_frame(AckNack{first_unacked:1,nack_bitmap:[0,0],stream_id:0x80},1)` ==
`[128,0,1,0,10,1,5,0,1,0,0,0,128]` — byte-identical to the C goldens.

**Status:** done

### §4 Async-decoupled writer + loss recovery (E2E)

**Spec:** §4 -- "The producer only enqueues (wait-free) into an SPSC ring; a dedicated
drain thread owns the socket and the reliable sender state and does all I/O (`WRITE_DATA`
send, the HEARTBEAT timer, ACKNACK-driven retransmit). The producer never enters the
kernel. Reliable delivery survives datagram loss — verified live against the shared Rust
peer with injected loss."

**Repo:** `endpoints/rust/src/reliable.rs::SpscRing` (line 384–443, wait-free `push`/`pop`
over two atomics, no lock), `ReliableWriterHandle::enqueue` (line 458–460, never enters
the kernel), `AsyncReliableWriter::start`/`drain_loop` (line 469–574: ring → sender
window → `send`, HEARTBEAT when due, ACKNACK drain → retransmit, `shutdown()` blocks until
the ring is empty AND the window is empty, with a 5s safety deadline against a peer that
stops ACKing). `crates/endpoint-e2e/src/lib.rs::reliable_collect` drives the peer as the
receiver (with `drop_every=Some(3)`, drops every 3rd distinct sample exactly once).

**Tests (codepit):** `cargo test -p zerodds-endpoint-e2e --test rust_reliable` —
`rust_reliable_loss_recovery` (peer drops every 3rd sample) and
`rust_reliable_no_loss_baseline` (lossless) — each delivers 12/12 samples gap-free in
order (`N = 12` in `tests/rust_reliable.rs`). 2/2 passed. Runs via the workspace-wide
`cargo test --workspace` job, not the separate `endpoints-rust` CI job (which covers only
the sync/async examples). Reproduced locally (this run): 2/2 passed in 2.47s.

**Status:** done

### §4 Rust as the shared peer (no standalone Rust loss-recovery test)

**Spec:** §4 -- "Rust is simultaneously the shared reliable peer
(`crates/endpoint-e2e`, `reliable_collect`) for all other languages — as with §1 there is
no standalone Rust-vs-Rust loss-recovery test; the loss-recovery conformance of
`endpoints/rust`'s own `AsyncReliableWriter` is checked via
`crates/endpoint-e2e/tests/rust_reliable.rs` (Rust SDK as sender, shared peer as the
dropping receiver)."

**Repo:** identical to the previous item — `rust_reliable.rs` drives `endpoints/rust`'s
`AsyncReliableWriter` as the sender against `crates/endpoint-e2e`'s `reliable_collect` as
the receiver; structurally this is the only "Rust loss-recovery test" there is, because
Rust supplies both the sender-under-test and the reference receiver implementation at
once.

**Tests:** see previous item (`rust_reliable_loss_recovery`, `rust_reliable_no_loss_baseline`).

**Status:** n/a (informative) — structural, not open: the test obligation is already met
by the previous item; this entry only documents why there is no *separate* Rust-vs-Rust
test.

### §4 Latency proof: decoupled vs. inline

**Spec:** §4 -- "MUST — latency proof (test obligation 4 from `reliable-endpoint-1.0`
§5): `enqueue` (wait-free, ring) measured against an inline `send` (kernel transition) on
the same producer path, against the same idle UDP sink."

**Repo:** `endpoints/rust/examples/example_reliable.rs::bench` (line 71–115) — 50,000
iterations, `handle.enqueue` against the ring vs. `isock.send(&write_frame(...))` inline,
both measured against the same idle UDP sink.

**Tests (codepit):** `cargo run -p zerodds-endpoint-rust --example example_reliable --
bench` → `enqueue(decoupled)=30ns inline(send)=3985ns` (~133×). Reproduced locally (this
run, a different machine): `enqueue=124ns inline=4422ns` (~35×) — the order of magnitude
confirmed, the absolute value is machine-dependent.

**Status:** done

---

## Audit status

7 done / 0 partial / 0 open / 2 n/a (informative) / 0 n/a (rejected).

Test run (codepit figures + reproduced locally): `cargo test -p zerodds-endpoint-rust
--lib` — 18/18 (state machine + wire round-trip + SPSC ring + byte-golden); `cargo test -p
zerodds-endpoint-e2e --test rust_reliable` — 2/2 (`rust_reliable_loss_recovery`,
`rust_reliable_no_loss_baseline`); `cargo run -p zerodds-endpoint-rust --example
example_sync|example_async` — 5/5 field decode each, `"ALL OK"` (CI job `endpoints-rust`);
latency bench `enqueue=30ns` vs. `inline=3985ns` (~133×, codepit) — reproduced locally at
`124ns`/`4422ns` (~35×).

Open items: none.
