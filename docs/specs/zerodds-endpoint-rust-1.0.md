# `zerodds-endpoint-rust` v1.0 — native Rust endpoint SDK

ZeroDDS vendor spec. Implemented in `endpoints/rust/`. Builds on ADR 0013
(native endpoint SDKs, frame hook contract) and DDS-XRCE 1.0 §8.3 (framing).
For the reliable stream contract see
[`reliable-endpoint-1.0`](reliable-endpoint-1.0.md). For codegen/wire format details
(XCDR2 encoding of the sample bodies) see
[`zerodds-xcdr2-rust-1.0`](zerodds-xcdr2-rust-1.0.md).

## §1 XRCE framing

Rust is one of the four native endpoint languages from ADR 0013 (C/C++/Python/
Rust) and at the same time the **shared reference peer** against which the other 13
endpoint languages run their own E2E tests (`crates/endpoint-e2e`). The
framing is transport-opaque: the frame hook contract (`deliver(frame)` /
`receive() -> frame`) knows no socket, no serial device — the
caller attaches their transport behind it (here: `MemTransport` for
tests/examples, UDP in the shared peer).

A WRITE_DATA frame is an 8-byte header + XCDR2 sample body:

```
[session, stream, seq_lo, seq_hi, submsg_id, flags, len_lo, len_hi] + body
```

- `session` = `0x80` (no-key session).
- `stream` = `0x01` (best-effort) in the sync/async path; `0x80` (bit 7 set)
  in the reliable path (§4).
- `seq` = 16-bit LE sequence number, monotonic per stream.
- `submsg_id` = `0x07` (WRITE_DATA).
- `flags` = `0x01` (E flag = LE plus `DataFormat` `FORMAT_DATA` = `0b000`).
- `len` = 16-bit LE length of the body.
- `body` = the XCDR2-encoded sample (LE), byte-identical to the
  `zerodds-cdr` reference (see `zerodds-xcdr2-rust-1.0` §2/§5).

The body carries **no** XRCE header of its own — the 8-byte frame is the
only framing layer; sample encoding starts directly after byte 8.

MUST: `xrce_write_frame`/`xrce_read_frame` build and parse exactly this
frame; a frame is a WRITE_DATA only if byte 4 is `0x07`.

Because Rust takes the peer role here (the 13 other languages talk
to it), there is no standalone Rust-against-Rust ping-pong test —
framing conformance is checked transitively by each of the 13
language E2E tests in `crates/endpoint-e2e/tests/`.

The submessage body is the `BaseObjectRequest` (DDS-XRCE 1.0 §7.7.8:
`request_id` then `object_id`, two octets each, most significant first),
followed by the sample. The flag byte is `0x01` — E flag (little-endian body)
plus `DataFormat` `FORMAT_DATA` (`0b000`) in bits 1..3 (§8.3.5.8);
`FORMAT_SAMPLE` (`0b001`, flag byte `0x03`) additionally requires a
`SampleInfo`, which this profile does not carry. The `request_id` is the
`sequence_nr` of the same message (§7.7.8 only requires uniqueness per
stream), the `object_id` names the target DataWriter — without it no
XRCE agent can assign the frame. The read path returns the sample behind these
four octets and rejects a body that is too short for them.
Measured by `crates/endpoint-e2e/tests/xrce_write_framing.rs`.

## §2 sync client

`Client` is the polling, non-blocking endpoint for callers that
own their run loop (no background thread, no callback).

MUST:

- `Client::new(transport)` — binds to a frame hook transport, initial
  sequence number `1`.
- `write(sample: &[u8])` — frames the XCDR2 sample body per §1 with the
  current `seq`, increments `seq` (`wrapping_add`), delivers via the
  transport. No reliability state — fire-and-forget.
- `poll() -> Option<Vec<u8>>` — exactly one non-blocking receive attempt;
  `None` if the transport is empty; otherwise the decoded sample body (the
  8-byte frame is removed).

The `write→poll` round trip is the reference path for the deep example
(`examples/example_sync.rs`): N `Reading{id,value,label}` samples,
decoded field by field.

## §3 async reader/writer

`AsyncReader` is the background reader half: no async runtime
(no `tokio`/`async-std`), but the idiomatic std concurrency model
— an OS thread plus an `mpsc` channel.

MUST:

- `AsyncReader::start(transport)` — spawns a thread that polls the transport
  in a loop (backoff `1ms` on an empty transport) and delivers every
  decoded WRITE_DATA body via an `mpsc::channel` to `recv`.
- `recv() -> Vec<u8>` — blocks until the next decoded sample body.
- `stop()` — signals the reader thread via an `AtomicBool` to
  stop; no forced abort of running work.

The async writer half (decoupled producer, SPSC ring, drain thread)
is part of the reliable stream (§4) — in the best-effort path there is no
separate `AsyncWriter`, because `Client::write` is already non-blocking
(no syscall wait, direct enqueueing in `MemTransport`).

Reference path: `examples/example_async.rs` — N samples, `reader.recv()`
blocking per sample, complete field decode.

## §4 reliable stream

Rust implements the full state machine contract canonically defined in
[`reliable-endpoint-1.0`](reliable-endpoint-1.0.md) §3
(sender + receiver, RFC-1982 sequence numbers,
`HEARTBEAT_PERIOD=500ms`, `SENDER_WINDOW=16`, `RECEIVER_BUFFER=64`,
`MAX_PAYLOAD=65535`) **self-contained** in `endpoints/rust/src/reliable.rs`
— deliberately duplicated instead of imported from `crates/xrce` (no
`zerodds-xrce` runtime dependency of the endpoint SDK on the hub crate).

MUST — sender (`ReliableSender`):

- `submit(payload) -> Result<seq, ReliableError>` — `PayloadTooLarge` for
  `len > MAX_PAYLOAD`, `WindowFull` for `in_flight >= SENDER_WINDOW`.
- `pending_heartbeat(now) -> Option<Heartbeat>`.
- `recv_acknack(AckNack)` — RFC-1982 comparison, removes acked entries from
  `in_flight`.
- `get_in_flight(seq)` — retransmit lookup.

MUST — receiver (`ReliableReceiver`):

- `recv_data(seq, payload) -> Result<(), ReliableError>` — duplicates
  discarded, `BufferFull` at `RECEIVER_BUFFER`.
- `drain_in_order() -> Vec<(seq, payload)>`.
- `pending_acknack(hint_last_seen) -> AckNack`.
- `reset()`.

MUST — wire: `write_frame`/`unframe` for the reliable data path
(`stream_id = 0x80`, header seq = sample seq); `heartbeat_frame`/
`acknack_frame`/`parse_heartbeat`/`parse_acknack` for the control path
(stream `0x00`, `submsg_id` `0x0B`/`0x0A`, `flags=0x01`); byte-identical to
the C SDK goldens (`golden_heartbeat_le.bin`/`golden_acknack_le.bin`).

MUST — async-decoupled writer (`AsyncReliableWriter` + `SpscRing` +
`ReliableWriterHandle`): the producer only calls
`enqueue(sample)` on a wait-free SPSC ring (no syscall, no
lock); a dedicated drain thread owns the socket + `ReliableSender` state
and does all the I/O (batched `WRITE_DATA` send, periodic
HEARTBEAT, ACKNACK-driven retransmit from the history). `shutdown()`
blocks until ring **and** sender window are empty, with a
safety deadline against a peer that has gone silent.

Rust is at the same time the shared reliable peer (`crates/endpoint-e2e`,
`reliable_collect`) for all other languages — analogous to §1 there is no
standalone Rust-against-Rust loss recovery test; the loss recovery
conformance of the `endpoints/rust` `AsyncReliableWriter` itself is checked via
`crates/endpoint-e2e/tests/rust_reliable.rs` (Rust SDK as the sender,
shared peer as the dropping receiver).

MUST — latency proof (evidence requirement 4 from `reliable-endpoint-1.0` §5):
`enqueue` (wait-free, ring) measured against an inline `send`
(kernel transition) on the same producer path, against the same idle
UDP sink. Reference path: `examples/example_reliable.rs::bench`.
