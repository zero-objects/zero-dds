# `zerodds-rtps`

Writer/Reader State-Machines, RTPS-Submessages, Wire-Format-Encoding.
Teil von [**ZeroDDS**](../../README.md). Safety-Klasse **SAFE** —
`forbid(unsafe_code)`, no_std + alloc.

DDSI-RTPS 2.5 — voll spec-konform (K3b-Audit abgeschlossen 2026-04-28:
121 done / 0 partial / 0 open / 3 n/a).

---

## Quick Start (E2E mit UDP)

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

## Module

| Modul | Zweck |
|---|---|
| `error` | `WireError`-Varianten |
| `wire_types` | `Guid`, `EntityId`, `SequenceNumber`, `Locator`, `ProtocolVersion`, `VendorId` |
| `header` | `RtpsHeader` (20B) + `RTPS_MAGIC` |
| `submessage_header` | `SubmessageHeader` (4B) + `SubmessageId`-Enum |
| `submessages` | DATA, DATA_FRAG, HEARTBEAT, HEARTBEAT_FRAG, ACKNACK, NACK_FRAG, GAP, INFO_TS, INFO_SRC, INFO_DST, INFO_REPLY + SequenceNumberSet |
| `datagram` | encode/decode RTPS-Messages, ParsedSubmessage-Iteration |
| `writer` | BestEffortWriter (1:1, stateless) |
| `reader` | BestEffortReader (1:1, mit Wildcard-Match) |
| `history_cache` | Geordnete `CacheChange`-Ablage (BTreeMap) + atomare Stats + LockFreeReadHistoryCache |
| `reader_proxy` / `writer_proxy` | Per-Endpoint State |
| `reliable_writer` / `reliable_reader` | Reliable State-Machines, tick-getrieben |
| `reliable_stateless_writer` | Stateless-Writer-Variante fuer SPDP |
| `fragment_assembler` | Reader-seitige Reassembly mit DoS-Caps |
| `participant_security_info` | PID `0x1005` (DDS-Security 1.2 §7.4.1.6) |
| `message_builder` | OutboundDatagram-Aggregation pro Send-Tick |

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

Das History-Cache-Modul ist in drei Schichten lock-free aufgebaut:

* **Atomic Stats** — `HistoryCacheStats` mit
  `AtomicUsize`/`AtomicI64` für `len`/`evicted`/`max_sn`/`min_sn`.
  Monitoring-Threads pollen via `cache.stats() -> Arc<HistoryCacheStats>`
  ohne den Writer-Lock zu nehmen.
* **Per-Endpoint-Mutex** — `dcps::user_writers`/`user_readers`
  nutzen `RwLock<BTreeMap<EntityId, Arc<Mutex<Slot>>>>`: pro
  Writer/Reader ein eigener Mutex statt globalem Lock.
* **RCU-Snapshot** — `LockFreeReadHistoryCache` mit `&self`-
  Mutationen via `zerodds_foundation::rcu::RcuCell` (Copy-on-Write
  Arc-swap). Reader-Snapshots leben unabhängig vom Cache-Lock.

## Wire-Format-Konformitaet

DDSI-RTPS 2.5 §8.3 — voll umgesetzt:

* RTPS-Header (§8.3.3): 20 Byte, Magic + Version + VendorId + GuidPrefix
* Submessage-Header (§8.3.4): 4 Byte, ID + Flags + OctetsToNextHeader
* DATA / DATA_FRAG / GAP / HEARTBEAT / HEARTBEAT_FRAG / ACKNACK /
  NACK_FRAG / INFO_TS / INFO_SRC / INFO_DST / INFO_REPLY
* Cross-Vendor-Wire-Compat byte-identisch gegen Cyclone DDS,
  FastDDS, RTI Connext, OpenSplice (siehe `docs/interop/`).

## Cross-Vendor-Compatibility

* **RTPS 2.1 mit 0x80-Submessage** (Cyclone/FastDDS-Legacy) —
  HeaderExtension wird nur ab Version 2.5 geparst, davor als
  Vendor-Specific behandelt. Regression-Test
  `rtps_2_1_treats_0x80_as_vendor_specific_not_header_extension`.
* **`fragments_in_submessage > 1`** (RTI-Bundling) — Decoder
  akzeptiert; Encoder emittiert 1 Fragment pro Submessage.
* **HEARTBEAT_FRAG** — Decoder bereit, Encoder nicht aktiv (regulaere
  HEARTBEATs reichen dem Reader).

## Tests

```bash
cargo test -p zerodds-rtps                     # 647 Tests
cargo test -p zerodds-rtps --test reliable_e2e # In-Order-Delivery + Loss-Recovery
cargo test -p zerodds-rtps history_cache       # incl. lock-free + atomic stats
```

E2E-Tests in `tests/reliable_e2e.rs` decken In-Order-Delivery
mit 0%/10%/30% simuliertem Packet-Loss + 10-kB-Fragmentation
ab.

## Documentation

Fuer einen Hot-Path-Trace siehe
[Documentation Trail Station 02 → data-flow](../../documentation/02-architecture/data-flow.md).
