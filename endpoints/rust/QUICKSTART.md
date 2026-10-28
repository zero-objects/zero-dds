<!-- SPDX-License-Identifier: Apache-2.0 -->
# ZeroDDS Rust endpoint — Quickstart

Two runnable programs show the same sensor-telemetry flow — a publisher frames
typed `Reading { id, value, label }` samples and delivers them; a subscriber
decodes **every field**.

```sh
cargo run -p zerodds-endpoint-rust --example example_sync    # subscriber polls
cargo run -p zerodds-endpoint-rust --example example_async   # subscriber blocks on a channel
```

Both print five decoded readings and `ALL OK`.

## Sync vs async

- **`example_sync`** — `Client::poll` in a loop (non-blocking; `None` when
  empty). The idiom when you own the run-loop.
- **`example_async`** — `AsyncReader::start` spawns a std thread that forwards
  decoded bodies over an `mpsc` channel; the consumer blocks on `recv`. The
  idiomatic std concurrency model — no async runtime dependency.

## Transport

`MemTransport` is an in-memory FIFO (a shared `VecDeque`); a real UDP or serial
link is a drop-in. XRCE WRITE_DATA framing is `xrce_write_frame` /
`xrce_read_frame`.

## Wire & codegen

The codec is the reference `zerodds-wire` runtime itself (`Writer` /
`Reader`, XCDR2, align cap 4) — so the bytes are byte-identical by
construction. IDL types are generated with `zerodds-idlc generate --lang rust` (a full IDL4
DataType codegen: `#[derive(DdsType)]`, enum/union/typedef) — see
[`docs/specs/zerodds-xcdr2-rust-1.0.md`](../../docs/specs/zerodds-xcdr2-rust-1.0.md).

## Unannotated types (DDS-XRCE 1.0 Annex A)

`ACKNACK_Payload` carries no `@extensibility` in Annex A and is APPENDABLE by
DDS-XTypes 1.3 section 7.3.1.2.1.8: the ACKNACK body is 9 octets (DHEADER `5`,
then the 5 octets of the members), the frame 17 octets. This is the only
unannotated type `reliable.rs` writes. The setting `UnannotatedTypes`
(`Appendable`, default; `Final`) selects the form per receiver:

```rust
use zerodds_endpoint_rust::reliable::{ReliableReceiver, UnannotatedTypes, acknack_frame_with};

let rx = ReliableReceiver::with_unannotated_types(UnannotatedTypes::Final);
let frame = rx.acknack_frame(1); // 13 octets: the form of the reference implementation
// pure builder: acknack_frame_with(ack, msg_seq, UnannotatedTypes::Final);
// acknack_frame(ack, msg_seq) means Appendable.
```

`parse_acknack` accepts both forms in either setting. `HEARTBEAT_Payload`
stays FINAL in both settings. `XrceClient` in `xrce_client.rs` is an alias of
`crates/xrce-client`; its setting is `zerodds_xrce::encoding::UnannotatedTypes`.

## Representation timestamps (DDS-XRCE 1.0 7.8.2.1)

The prose of 7.8.2.1 lists `Time_t client_timestamp` in `CLIENT_Representation`
after `xrce_vendor_id`; the IDL of Annex A and the bytes of Annex B.1 do not
have it. The two forms cannot be told apart by their bytes, so it is a setting:
`zerodds_xrce::encoding::RepresentationTimestamps::Absent` (default, Annex A
form, 16 octets) or `RepresentationTimestamps::Present` (prose form). The
Rust XRCE client (`src/xrce_client.rs` is an alias of `crates/xrce-client`) sets
it with `XrceClient::set_representation_timestamps`:

```rust
use zerodds_xrce::encoding::RepresentationTimestamps;

let mut client = XrceClient::new(key, 0x01, transport);
client.set_representation_timestamps(RepresentationTimestamps::Present);
```

The agent has to be set the same way (`representation_timestamps: absent |
present` in its configuration).

`Time_t` carries no `@extensibility` and is APPENDABLE (DDS-XTypes 1.3
7.3.1.2.1.8): DHEADER `8`, seconds `i32`, nanoseconds `u32`, 12 octets. With the
unannotated-types setting FINAL there is no DHEADER (8 octets). Layouts
(client_key `01 02 03 04`, session_id `01`, requested_mtu 508, seconds
1518905996, nanoseconds 500000000, little endian):

```
absent (16):               58 52 43 45 01 00 00 00 | 01 02 03 04 | 01 | 00 | fc 01
present, appendable (28):  58 52 43 45 01 00 00 00 | 08 00 00 00 | 8c aa 88 5a 00 65 cd 1d | 01 02 03 04 | 01 | 00 | fc 01
present, final (24):       58 52 43 45 01 00 00 00 | 8c aa 88 5a 00 65 cd 1d | 01 02 03 04 | 01 | 00 | fc 01
```
