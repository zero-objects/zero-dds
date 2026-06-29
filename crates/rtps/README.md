# `zerodds-rtps`

Writer/reader state machines, RTPS submessages, wire-format encoding.
Part of [**ZeroDDS**](../../README.md). Safety class **SAFE** —
`forbid(unsafe_code)`, no_std + alloc.

DDSI-RTPS 2.5 — fully spec-conformant (K3b audit completed 2026-04-28:
121 done / 0 partial / 0 open / 3 n/a).

---

## Quick start (E2E with UDP)

```rust,no_run
use std::net::Ipv4Addr;
use zerodds_rtps::reader::BestEffortReader;
use zerodds_rtps::writer::BestEffortWriter;
use zerodds_rtps::wire_types::{EntityId, GuidPrefix};
use zerodds_transport::Transport;
use zerodds_transport_udp::UdpTransport;

let prefix = GuidPrefix::from_bytes([1; 12]);
let writer_id = EntityId::user_writer_with_key([0x10, 0x20, 0x30]);
let reader_id = EntityId::user_reader_with_key([0xA0, 0xB0, 0xC0]);

let writer_xport = UdpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0)?;
let reader_xport = UdpTransport::bind_v4(Ipv4Addr::LOCALHOST, 0)?;
let dest = reader_xport.local_locator();

let mut writer = BestEffortWriter::new(prefix, writer_id, reader_id);
let reader = BestEffortReader::new(prefix, reader_id);

let datagram = writer.write(b"hello rtps")?;
writer_xport.send(&dest, &datagram)?;

let received = reader_xport.recv()?;
let samples = reader.recv_datagram(&received.data)?;
assert_eq!(samples[0].payload, b"hello rtps");
# Ok::<(), Box<dyn std::error::Error>>(())
```

---

## Modules

| Module | Purpose |
|---|---|
| `error` | `WireError` variants |
| `wire_types` | `Guid`, `EntityId`, `SequenceNumber`, `Locator`, `ProtocolVersion`, `VendorId` |
| `header` | `RtpsHeader` (20B) + `RTPS_MAGIC` |
| `submessage_header` | `SubmessageHeader` (4B) + `SubmessageId` enum |
| `submessages` | DATA, DATA_FRAG, HEARTBEAT, HEARTBEAT_FRAG, ACKNACK, NACK_FRAG, GAP, INFO_TS, INFO_SRC, INFO_DST, INFO_REPLY + SequenceNumberSet |
| `datagram` | encode/decode RTPS messages, ParsedSubmessage iteration |
| `writer` | BestEffortWriter (1:1, stateless) |
| `reader` | BestEffortReader (1:1, with wildcard match) |
| `history_cache` | Ordered `CacheChange` store (BTreeMap) + atomic stats + LockFreeReadHistoryCache |
| `reader_proxy` / `writer_proxy` | Per-endpoint state |
| `reliable_writer` / `reliable_reader` | Reliable state machines, tick-driven |
| `reliable_stateless_writer` | Stateless writer variant for SPDP |
| `fragment_assembler` | Reader-side reassembly with DoS caps |
| `participant_security_info` | PID `0x1005` (DDS-Security 1.2 §7.4.1.6) |
| `message_builder` | OutboundDatagram aggregation per send tick |

## Reliable-Quickstart

```rust,ignore
use core::time::Duration;
use zerodds_rtps::reliable_writer::{ReliableWriter, ReliableWriterConfig};

let mut w = ReliableWriter::new(ReliableWriterConfig {
    guid, vendor_id,
    reader_proxies: vec![proxy],
    max_samples: 1024,
    history_kind: HistoryKind::KeepLast { depth: 32 },
    heartbeat_period: Duration::from_millis(500),
    fragment_size: 1344,
    mtu: 1400,
});

let dgs = w.write(&payload)?;
for dg in dgs { transport.send(&dg.targets, &dg.bytes); }

loop {
    for dg in w.tick(uptime())? { transport.send(&dg.targets, &dg.bytes); }
    if let Some(ack) = transport.recv_acknack() {
        w.handle_acknack(ack.src, ack.base, ack.requested);
    }
}
```

## Lock-Free Read-Path & Per-Slot Mutex

The history-cache module is built lock-free in three layers:

* **Atomic stats** — `HistoryCacheStats` with
  `AtomicUsize`/`AtomicI64` for `len`/`evicted`/`max_sn`/`min_sn`.
  Monitoring threads poll via `cache.stats() -> Arc<HistoryCacheStats>`
  without taking the writer lock.
* **Per-endpoint mutex** — `dcps::user_writers`/`user_readers`
  use `RwLock<BTreeMap<EntityId, Arc<Mutex<Slot>>>>`: a separate mutex
  per writer/reader instead of a global lock.
* **RCU snapshot** — `LockFreeReadHistoryCache` with `&self`
  mutations via `zerodds_foundation::rcu::RcuCell` (copy-on-write
  Arc swap). Reader snapshots live independently of the cache lock.

## Wire-format conformance

DDSI-RTPS 2.5 §8.3 — fully implemented:

* RTPS header (§8.3.3): 20 byte, magic + version + VendorId + GuidPrefix
* Submessage header (§8.3.4): 4 byte, ID + flags + OctetsToNextHeader
* DATA / DATA_FRAG / GAP / HEARTBEAT / HEARTBEAT_FRAG / ACKNACK /
  NACK_FRAG / INFO_TS / INFO_SRC / INFO_DST / INFO_REPLY
* Cross-vendor wire compat byte-identical against Cyclone DDS,
  FastDDS, RTI Connext, OpenSplice (see `internal/interop/`).

## Cross-vendor compatibility

* **RTPS 2.1 with a 0x80 submessage** (Cyclone/FastDDS legacy) —
  HeaderExtension is parsed only from version 2.5 on, before that
  treated as vendor-specific. Regression test
  `rtps_2_1_treats_0x80_as_vendor_specific_not_header_extension`.
* **`fragments_in_submessage > 1`** (RTI bundling) — the decoder
  accepts it; the encoder emits 1 fragment per submessage.
* **HEARTBEAT_FRAG** — decoder ready, encoder not active (regular
  HEARTBEATs are enough for the reader).

## Tests

```bash
cargo test -p zerodds-rtps                     # 647 tests
cargo test -p zerodds-rtps --test reliable_e2e # in-order delivery + loss recovery
cargo test -p zerodds-rtps history_cache       # incl. lock-free + atomic stats
```

E2E tests in `tests/reliable_e2e.rs` cover in-order delivery with
0%/10%/30% simulated packet loss + 10-kB fragmentation.

## Documentation

For a hot-path trace see
[Documentation Trail Station 02 → data-flow](../../documentation/02-architecture/data-flow.md).
