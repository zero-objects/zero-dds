# Zero-Copy / FlatData — Recherche-Quellen

Ausgangslage: **es gibt keine OMG-Spec fuer Zero-Copy-DDS**. RTI hat
"FlatData" als Vendor-Erweiterung, Eclipse Iceoryx hat einen eigenen
Standard, ROS-2 hat REP-2007-Loaning. Wir definieren
`zerodds-flatdata-1.0` als Vendor-Spec mit klaren Bezugspunkten.

## Primaerquellen

### Iceoryx2 (Eclipse, Apache-2.0)

- **URL:** <https://github.com/eclipse-iceoryx/iceoryx2>
- **Spec-Doku:** <https://iceoryx.io/v2.0.5/getting-started/overview/>
- **Architektur:** zero-copy IPC ueber POSIX-SHM, Lock-free-Queues,
  POSH (Power-of-Safety-Habituation) als Zertifizierungs-fertiger
  Layer. Pure-Rust ab v2.
- **Relevanz:** Industry-Standard fuer Same-Host-Zero-Copy in
  Robotics/Avionics. ROS-2 Iceoryx-Plugin ist Pionier-Implementation.

### RTI Connext FlatData

- **URL:** <https://community.rti.com/kb/flatdata-and-zerocopy-examples>
- **Whitepaper:** "Achieving Zero-Copy Data Transfer in DDS" (RTI,
  2021).
- **Konzept:** **FlatStruct<T>** als alignment-stabiles in-place-
  encoded Layout. Writer reserviert SHM-Slot, schreibt direkt rein,
  publish'd Slot-Pointer (16-byte). Same-Host-Reader mmap't den Slot
  ohne Copy.
- **Relevanz:** Definitive Industry-Reference fuer DDS-spezifische
  Zero-Copy. Wir folgen dem Pattern, aber mit Rust-Type-System statt
  C++-Templates.

### ROS-2 Loaned-Messages (REP)

- **REP-2007:** Reference RMW. §"Loaned Messages": optional, RMW-
  Implementation kann pre-allocated Buffers loaning + commit
  bereitstellen.
- **rmw API:** `rmw_borrow_loaned_message`, `rmw_publish_loaned_message`,
  `rmw_return_loaned_message`.
- **Relevanz:** Unsere `rmw_zerodds`-Shim hat diese Funktionen
  bereits stub-mässig. Vollwertige Loaned-Messages =
  Zero-Copy-Pfad.

### Cyclone DDS Zero-Copy via Iceoryx

- **Doku:** <https://cyclonedds.io/docs/cyclonedds/latest/shared_memory.html>
- **Pattern:** Cyclone delegiert SHM komplett an Iceoryx; integriert
  via "Iceoryx-Subscriber-as-DDS-Reader"-Adapter.
- **Relevanz:** Bestaetigt Industry-Pattern: SHM-Layer ist
  separat von DDS-Wire-Stack.

## Architektur-Optionen

### Option A: Iceoryx2-Integration
- Cargo-Dep `iceoryx2 = "0.5"` (pure-Rust ab v0.5).
- ZeroDDS-DataWriter mit Same-Host-Reader → Iceoryx2-Pub/Sub.
- Cross-Host bleibt UDP/RTPS.
- **Pro:** Industry-Standard, fertige Implementation, POSH-Sicherheit.
- **Con:** External Dep mit eigener Lifecycle/Konfiguration. Iceoryx2
  ist 2026 noch unter Stabilization (API-changes).

### Option B: Eigener PosixShmTransport
- Bestehender `crates/transport-shm` ist schon da.
- DataWriter::loan / commit auf PosixShmTransport-Slots.
- **Pro:** keine externe Dep, full Control.
- **Con:** wir reimplementieren was Iceoryx schon hat (Lock-free-
  Ringbuffer, Schema-Versioning, Multi-Reader-Fairness).

### Option C: Hybrid — eigene Bridge zu Iceoryx2 als optional Feature
- Default-Build hat `transport-shm` (eigen).
- `--features iceoryx2-bridge` aktiviert Iceoryx2-Adapter.
- **Pro:** Caller waehlt Komplexitaet.
- **Con:** doppelte Test-Surface.

## Empfehlung fuer zerodds-flatdata-1.0

**Option B (eigener PosixShmTransport)** als Default-Pfad:
- bestehender `crates/transport-shm` schon vorhanden, vollwertig
  implementiert (1678 LOC).
- Einfacher Code-Pfad: DataWriter loan'd Slot direkt vom
  PosixShmTransport, der Reader-Pfad kennt SHM via SEDP-Locator.
- Keine externe Dep — bleibt Pure-Rust workspace.

**Option C (Iceoryx2-Bridge)** als optional Feature `iceoryx2-bridge`
fuer Caller, die Iceoryx-Oekosystem nutzen wollen.

## Wire-Spezifikation

### FlatStruct-Layout

Plain-old-data Rust-Struct mit:
- `#[repr(C, align(N))]` mit N als groessten primitiven Type-Alignment.
- Nur `Copy`-Felder.
- Keine Pointer / Vec / String — flache Bytes.
- `#[derive(FlatStruct)]`-Macro generiert `as_bytes()` / `from_bytes_unchecked()`.

### SHM-Slot-Layout

```
+--------- SHM-Slot (size = sample_size + header) ---------+
| 4 byte | u32 | Sequence-Number (writer-lokal)            |
| 4 byte | u32 | Sample-Size (= sizeof(FlatStruct<T>))     |
| 4 byte | u32 | Reader-Mask (Bitmap: welche Reader gelesen)|
| 4 byte | pad |                                           |
| N byte | T   | FlatStruct-Daten                          |
+----------------------------------------------------------+
```

### Discovery-PID

Neue Vendor-PID `0x8001 PID_SHM_LOCATOR`:
- `value = (segment_path_string, slot_count: u32, slot_size: u32)`
- Writer publisht im SEDP-Discovery-Sample.
- Reader auf demselben Host (uid_t-Match) attached an SHM-Segment.

### Wire-Pfad

1. Writer reserviert Slot via PosixShmTransport.
2. Writer schreibt FlatStruct direkt in den Slot (memory-mapped).
3. Writer schickt **DDS-DATA-Submessage mit nur 4-byte Slot-Index**
   (SHM-Locator als Inline-QoS PID_SHM_LOCATOR statt voller Payload).
4. Same-Host-Reader liest Slot direkt; Cross-Host-Reader bekommt
   konventionelle UDP-DATA mit voller Payload (Writer schickt parallel).

## Sicherheit & Lifetime

- **Slot-Refcount:** SHM-Slots haben Lifetime-Counter; Writer kann erst
  ueberschreiben wenn alle matched Reader gelesen haben (oder Timeout).
- **Schema-Versioning:** TypeIdentifier in SHM-Header; Reader mit
  ungleichem Type-Hash droppen den Slot.
- **POSIX-Permissions:** SHM-Segment ist mode=0600, owner-only.

## Performance-Targets

- 1 KiB Sample, 1 Mio Samples: Latenz P99 < 5 µs (gegen UDP-loopback ~30 µs).
- Throughput: ~1 GB/s pro Writer (Memcpy-bound).
- 0 Heap-Allokation pro write (Slot-Reuse).
