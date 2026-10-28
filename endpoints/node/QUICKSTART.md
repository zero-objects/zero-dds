<!-- SPDX-License-Identifier: Apache-2.0 -->
# ZeroDDS Node endpoint — Quickstart

Two runnable programs show the same sensor-telemetry flow — a publisher frames
typed `Reading { id, value, label }` samples and delivers them; a subscriber
decodes **every field**.

```sh
cd endpoints/node
node example_sync.js     # subscriber polls
node example_async.js    # subscriber iterates the async stream (`for await`)
```

Both print five decoded readings and `ALL OK`. No dependencies — just Node 18+.

## Sync vs async

- **`example_sync`** — `Client.poll()` in a loop (non-blocking; `null` when
  empty). The idiom when you own the run-loop.
- **`example_async`** — `AsyncReader.stream()` is an async iterator; `for await
  (const body of ...)` yields decoded frames, `close()` stops it. The idiomatic
  Node async model.

## Transport

`Transport` is `{ deliver, receive }`. The examples use the in-memory
`MemTransport`; a real UDP (`dgram`) or shared-memory link is a drop-in.
`udp.js` provides a `UdpTransport` that carries the sync/async loopback over a
real socket:

```sh
node example_ping_pong.js sync  <peer-port>   # Client.poll() owns the loop
node example_ping_pong.js async <peer-port>   # AsyncReader.stream() drains it
```

## Reliable stream (RFC-1982 HEARTBEAT / ACKNACK / retransmit)

`reliable.js` adds a genuine reliable writer (DDS-XRCE §8.4.10/§8.4.11),
byte-identical to `endpoints/c` and `endpoints/java`: a 16-bit RFC-1982 window,
`WRITE_DATA` / `HEARTBEAT` / `ACKNACK` frames, and ACKNACK-driven retransmit.
`AsyncReliableWriter` is the async-decoupled sender — the producer `submit(...)`
is a Promise-based enqueue with backpressure on overflow (no sample dropped, no
producer hang); a drain loop owns the send window and does the I/O.

```sh
node example_reliable.js <peer-port> [N]   # submit N, recover loss, drain window
node reliable_selftest.js                  # unit + byte-golden (prints ALL OK)
```

Unannotated types: the option `unannotatedTypes` takes `'appendable'`
(default) or `'final'` (`UNANNOTATED_APPENDABLE` / `UNANNOTATED_FINAL`).
DDS-XRCE 1.0 Annex A types without `@extensibility` are APPENDABLE
(DDS-XTypes 1.3 section 7.3.1.2.1.8) and carry a DHEADER; `'final'` writes the
FINAL form of the reference implementation (eProsima). The SDK writes one such
type, `ACKNACK_Payload`: 17-octet frame (length field 9) with `'appendable'`,
13-octet frame (length field 5) with `'final'`. `HEARTBEAT_Payload` is FINAL in
both settings. `parseAckNack` accepts both forms in either setting.

```js
const rx = new ReliableReceiver(STREAM_RELIABLE, { unannotatedTypes: 'final' });
rx.acknackFrame(null);                       // 13 octets
acknackFrame(body, 'final');                 // frame builder, explicit mode
```

Live loss-recovery + baseline + unit/golden run against the shared Rust peer in
`crates/endpoint-e2e/tests/node_reliable.rs`; sync/async ping-pong in `node.rs`.

## XRCE client: representation timestamps

Option `representationTimestamps` with the values `'absent'` (default) and
`'present'`. DDS-XRCE 1.0 contradicts itself on `CLIENT_Representation`: the
prose of section 7.8.2.1 lists `Time_t client_timestamp` after
`xrce_vendor_id`; the IDL of Annex A and the bytes of Annex B.1 do not have it.
`'absent'` writes the Annex A form (what the SDK wrote before). `'present'`
writes the prose form: seconds `i32` and nanoseconds `u32`, little endian.
`Time_t` has no `@extensibility`, so it is APPENDABLE (DDS-XTypes 1.3 section
7.3.1.2.1.8) with `unannotatedTypes: 'appendable'` (default): DHEADER = 8, 12
octets. With `unannotatedTypes: 'final'` there is no DHEADER, 8 octets.

The agent has to be set the same way (`representation_timestamps: absent |
present` in its configuration).

```js
const client = new XrceClient(key, { representationTimestamps: 'present' });
const rep = client.clientRepresentation();      // Time_t = wall clock at the call
encodeClientRepresentation(ver, vendor, key, 1, 508,
  { representationTimestamps: 'present', seconds: 1518905996, nanoseconds: 500000000 });
```

Bytes (client_key 01 02 03 04, session_id 01, version 01 00, vendor 00 00,
requested_mtu 508, seconds 1518905996, nanoseconds 500000000):

    absent (16):              58 52 43 45 01 00 00 00 | 01 02 03 04 | 01 | 00 | fc 01
    present, appendable (28): 58 52 43 45 01 00 00 00 | 08 00 00 00 | 8c aa 88 5a 00 65 cd 1d | 01 02 03 04 | 01 | 00 | fc 01
    present, final (24):      58 52 43 45 01 00 00 00 | 8c aa 88 5a 00 65 cd 1d | 01 02 03 04 | 01 | 00 | fc 01

The SDK does not read `AGENT_Representation`; `STATUS_AGENT` is a state change
only.

## Wire & codegen

Byte-identical to the Rust core (XCDR2, align cap 4, `f32` via
`Buffer.writeFloatLE`, `u64` via `BigInt`). IDL types are generated as TypeScript
with `zerodds-idlc generate --lang typescript` and run on Node — see
[`docs/specs/zerodds-xcdr2-node-1.0.md`](../../docs/specs/zerodds-xcdr2-node-1.0.md).
