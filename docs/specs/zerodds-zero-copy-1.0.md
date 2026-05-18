# ZeroDDS Zero-Copy 1.0

> Status: Draft v0.1 (2026-05-17)
> Scope: Datenpfad-Copy-Inventory + Reduktions-Strategie
> Implementiert: teils (flatdata, transport-shm, Loan-API-Skeleton)
> Vendor-spezifisch (kein OMG-Normativ)

## 1 Zweck

Diese Spec inventarisiert alle **Kopier-Operationen** auf dem Sample-Datenpfad
zwischen User-Application und Wire, klassifiziert sie nach
Reduzierbarkeit, und definiert die Reduktions-Architektur fuer
ZeroDDS. Ziel: jedes Copy hat eine dokumentierte Begruendung
(Spec-Pflicht / Implementierung / Reduzierbar) und einen
benannten Migrations-Pfad.

## 2 Daten-Pfad — Write (User → Wire)

| # | Stelle | Crate / Datei | Operation | Klasse |
|---:|---|---|---|---|
| W1 | C-FFI `zerodds_writer_write` | `zerodds-c-api/lib.rs:334` | `slice::from_raw_parts(*const u8, len).to_vec()` | Ownership-Transition |
| W2 | DCPS `write_user_sample` (≤ 1.5 kB) | `dcps/runtime.rs:2528` | `PoolBuffer::extend(encap, payload)` | Wire-Framing |
| W2' | DCPS `write_user_sample` (> 1.5 kB) | `dcps/runtime.rs:2550` | `Vec::with_capacity(total) + extend_from_slice × 2` | Wire-Framing |
| W3 | RTPS `MessageBuilder::build_datagram` | `rtps/message_builder.rs` | Submessage-Build → `OutboundDatagram { bytes: Vec<u8>, … }` | Datagram-Konstruktion |
| W4 | Security `secure_outbound_bytes` | `dcps/secure_outbound.rs` | sign/encrypt + Tag-Append | Spec-Pflicht (Security on) |
| W5 | Transport-UDP `send` | `transport-udp/udp_transport.rs:269` | `socket.send_to(data, addr)` | Kernel-Socket |

**Total: 4–5 User-Space-Copies + 1 Kernel-Copy pro Sample** auf dem
Cross-Host-UDP-Pfad.

## 3 Daten-Pfad — Read (Wire → User)

| # | Stelle | Crate / Datei | Operation | Klasse |
|---:|---|---|---|---|
| R1 | Transport-UDP `recv` | `transport-udp/udp_transport.rs:282` | `recv_from(&mut buf)` + `buf[..len].to_vec()` | Owned-Return |
| R2 | DCPS `handle_user_datagram` | `dcps/runtime.rs:3753` | `decode_datagram` → Submessage-Slices (borrowed) | — (kein Copy) |
| R3 | RTPS HistoryCache push | `rtps/history_cache.rs` | `Arc<[u8]>` push | — (Shared-Ownership) |
| R4 | DCPS `strip_user_encap` | `dcps/runtime.rs:3749` | `payload[off..].to_vec()` | Encap-Strip |
| R5 | DCPS mpsc-Send | `dcps/runtime.rs:2905` | `sample_tx.send(UserSample::Alive { payload: Vec<u8> })` | Move-only (kein Copy) |
| R6 | C-FFI `zerodds_reader_take` | `zerodds-c-api/lib.rs:575` | `bs.into_boxed_slice() → Box::into_raw` | Leak/Move (kein Copy) |

**Total: 2 User-Space-Copies + 1 Kernel-Copy pro Sample**.

## 4 Bestehende Zero-Copy-Infrastruktur

| Komponente | Crate | Status | Spec |
|---|---|---|---|
| `unsafe trait FlatStruct` | `crates/flatdata/` | live, feature-gated | `zerodds-flatdata-1.0.md` |
| POSIX SHM (`shm_open` + `mmap`) | `crates/flatdata/posix.rs` | live | `zerodds-flatdata-1.0.md` §5 |
| iceoryx2-Backend | `crates/flatdata/iceoryx.rs` | feature `iceoryx2-bridge` | `zerodds-flatdata-1.0.md` §6 |
| SHM-Transport (SpSc-Ringbuffer) | `crates/transport-shm/` | live | `zerodds-shm-transport-1.0.md` |
| `Arc<[u8]>` Shared-Ownership | `crates/rtps/` (21 usages) | live | — implementation-detail |

**Gap:** Die SHM-Infrastruktur existiert, ist aber **nicht im
DCPS-Hot-Path verdrahtet**. `commit_loan` faellt heute auf
`write_user_sample(Vec<u8>)` zurueck — das Slot-basierte
Loan-Modell ist nur Heap-Box-emuliert.

## 5 Copy-Klassifizierung

### 5.1 Spec-Pflicht (nicht reduzierbar ohne Wire-Bruch)

| # | Was | Warum Pflicht |
|---|---|---|
| CDR-Encode | struct → wire-bytes | XCDR2-Format ≠ Rust-Layout; XCDR2-Spec §7 |
| W4 Security | Tag/MAC anfuegen | DDS-Security 1.2 §9.5 |
| W5 UDP `send_to` Kernel | User-Buffer → Socket-Buffer | POSIX socket API |
| R1 UDP `recv_from` Kernel | Socket-Buffer → User-Buffer | POSIX socket API |

### 5.2 Implementations-Wahl (reduzierbar)

| # | Was | Heutiger Grund | Reduktions-Pfad |
|---|---|---|---|
| **W1** | C-FFI `slice → Vec` | Ownership-Transition C→Rust | `Loan<u8>` Slot-Pattern: Caller schreibt direkt in Pool-Slot |
| **W2/W2'** | Encap-Header-Mounting | Linearisierung fuer write-API | Scatter-Gather: `write_segments(&[&[u8]])` |
| **W3** | Submessage-Build → Vec | RTPS-Header-Vorbereitung | `BytesMut` Builder + `freeze() → Bytes` |
| **R1** | `recv_from` → Vec | Owned-Return fuer Caller | Slab-Pool + `Arc<[u8]>` mit Pool-Recycle |
| **R4** | `strip_encap → Vec` | Encap strippen | `Bytes::slice(off..)` ohne Copy |

### 5.3 Architektur-Investment (groesster Hebel)

| Pfad | Heute | Nach Wiring |
|---|---|---|
| Same-Host (SHM) | 5 Copies (faellt auf UDP-Pfad zurueck) | **0 Copies** via flatdata + iceoryx Slot-Backend |
| Loan-API (C-FFI) | Heap-Box, danach `commit_loan = write` ⇒ Vec-Copy | Slot-Pointer aus SHM-Backend, `commit_loan` ⇒ Slot-Publish |

## 6 Migrations-Wellen

### Welle 1 — C-FFI Loan-API echt Zero-Copy

**Scope:** `zerodds_writer_loan_message` + `zerodds_dw_loan_message`
auf einen pluggable Slot-Backend umstellen. Default-Backend bleibt
Heap-Box (no-op Aenderung); SHM-Backend nutzt iceoryx2-Slot wenn
Feature aktiv.

**ABI-Stabilitaet:** Funktionssignaturen unveraendert.

**Reduktion:** -1 Copy pro `commit_loan` wenn SHM-Backend an.

### Welle 2 — Reader-Path Zero-Copy (UDP-Pool + Arc)

**Scope:**
1. UDP-Recv: stack-buf → Slab-Pool-Alloc, return `Arc<[u8]>`
2. `strip_user_encap` ohne `.to_vec()` (offset + Arc-slice)
3. `UserSample::Alive { payload: Bytes }` statt `Vec<u8>`

**ABI:** Cross-language Bindings sehen weiter `(*mut u8, len)` —
C-FFI hat eigenen Vec-Materialization-Step nur an der FFI-Boundary.

**Reduktion:** -2 Copies pro empfangenem Sample.

### Welle 3 — Iceoryx-Backend-Hot-Path-Wiring (= Roadmap-Welle 4)

**Scope:** DCPS-Runtime erkennt zur Write-Zeit ob Same-Host-Reader
existiert, dann publish via flatdata-Slot statt UDP-Datagram.
Reader liest Slot direkt (mmap'd), kein Wire-Decode noetig.

**Voraussetzungen:** flatdata-Iceoryx-Backend stable, Welle 1 + 2
abgeschlossen, Discovery erkennt Same-Host-Peers (SPDP enthaelt
PID + Host-ID).

**Reduktion:** Same-Host-Pfad geht von 5 Copies auf **0 Copies**.

#### Implementierungs-Status (2026-05-18)

Diese Welle ist in fuenf Sub-Sprints (4a/4b.1-4b.5) untergliedert.
Stand aktueller Branch:

| Sub | Beschreibung | Status |
|---|---|---|
| 4a | `GuidPrefix::host_id` + `is_same_host` (FNV1a-Hash von gethostname in `bytes[0..4]`) | ✓ done (commit 456f3265) |
| 4b.1 | `dcps::same_host` Modul: SHM-Pfad-Convention + SameHostTracker mit Pending/Bound/Failed-State | ✓ done (commit 6b140ead) |
| 4b.2 | SEDP-Match-Hook in `wire_writer_to_remote_reader` und `wire_reader_to_remote_writer` registriert Same-Host-Paare im Tracker | ✓ done (commit f056ff46) |
| 4b.3 | DCPS-Cargo.toml: `zerodds-transport-shm` als feature-gated Dep (`same-host-shm`, default-off). Hook setzt nach `register_pending` den konkreten `PosixShmTransport` auf via `open_owner` (Reader-Seite) bzw. `open_consumer` (Writer-Seite). `mark_bound` schliesst das Lifecycle ab. | ⏳ open |
| 4b.4 | `send_on_best_interface` konsultiert vor UDP-Send den `SameHostTracker.lookup(writer_guid, reader_guid)`. Bei `Bound { transport, Consumer }` wird `transport.downcast_ref::<PosixShmTransport>().send(bytes)` aufgerufen; UDP-Fallback bei `Pending`/`Failed`. | ⏳ open |
| 4b.5 | Per-Owner-SHM-Recv-Worker: neuer Thread `recv_user_shm_loop`, der ueber alle Reader-Side Bound-Eintraege polled und `handle_user_datagram(&dg.data, ...)` analog UDP-Pfad dispatcht. | ⏳ open |
| 4c | Cross-Process-Test: Writer in Proc A, Reader in Proc B (gleicher Host), UDP-Sniffer auf 127.0.0.1 darf 0 User-Sample-Bytes sehen. | ⏳ open |

**Architektur-Notiz zu 4b.3+:**

`zerodds-transport-shm::PosixShmTransport` ist eine SpSc-Ringbuffer-
Implementation auf POSIX `shm_open` + `mmap` (Linux/macOS) bzw.
`CreateFileMapping` (Windows). Die 1:1-Owner/Consumer-Paarung
matched genau auf das DCPS-Modell `(Writer, Reader) -> Datagram-
Stream`. Pro Same-Host-Paar wird ein eigenes Segment angelegt
(Pfad via `shm_segment_filename` aus dem 4b.1-Modul, Verzeichnis
`${TMPDIR}/zerodds-shm/${host_id_hex}/`).

Eine Race liegt zwischen Owner-Bind (Reader-Seite) und Consumer-
Attach (Writer-Seite). Wenn SEDP-Match zuerst auf Writer-Seite
ankommt, schlaegt `open_consumer` fehl. Behandlung: `mark_failed`
und auf den naechsten Match-Versuch warten (SEDP ist periodisch),
oder spaeter beim ersten Send retry. UDP-Fallback bleibt jederzeit
aktiv, so dass kein Sample verloren geht.

**Optionale Feature-Gates** (4b.5 / separater Sprint):

- `same-host-iceoryx2`: nutzt `zerodds-flatdata::iceoryx` statt
  POSIX-Shm. Liefert echtes Zero-Copy fuer `T: FlatStruct`-Samples,
  fuer generische Bytes faellt es auf den POSIX-Shm-Pfad zurueck.
- `same-host-uds`: nutzt `zerodds-transport-uds` (UnixDatagram).
  Eliminiert IP-Stack-Overhead, ist aber **kein** echtes Zero-Copy
  (Kernel-Copy bleibt). Sinnvoll auf Targets, wo `shm_open` nicht
  erlaubt ist (z.B. Sandboxed-Container ohne `/dev/shm`).

## 7 Tests + Verifikation

| Welle | Test |
|---|---|
| Welle 1 | bench `loan_message + commit_loan`-Cycle: heap-vs-iceoryx-Slot-Vergleich |
| Welle 2 | bench `recv_loop`-Throughput vorher/nachher; Miri auf Arc<[u8]>-Lifecycle |
| Welle 3 | Same-Host-Cross-Process-Test mit zwei prozessen, verify zero Bytes durch UDP |

ABI-Snapshot-Test bleibt durchgehend gruen — keine extern fn-Signaturen
aendern sich.

## 8 Spec-Cross-References

- `zerodds-flatdata-1.0.md` — FlatStruct + Slot-Layout
- `zerodds-shm-transport-1.0.md` — SpSc-Ringbuffer auf SHM
- `zerodds-c-api-1.0.md` — Loan-API Signaturen (§2.6)
- DDS 1.4 §2.2.2.4.2 — Loan-API Spec-Vertrag
- DDSI-RTPS 2.5 §9.4 — Locator-Kind LOCATOR_KIND_SHM

## 9 Open-Items

- [ ] `BytesMut`/`Bytes` als kanonischer Wire-Buffer-Typ entscheiden
- [ ] `recvmmsg` Batch-Recv evaluieren (Linux-spez. Performance-Win)
- [ ] `sendmsg(iovec)` Scatter-Gather fuer W2-Encap-Framing
- [ ] Bench-Baseline messen: Throughput + Heap-Allocs pro Sample
