# `zerodds-endpoint-c` v1.0 — Native C endpoint SDK (XRCE framing, sync/async, reliable stream)

ZeroDDS vendor spec. Implemented in `endpoints/c/`.

Builds on [ADR 0013](../adr/0013-native-endpoint-sdks.md) (native endpoint SDKs,
frame hook contract) and DDS-XRCE 1.0 §8.3 (framing) as well as §8.4.10/§8.4.11
(reliable streams). The reliable delivery contract itself is standardized
separately in [`reliable-endpoint-1.0`](reliable-endpoint-1.0.md).

## §1 XRCE framing

The C endpoint MUST create and parse a DDS-XRCE-1.0-conformant **framing structure** — message header
(§8.3.2) and submessage header (§8.3.4) — both for packet-based transports
(UDP) and for serial byte-stream transports (RS-485, UART).

The **data submessage payload** of the C endpoint is the spec-conformant
DDS-XRCE form (§1.1): the WRITE_DATA/DATA body starts with the
`BaseObjectRequest` (`request_id` + `object_id`) required by
DDS-XRCE §8.3.5.8/§7.7.8, followed by the XCDR sample. The `object_id` names the
DataWriter — without it no foreign XRCE agent can assign the frame.

The Rust origin `crates/xrce` implements the same wire form
(`WriteDataPayload`/`ReadDataPayload`/`DataPayload`). Both sides are
measured against each other: `crates/endpoint-e2e/tests/xrce_write_framing.rs`
sends the frame that every endpoint SDK builds through
`WriteDataPayload::try_from_submessage`.

### §1.1 WRITE_DATA frame

Message header (`session_id`, `stream_id`, `sequence_nr` LE — DDS-XRCE 1.0
§8.3.2.3) followed by the WRITE_DATA submessage header (id=7, flags, length LE —
§8.3.4), followed by the `BaseObjectRequest` (§7.7.8: `request_id` then
`object_id`, two octets each, most significant first), followed by the
XCDR sample body. Best-effort operation without ClientKey: `session_id ≥ 128`.

The flag byte is `0x01` = E flag (little-endian body) plus `DataFormat`
`FORMAT_DATA` (`0b000`) in bits 1..3 (§8.3.5.8). `FORMAT_SAMPLE`
(`0b001`, flag byte `0x03`) additionally requires a `SampleInfo` in the body,
which this profile does not carry.

The `request_id` is the `sequence_nr` of the same message; §7.7.8 requires of
a RequestId only uniqueness per stream.

### §1.2 Receive path (DATA message)

The endpoint MUST be able to parse DATA submessages (id=9, §8.3.4) that the agent pushes to the
client via the same unwrap path as WRITE_DATA. The
sample body starts behind the `BaseObjectRequest`; a body that is too short for
these four octets is rejected and not returned truncated.

### §1.3 Serial framing (Annex C)

For serial byte-stream transports the endpoint MUST implement the framing of
DDS-XRCE 1.0 Annex C.1: `7E | sourceAddress | remoteAddress |
payloadLength (2, LE) | payload | frameCheck (2, LE)`. Every byte after the
flag that is `0x7E` or `0x7D` is sent as `0x7D` followed by the byte XOR `0x20`
(C.1.2). `frameCheck` is the FCS-16 per RFC 1662 (polynomial
`x^16 + x^12 + x^5 + 1`, reflected `0x8408`, init `0xFFFF`, complemented;
`0x906E` for "123456789") over the addresses, `payloadLength` and payload
(C.1.1.6). `zdw_serial_frame(out, cap, source, remote, payload, n)` frames;
`zdw_serial_deframe` returns payload, `source` and `remote` and rejects a
wrong `frameCheck`, a wrong escape, a `0x7E` before the end of the frame
(C.1.3) and a truncated frame. `zdw_fcs16` is standalone.

## §2 sync send/recv

### §2.1 Frame hook contract

The endpoint is transport-opaque (ADR 0013, invariant 5): it passes a
fully framed, encoded message to a transport filled in by the integrator
(context pointer + `deliver`/`receive` function pointers) and
receives complete frames back. Encode/send and receive/decode run
symmetrically over the same hook.

### §2.2 Poll loop idiom (C89)

The baseline usage is synchronous: the integrator owns the run loop and
calls the receive function itself (poll). The C89 core must not assume
C99/C11 syntax.

## §3 async reader/writer

### §3.1 Event-driven reactor

In addition to the C89 core, a callback-driven, latency-decoupled
alternative to the poll loop MUST exist (C11, no `malloc` in the reactor path):
a reader that binds an `on_sample` callback and dispatches every decoded sample when draining the
transport; a writer with an equivalent
asynchronous write path.

### §3.2 Live transport

The reactor MUST demonstrably work over real datagram I/O (POSIX UDP,
non-blocking), not only over an in-memory queue.

### §3.3 Async deep example

A runnable example (not a stub) MUST show the reactor end to end.

### §3.4 Test obligation: live ping-pong E2E

In addition to the isolated unit/loopback tests from §1–§3, every
language MUST provide a live E2E file following the pattern
`crates/endpoint-e2e/tests/<lang>.rs` (reference: `cpp.rs`):
raw/sync/async modes over a real POSIX socket against the generated
`Ping`/`Pong` codec, so that framing (§1), sync (§2) and async (§3) are proven
in combination — not only in isolation.

## §4 reliable stream

The reliable stream is a standalone component and is specified normatively and completely
in [`reliable-endpoint-1.0`](reliable-endpoint-1.0.md)
(state machine contract §3, wire format §4, test & evidence obligation §5). The
C endpoint MUST implement this contract:

- a C89 state machine (fixed storage, no `malloc`) for sender and
  receiver according to §3, including the constants from §3.1;
- byte-identical HEARTBEAT/ACKNACK wire codecs according to §4;
- a latency-decoupled async writer whose drain task carries the reliable
  sender state (§2) — realized via a wait-free ring plus a
  dedicated drain thread (here: SPSC ring + pthread, C11);
- the complete test and evidence obligation from §5: unit tests, byte golden,
  E2E loss recovery against the Rust reference peer, a latency bench measurement
  (decoupled ring enqueue vs. inline `send()`), and a runnable
  `example_reliable_*`.
