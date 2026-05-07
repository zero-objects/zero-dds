# 02 – Architecture

ZeroDDS is layered. This station gives you the bird's-eye view;
deeper internal-developer documentation lives in
`docs/architecture/` (internal repo only — German,
contributor-focused).

## Layers

```
┌─────────────────────────────────────────────────────────────┐
│  Bindings              C │ C++ │ C# │ Java │ Python │ TS    │
│                       (rs · sys · cpp · cs · java · py · ts)│
├─────────────────────────────────────────────────────────────┤
│  DCPS API              dcps                                 │
│  (Pub/Sub, QoS, Topics, Listeners, Discovery orchestration) │
├─────────────────────────────────────────────────────────────┤
│  Bridges               coap · websocket · mqtt · amqp ·     │
│                        grpc · ros2-rmw · opcua · soap       │
├─────────────────────────────────────────────────────────────┤
│  Discovery + Wire      discovery (SPDP/SEDP) · rtps         │
│                                                             │
│  Security              security · security-pki · -crypto ·  │
│                        -permissions · -rtps · -keyexchange  │
├─────────────────────────────────────────────────────────────┤
│  Encoding              cdr · idl · types · qos              │
├─────────────────────────────────────────────────────────────┤
│  Transport             transport-{udp,tcp,shm,uds,tsn}      │
├─────────────────────────────────────────────────────────────┤
│  Foundation            buffer pools · CRC · RCU cells ·     │
│                        observability · rt-linux             │
└─────────────────────────────────────────────────────────────┘
```

A request flows top-down on the publisher side and bottom-up on
the subscriber side. The hot path is `dcps → rtps → transport-udp`;
everything else (bridges, security, observability) is opt-in
indirection.

## Process model

A single `DcpsRuntime` owns:

- One UDP socket pair per discovery channel (SPDP multicast +
  unicast, plus user data unicast).
- An async event-loop thread that pumps inbound datagrams,
  handles SPDP/SEDP, and ticks the reliable writer/reader state
  machines.
- A registry of user writers and readers (`Arc<RwLock<...>>` —
  per-slot `Mutex` so two writers on different topics never
  serialize).

User code calls `register_user_writer` / `register_user_reader`
to attach endpoints, and `write_user_sample` / receives via an
`mpsc::Receiver` to push/pull samples. The hot-path on
`write_user_sample` for ≤ 1.5 KiB samples avoids heap allocation
entirely.

## Wire stack

Every byte that leaves a participant goes through:

```
sample bytes
    │  user-payload encap (4 B): 00 07 00 00  = XCDR2 LE
    ▼
┌──────────────────────────────────────┐
│ DATA / DATA_FRAG submessage          │
│ (writer-eid, reader-eid, SN, payload)│
└──────┬───────────────────────────────┘
       │  (potentially N fragments, each in its own DATA_FRAG)
       ▼
┌──────────────────────────────────────┐
│ RTPS message: header + submessages   │
│ "RTPS" │ ver │ vendor │ guid-prefix  │
└──────┬───────────────────────────────┘
       │ (optional) Security: SRTPS-wrap
       │ §7.4.6.6, header AAD, encrypted body
       ▼
┌──────────────────────────────────────┐
│ UDP datagram → unicast or multicast  │
└──────────────────────────────────────┘
```

The exact wire format is [DDSI-RTPS 2.5][rtps]; ZeroDDS is
byte-identical to Cyclone DDS, FastDDS, RTI Connext, and
OpenSplice.

## Crate map

For a per-crate README and dependency graph see
[components.md](components.md).

## Data flow on `write_user_sample`

Worth a chapter of its own — see [data-flow.md](data-flow.md).

## Concurrency model

- Per-slot `Mutex` on every user-writer / user-reader. Two
  writers on different topics never serialize.
- Atomic stats on the history cache — monitoring
  threads poll `Arc<HistoryCacheStats>` without taking any lock.
- Optional lock-free read cache — opt-in
  `LockFreeReadHistoryCache` for read-heavy paths.

## Real-time profile

`crates/rt-linux` (UNSAFE-FFI, isolated) wraps Linux
`sched_setattr(2)` for SCHED_FIFO / SCHED_RR / SCHED_DEADLINE plus
CPU pinning. See `docs/REALTIME_DEPLOYMENT.md` (internal repo
only) for the full kernel-tuning playbook.

## PDF

```bash
make -C documentation pdf-arch
# documentation/dist/architecture.pdf
```

The PDF is built from
[`architecture.tex`](architecture.tex) — the same content as
this trail station, formatted for print and offline review.

## Next

- [03 Configuration](../03-configuration/README.md) — dial in the
  knobs you just learned about.
- `docs/architecture/` (internal repo only — German) — go deep.

[rtps]: https://www.omg.org/spec/DDSI-RTPS/2.5/
