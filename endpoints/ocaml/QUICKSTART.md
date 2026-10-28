<!-- SPDX-License-Identifier: Apache-2.0 -->
# ZeroDDS OCaml endpoint — Quickstart

Two runnable programs show the same sensor-telemetry flow — a publisher frames
typed `Reading { id; value; label }` samples and delivers them; a subscriber
decodes **every field**.

```sh
cd endpoints/ocaml
make example_sync     # subscriber owns the run-loop and polls
make example_async    # subscriber blocks on a mailbox filled by a Thread
```

Both print five decoded readings and `ALL OK`.

## Sync vs async

- **`example_sync`** — `Client.poll` in a loop (non-blocking; `None` when
  empty). The idiom when you own the run-loop.
- **`example_async`** — `AsyncReader.start` spawns a `Thread` that fills a
  Mutex/Condition mailbox; the consumer blocks on `AsyncReader.recv`. The
  idiomatic OCaml stdlib concurrency model — no Lwt, no Async.

## Threads

The wire-core is pure OCaml; `AsyncReader`/`Mailbox` use `Thread`, `Mutex`, and
`Condition`, so every target links `threads.posix`:

```sh
ocamlfind ocamlopt -thread -package threads.posix -linkpkg zerodds.ml example_sync.ml -o bin && ./bin
```

## Transport

`transport` is `{ deliver : bytes -> unit; receive : unit -> bytes option }`.
The examples use the in-memory `MemTransport` (a Mutex-guarded FIFO); a real UDP
or shared-memory link is a drop-in.

## Wire & codegen

Byte-identical to the Rust core (XCDR2, align cap 4, `f32` via
`Int32.bits_of_float`). The same types can be generated from IDL with
`zerodds-idlc generate --lang ocaml` — see
[`docs/specs/zerodds-xcdr2-ocaml-1.0.md`](../../docs/specs/zerodds-xcdr2-ocaml-1.0.md).

## Reliable stream: unannotated types

Optional argument `?unannotated_types` of `Reliable.Receiver.create` and of
`Reliable.acknack_frame`, type `Reliable.unannotated_types = Appendable | Final`,
default `Appendable`. DDS-XRCE 1.0 Annex A types without `@extensibility` are
APPENDABLE by DDS-XTypes 1.3 section 7.3.1.2.1.8 and carry a DHEADER. `Final` writes
the FINAL form of the reference implementation (eProsima).

```ocaml
let r = Reliable.Receiver.create ~unannotated_types:Reliable.Final () in
let frame = Reliable.Receiver.acknack_frame r (Some last_seen) in
(* pure builder *)
let f2 = Reliable.acknack_frame ~unannotated_types:Reliable.Final ak
```

Types covered: `ACKNACK_Payload` (`Appendable`: 17-octet frame, length field 9,
DHEADER = 5; `Final`: 13-octet frame, length field 5). This SDK writes no other
unannotated type (no `Time_t`, no `Property` entries: `CLIENT_Representation` carries
an absent `PropertySeq`). `HEARTBEAT_Payload` stays FINAL in both modes. The parser
accepts both forms in either setting.

## XRCE client: representation timestamps

DDS-XRCE 1.0 contradicts itself on `CLIENT_Representation`. The prose of 7.8.2.1 lists
`Time_t client_timestamp` after `xrce_vendor_id`; the IDL of Annex A and the bytes of
Annex B.1 do not have it. The bytes do not tell the two forms apart, so it is a setting.

- Name `Xrce_client.representation_timestamps = Absent | Present`, default `Absent` (Annex A / B.1 form); `Present` is the prose form.
- Client: `Xrce_client.create ~representation_timestamps:Xrce_client.Present key transport` or `Xrce_client.set_representation_timestamps`; `client_representation` and `connect_default` build the representation with the wall clock as timestamp.
- Encoder: `Xrce_client.encode_client_representation ~version ~vendor_id ~client_key ~session_id ~representation_timestamps:Xrce_client.Present ~timestamp:(sec, nsec) ~unannotated_types:Reliable.Final ()`; `?unannotated_types` is `Reliable.unannotated_types` (default `Appendable`); without `?timestamp` the wall clock is used.
- The agent has to be set the same way (`representation_timestamps:` in its configuration).
- The client reads STATUS_AGENT only as a state change; it does not parse AGENT_Representation.

Layouts (client_key 01 02 03 04, session 0x01, mtu 508, 1518905996 s, 500000000 ns, little-endian):

```
absent (16):             58 52 43 45 01 00 00 00 | 01 02 03 04 | 01 | 00 | fc 01
present, appendable (28): 58 52 43 45 01 00 00 00 | 08 00 00 00 | 8c aa 88 5a 00 65 cd 1d | 01 02 03 04 | 01 | 00 | fc 01
present, final (24):      58 52 43 45 01 00 00 00 | 8c aa 88 5a 00 65 cd 1d | 01 02 03 04 | 01 | 00 | fc 01
```

`Time_t` carries no `@extensibility`, so it is APPENDABLE (DDS-XTypes 1.3 7.3.1.2.1.8): DHEADER u32 = 8.
