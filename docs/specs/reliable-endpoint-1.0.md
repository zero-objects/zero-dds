# `reliable-endpoint` v1.0 — reliable delivery as an endpoint capability

ZeroDDS vendor spec. Implemented in `crates/xrce/src/reliable.rs` (reference state machine)
and `crates/xrce/src/submessages/acknack.rs` / `heartbeat.rs` (wire codec). Builds on
DDS-XRCE 1.0 §8.4.10/§8.4.11 (reliable streams) and §8.3.2 (stream IDs).

## §1 Decision — reliable delivery lives in the endpoint, not at the hub

The endpoint is the data producer. Reliable delivery at the edge (source → aggregator,
aggregator → bus) only exists if the reliable **writer state** sits in the endpoint:
a history cache until the ACK, a HEARTBEAT announce, ACKNACK reception, retransmit.
Moving this out to a central hub makes the endpoint→hub hop best-effort and
breaks exactly the one hop that must not lose data.

The cut:

- **Hub:** discovery (SPDP/SEDP), QoS matching, routing.
- **Endpoint (mandatory):** reliable *delivery* — a stateful writer and reader. A
  bounded, composable building block (XRCE reliable stream, `stream_id ≥ 128`), not a
  full stack. Does not conflict with a thin endpoint: discovery stays outside.

## §2 Two axes, one component

Async write and reliability are the same component: the **drain task** of the async writer
holds the reliable state.

The motive for async write is not waiting for ACKNACK — it is the **syscall latency
in a tight producer loop**. A `sendto` is a kernel transition (hundreds of ns to µs);
on a multi-core aggregator that produces at ns intervals, that is a factor of 100–1000 too
expensive. The producer must never enter the kernel.

```
producer   →  enqueue(sample)               // wait-free, ns, no kernel
drain-task →  submit(sample) → seq, history  // reliable writer state
           →  sendmmsg(WRITE_DATA…)           // batched, one syscall per N
           →  HEARTBEAT (periodic)
           ←  ACKNACK  →  retransmit(missing) from history
           →  prune(acked)
Backpressure = history/window full (producer decides locally: drop / spin / count)
```

An inline "AsyncWriter" that calls the transport deliver directly on the
producer thread — one syscall, no ring, no drain, no history — delivers
neither latency decoupling nor reliable state and does not conform to this component.

## §3 Canonical state machine contract

Every language mirrors this surface (idiomatic names, identical semantics).
RFC-1982 16-bit sequence numbers.

### §3.1 Constants (binding)

| Constant | Value | Rationale |
|---|---|---|
| `HEARTBEAT_PERIOD` | 500 ms | spec recommends 100 ms; 500 ms is conservative without a Tx pacing layer |
| `SENDER_WINDOW` | 16 | corresponds to the 16-bit ACKNACK bitmap |
| `RECEIVER_BUFFER` | 64 | DoS limit against a reorder flood |
| `MAX_PAYLOAD` | 65535 | u16 submessage length limit |
| reliable stream id | bit 7 set (`≥ 128`) | spec §8.3.2 |

### §3.2 Sender

- `submit(payload) -> seq` — errors `PayloadTooLarge` (`len > MAX_PAYLOAD`) and
  `WindowFull` (`in_flight ≥ SENDER_WINDOW`); assigns a monotonic `seq`, buffers in
  `in_flight: seq→payload`.
- `pending_heartbeat(now) -> HEARTBEAT?` — `None` if `in_flight` is empty; fires on the
  first call, after that only after `HEARTBEAT_PERIOD`; body `{ first_unacked, last_unacked, stream_id }`.
- `recv_acknack(payload)` — `base = first_unacked`; everything `< base` (RFC-1982) is
  acknowledged and is removed; in the window `[base, base+16)` a set bit means
  missing (keep), a cleared bit means acked (remove).
- `get_in_flight(seq)` — retransmit lookup.

### §3.3 Receiver

- `recv_data(seq, payload)` — a duplicate (`seq < expected`) is silently discarded; already
  buffered is a no-op; error `BufferFull` at `RECEIVER_BUFFER`; otherwise into the
  out-of-order buffer `received: seq→payload`.
- `drain_in_order() -> [(seq, payload)]` — delivers without gaps from `expected`, advances
  `expected`.
- `pending_acknack(hint_last_seen) -> ACKNACK` — bitmap of the missing slots in
  `[expected, expected+16)`; slots beyond `hint_last_seen` are not marked as missing.
- `reset()` — complete state clear (after a RESET submessage).

## §4 Wire format (byte-golden)

`HEARTBEAT` and `ACKNACK` are byte-identical to the Rust reference:

- `AckNack { first_unacked_seq_num: i16, nack_bitmap: [u8;2] LE, stream_id: u8 }` —
  XRCE submessage ID `0x0A`, flags `0x01` (only E flag = LE set).
- `Heartbeat { first_unacked_seq_nr: i16, last_unacked_seq_nr: i16, stream_id: u8 }` —
  XRCE submessage ID `0x0B`, flags `0x01` (only E flag = LE set).
- `WriteData` (payload submessage of the reliable stream) — XRCE submessage ID `0x07`,
  flags `0x01` (E flag = LE plus `DataFormat` `FORMAT_DATA` = `0b000` in bits
  1..3, spec §8.3.5.8). The submessage body begins with the `BaseObjectRequest`
  (§7.7.8: `request_id` then `object_id`, two octets each, most significant first)
  and carries the sample after it; the `request_id` is the RFC-1982 sequence number
  of the sample, the `object_id` names the target DataWriter.
- Every submessage carries the XRCE submessage header (ID, flags, len LE) + the reliable
  stream ID in the session/stream row (`stream_id ≥ 128`, bit 7 set, spec §8.3.2).

Golden sources: `golden_heartbeat_le.bin` / `golden_acknack_le.bin` of the C SDK
(`endpoints/c/test/test_reliable.c`) and `crates/xrce`. Every language asserts against them.

## §5 Test & evidence obligation (per language)

1. **Unit** — mirrors the reference tests: monotonic seq, window-full, heartbeat
   first/silence/empty, acknack-clear, reorder, duplicate-drop, buffer-full,
   pending-acknack bitmap, reset.
2. **Byte-golden** — HEARTBEAT + ACKNACK identical to `golden_*.bin`.
3. **E2E loss recovery** — live against the Rust reliable peer (`zerodds-endpoint-e2e`),
   `stream_id ≥ 128`, with injected drop (every n-th datagram / `netem drop`); assert:
   **all** samples delivered in order without gaps despite loss.
4. **Latency bench** — producer `write→return` in the async-decoupled path vs. inline
   deliver; one measured value (ns/µs) that shows the decoupling.
5. **Example** — a runnable `example_reliable_*` (aggregator sends N reliable
   samples, reader prints the gap-free sequence). Generated, not a stub.

No false green: every test runs on a real toolchain, a loud skip only when the
toolchain is missing.

## §6 Out of scope (separate rounds)

- Fragmentation (FRAGMENT submessage, payload > 64 KiB).
- RESET handshake over the live connection.
- Kernel bypass drain (io_uring / AF_XDP) — an optimisation after the `sendmmsg` baseline.
