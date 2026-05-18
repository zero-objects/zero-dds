# Milestone v1.2 — Testable, Benchmarkable ZeroDDS

**Status:** Planning, 2026-04-20
**Vorgänger:** Phase 1 (v1.1) — WP 1.1-1.5 abgeschlossen, Post-Fix-Audit-Runde 2 grün
**Ziel:** Erste Distribution, die gegen Cyclone + FastDDS messbar und isoliert antreten kann

## Leitidee

Phase 1 hat **Protokoll-Compliance** (Wire-Format, SPDP, SEDP, Reliable,
XTypes) belegt. v1.2 liefert die drei Schichten, die ZeroDDS von einem
Spike zu einer **benutzbaren, vermessenen Library** machen:

1. **Transport-Parity**: echte POSIX-SHM + TCP-PSM + UDS — nicht Stubs.
2. **Public API**: DCPS nach OMG DDS 1.4, damit überhaupt jemand die Lib verwenden kann.
3. **QoS + History + Durability**: 22 QoS-Policies + Matching + History-Depth-Semantik + volle Durability-Leiter (VOLATILE → PERSISTENT, disk-backed).
4. **Proof**: dokumentierter Harness gegen Cyclone + FastDDS mit methodischer Strenge.

DDS-Security bleibt v1.3. **Persistenz ist v1.2-Scope — volle Leiter bis PERSISTENT, sqlite-backed, Durability-Service-Binary.**

## Scope — vier Phasen

```
Phase 2.0 — Enabler ✅ ABGESCHLOSSEN
  2.0a Zero-Copy-Payload-Spike           ✅ done (3-7 % gain)
  2.0b Transport-Parity (UDS-FS + TCP-Handshake + POSIX-SHM + UDS-Abstract + Isolation-Modes) ✅ done
  2.0c Hygiene (F3 O(log n), #11, alive_arc) ✅ done
  2.0a-2 Zero-Copy vollenden (iovec)     ❌ verworfen (measured negative,
                                          siehe docs/perf/v1.2-vectored-send-bench.md)
  2.0c-2 Hygiene-Batch 2 (F1/F2/#2/Rest) — opportunistisch

Phase 2.1 — Public API (~3-4 PT)
  2.1 DCPS (Publisher/Subscriber/DataWriter/DataReader/Topic/Domain)

Phase 2.2 — Semantics (~7-9 PT)
  2.2a QoS-Matching + 22 Policies + Compat-Matrix
  2.2b History-Backend (KEEP_LAST(N) + KEEP_ALL + VOLATILE + TRANSIENT_LOCAL)
  2.2c Durability-Service (TRANSIENT + PERSISTENT, sqlite-backed, Standalone-Binary)

Phase 2.3 — Proof (~4-5 PT)
  2.3 Interop + Perf Harness (abrufbar, dokumentiert, regressions-gated)

Gesamt: 18-23 Personen-Tage
```

## Phase 2.0 — Enabler

### WP 2.0a Zero-Copy-Payload-Spike

**Ziel:** Submessage-Level Zero-Copy. Derzeit endet `Arc<[u8]>` am
`DataSubmessage::serialized_payload: Vec<u8>`; `.to_vec()`-Copies in
`reliable_writer.rs:428,599,671` killen den Gewinn.

**Deliverable:**
- `DataSubmessage::serialized_payload` → `Cow<'_, [u8]>` oder `PayloadView`
- Baseline-Bench (vor Refactor) + Delta-Bench (nach)
- `docs/perf/v1.2-zerocopy-bench.md` mit Messwerten + Flamegraph-Delta

**Akzeptanz:** Reliable-Writer-Tick ≥ 20 % Throughput-Gewinn.

### WP 2.0b Transport-Parity

**Ziel:** UDP ist echt, TCP und SHM sind Stubs. Ohne echte Transports
ist WP 2.3 Harness nur 1/4 belastbar, und Containerized-Environments
haben keinen praktikablen Transport (Multicast oft gesperrt, SHM
cross-container schwierig).

**Transport-Matrix nach v1.2:**

| Transport | Status v1.1 | v1.2 Ziel | Use-Case |
|-----------|-------------|-----------|----------|
| UDP unicast/multicast | ✅ | ✅ | LAN default |
| TCP | Stub (framing only) | Handshake nach DDS-TCP-PSM §5.2.1 | WAN, Firewall-traversal |
| SHM POSIX | Stub (intra-proc `Arc<Mutex<Ring>>`) | `shm_open` + `mmap` + fd-passing | Host-local high-throughput |
| **UDS** (neu) | — | Unix-Domain-Sockets (`SOCK_SEQPACKET`) | Container-IPC ohne Multicast |

**Warum UDS dazu:** In Dockerized Environments ist Multicast meist
gesperrt und POSIX-SHM cross-container pain (UID-Mapping, SELinux,
`/dev/shm`-Visibility). UDS über gemountete Volumes ist der
realistische Container-IPC-Pfad. Kein DDS-Standard — ZeroDDS-Extension
über Vendor-PSM-Kind im Locator.

**Isolation-Modes (neu, Harness-relevant):**

Jeder Transport muss in diesen Isolation-Stufen funktionieren:

| Level | Beschreibung | Was getestet wird |
|-------|--------------|-------------------|
| L0 | Same-Process | Baseline (heute Phase 1) |
| L1 | Same-User-Different-Process | `fork`/`exec`-Isolation, fd-Sharing |
| L2 | Different-User-Same-Host | POSIX-Permissions, SHM-Group-Access |
| L3 | Different-Container-Same-Host | `/dev/shm`, UDS über mounted Volumes, Netns |
| L4 | Different-Host-LAN | UDP/TCP über echtes Netz |

**Deliverable:**
- `transport-shm-posix` Crate (neu)
- `transport-tcp` mit §5.2.1-Handshake
- `transport-uds` Crate (neu), ZeroDDS-Extension
- `testing/isolation/` Setup-Scripts für L1-L4
- ADR `docs/adr/0xxx-uds-transport.md`
- ADR `docs/adr/0xxx-shm-posix-model.md`

**Akzeptanz:** Jeder Transport durchläuft L0-L4 Smoke-Test (1W-1R,
Sample-Roundtrip).

### WP 2.0a Status

✅ Abgeschlossen 2026-04-20, commit `0c19d7d`. `DataSubmessage`/
`DataFragSubmessage` `serialized_payload` auf `Arc<[u8]>` umgestellt;
Writer-Hot-Path nutzt `Arc::clone` statt `to_vec`. Bench-Report:
`docs/perf/v1.2-zerocopy-bench.md`.

**Messergebnis:** 3–7 % Gewinn (statt 30–50 % aus dem Pre-Fix-Audit).
Grund: der Refactor eliminiert **eine** von drei Payload-Kopien im
Wire-Pfad. Die restlichen zwei (`DataSubmessage::write_body` +
`MessageBuilder::append_submessage`) werden erst mit
`sendmsg(2)`+iovec eliminiert → **WP 2.0a-2** (folgt nach WP 2.0b,
weil iovec nur gewinnt, wenn der Transport es tatsaechlich nutzt).

### WP 2.0c Hygiene ✅ abgeschlossen

Batch 1, commit `5eda933`:

- ✅ **F3** SEDP-Cache-Insert O(N) → O(log n): Sekundaer-Index
  `BTreeMap<GuidPrefix, BTreeSet<(Duration, Guid)>>` pro
  Publications/Subscriptions. Invariante wird in Insert/Update/
  Remove/on_participant_lost erhalten.
- ✅ **#11** `read_opt_string` / `read_opt_bytes`: `len > 1`
  rejected mit `DecodeError::LengthExceeded` statt stillem
  Datenverlust.
- ✅ `CacheChange::alive_arc` auf `pub(crate)` — Arc-Pfad ist
  interne Writer/Reader-Optimierung.
- ✅ Smells #8 SilentDowngrade re-verify: durch P1-4b-Konsolidierung
  (Duration/DurabilityKind/ReliabilityQos re-exports) implizit
  resolved.

## Hygiene-Backlog (deferred, in v1.2 einzuordnen)

Items aus den Phase-1-Audits, die bewusst nicht im ersten
Hygiene-Batch stecken, aber vor v1.2-Release adressiert werden
muessen. Zuordnung zu WPs je nach Touchpoint.

### Geht in WP 2.0a-2 (Zero-Copy vollenden, nach 2.0b)

- **N-M1** `DataSubmessage::write_body` + `MessageBuilder::
  append_submessage` kopieren Payload weiter (2 von 3 Kopien). Fix
  mit `sendmsg(2)` + iovec im Transport-Layer. Bench-Ziel: ≥30 %
  zusaetzlicher Gewinn gegenueber WP 2.0a.
- **F6** `FragmentBuffer::missing` scannt komplette Fragment-Range
  — O(fragments) pro `NACK_FRAG`. Fix: tracked missing-Set mit
  O(log n) Lookup. Niedrig, aber bei 16k-Fragment-Samples sichtbar.

### Geht in WP 2.0b (Transport-Parity)

- **F13** `TcpTransport` haelt Inbound-Mutex ueber `Condvar::wait`
  — blockt Reader bei Sender-Kontention. Fix: Lock droppen vor
  wait, MPSC-Channel fuer Inbound. Teil des TCP-Rewrites.
- **F14** `ShmTransport::send` kloniert Payload innerhalb des
  Peer-Locks. Fix: Arc-Payload-API durchziehen (WP 2.0a-Signatur
  hilft), clone vor `lock()`. Teil des SHM-POSIX-Rewrites.
- **Smells #6 + #7** Silent Lock-Failure in SHM/TCP (`unwrap_or`
  auf `lock()`-Result). Fix im Zuge des SHM/TCP-Refactors.
- **F16** UDP `recv()` allokiert 64 KiB Stack-Array pro Call —
  Low, aber bei hoher Rx-Rate messbar. Fix: pool-allozierter
  Buffer pro Transport-Thread.

### Geht in WP 2.1 (DCPS) — Touchpoint-Aufraeumen

- **F4** `ReaderProxy`-Lookup `iter().position()` bei AckNack/
  NackFrag — O(P). Fix: Parallel-`BTreeMap<Guid, usize>`-Index
  auf den Vec. Bei grossen Fan-outs (P>50) relevant.
- **F5** `ReliableReader::proxy_index_by_writer_id` O(W) pro
  eingehendem DATA/HEARTBEAT/GAP. Fix: `BTreeMap<EntityId,
  usize>`. Bei vielen Writern linear pro Rx-Paket.
- **F12** `ParticipantData::targets_for` kloniert Locator-Vecs pro
  Reader-Proxy. Fix: `Rc<Vec<Locator>>` oder Indirection. Betrifft
  DCPS-Topic-Matching-Hot-Path.
- **Perf-N2** `Arc::from(Vec)` Overhead im Writer fuer kleine
  Payloads — Design-Input fuer DCPS `write_arc()`-Entrypoint, der
  dem Nutzer direkt einen Arc entgegennimmt.
- **Smells #9** Annotation-Parsing `unwrap_or_default` in 6
  Stellen — swallows parse errors. Fix: eigentliche Error-
  Propagation, sobald die DCPS-Typed-API die Annotationen sichtbar
  macht.

### Geht in WP 2.2a (QoS-Matching)

- **F19** `HistoryCache::remove_up_to` ist O(n log n) — wird in
  KEEP_LAST-Pfaden relevant. Fix im Zuge des History-Backend-WPs,
  weil die API sowieso neu geschnitten wird.
- **Smells #10** Padding-Reader akzeptiert Nicht-Null-Bytes. Spec
  sagt MUST be zero. Fix als strict validation im QoS-Wire-Decoder.

### Geht in WP 2.0c-2 (Hygiene-Batch 2)

Kleines zweites Hygiene-Batch, wenn sich die Touchpoints oben nicht
ergeben:

- **F1** `Mutable`-Struct-Assignability O(n·m). Member-Loop
  Refactor mit vorsortiertem Id-Set. ~100 LOC, nicht trivial. Heute
  noch nicht relevant, aber bei 1000+-Member-Types waere das eine
  User-sichtbare Latenz.
- **F2** Enum-Assignability analog (Med).
- **Smells #2** Recursion-depth-Marker-Audit. 17 Stellen im `idl/
  src/**` tragen `zerodds-lint: recursion-depth 64`, deren reale Tiefe
  deutlich niedriger ist. Audit + reale Grenzen einsetzen.
- **F11** CDR-Extensibility-Encoder Per-Member-Inner-Buffer (Low) —
  kleiner Alloc-per-Member-Overhead.
- **F15** TCP peer-pool eviction-Policy unklar (Low).
- **Smells #12** Dead-Code-Attribute ohne Tracking-Issue.
- **Smells #13** Test-Formatter-Hack als Platzhalter.
- **Smells #15** `if let Ok(...)` ohne else-Branch in
  `transport-{tcp,shm}` — entweder `.ok()` mit explizitem drop
  oder Error-Propagation.

### Strategisch, kein separater WP — in Phase-2-Normal-Entwicklung

- **F20** `Transport::recv` ist blocking (Low, Info). Phase-2-
  tokio-Integration waere hilfreich, aber das ist eine
  Architektur-Entscheidung, keine Hygiene.
- **Perf-N3** Arc-Atomics sind unkritisch solange `tick()`
  single-threaded bleibt. Wenn WP 2.1 das Multi-Thread-Modell
  oeffnet, muss Arc vs. Rc neu bewertet werden.

**Tracking:** jedes Item traegt ein Kuerzel (F#, #, N-M#) und
referenziert den Audit-Report — `phase1-perf-audit.md`,
`phase1-smell-audit.md`, `phase1-post-fix-*.md`. Beim Abschluss
eines Items in den jeweiligen Audit-Reports abhaken.

## Phase 2.1 — DCPS Public API

### WP 2.1 DCPS

**Ziel:** Die API aus OMG DDS 1.4 §2.2.2. Ohne das ist ZeroDDS nicht
als Library nutzbar, nur als Protokoll-Stack.

**Deliverable:**
- `crates/dds/` — neue top-level-Crate
- `DomainParticipantFactory::instance()` → `get_participant(domain_id)`
- `DomainParticipant::create_publisher/subscriber/topic`
- `Publisher::create_datawriter<T>` / `Subscriber::create_datareader<T>`
- `Topic<T>` typed, mit `dds_idlgen`-generierten `T`-Types
- `DataWriter::write/dispose/unregister_instance`
- `DataReader::take/read` + `Listener`-Trait + `WaitSet`/`Condition`
- Lifecycle + Thread-Safety (`Arc<RwLock<...>>` über Participant-Tree)

**Akzeptanz:**
- Beispiel-App `examples/hello_dds/` publisht "HelloWorld" und liest
  ihn gegen Cyclone-Subscriber (und umgekehrt).
- DCPS-Unit-Tests: 80+ Tests über API-Fälle.
- Coverage-Ziel: ≥ 85 % R / ≥ 95 % L im neuen Crate.

## Phase 2.2 — Semantics

### WP 2.2a QoS-Matching

**Ziel:** Alle 22 QoS-Policies aus §2.2.3 + Compat-Matrix aus §2.2.3.
Heute: nur Reliability + Durability matching-relevant.

**Deliverable:**
- `qos-matcher` Modul
- Policy-Enforcement-Table nach Spec-Compatibility
- `INCOMPATIBLE_QOS`-Events wie von Spec verlangt
- Test-Grid: jede Policy × {compatible, incompatible} → 44 Matching-Tests
- `docs/standards/qos-compatibility-matrix.md`

**Akzeptanz:** Cyclone-Parity auf QoS-Matching (dieselben
inkompatiblen Requested/Offered werden identisch gematcht bzw.
abgelehnt).

### WP 2.2b History-Backend

**Ziel:** `KEEP_LAST(depth=N)` + `KEEP_ALL` + In-Memory-Durability
für `VOLATILE` + `TRANSIENT_LOCAL`.

**Depth-Semantik:**

| Policy | Verhalten |
|--------|-----------|
| `KEEP_LAST(1)` | nur letztes Sample pro Instance |
| `KEEP_LAST(N)` | letzte N Samples (Ring pro Instance) |
| `KEEP_ALL` | alle bis `resource_limits` greifen |
| `RELIABILITY=RELIABLE + KEEP_ALL` | block-on-full bis Space frei |

**Durability-Levels in 2.2b:**
- `VOLATILE` — keine Replay für Late-Joiner
- `TRANSIENT_LOCAL` — in-memory Replay-Queue pro Writer,
  SEDP-triggered Replay an Late-Reader

**Deliverable:**
- `history` Crate mit `HistoryBackend`-Trait + `MemoryBackend`
- `DurabilityReplay`-Engine (writer-lokal, SEDP-triggered)
- Dedizierter History-Harness (siehe 2.3 `h_history_depth`)
- ADR `docs/adr/0xxx-history-semantics.md`

**Akzeptanz:** Late-Joiner-Replay deterministic beweisbar gegen
Cyclone (pcap-verifiziert).

### WP 2.2c Durability-Service — TRANSIENT + PERSISTENT

**Ziel:** Vollständige Durability-Leiter aus §2.2.3.4. Das ist der
Feature-Claim, der ZeroDDS von "Proof-of-Concept" zu "kann Cyclone
ersetzen" bringt.

**Durability-Levels in 2.2c:**

| Level | Überlebt | Scope | Backend |
|-------|----------|-------|---------|
| `TRANSIENT` | Writer-Neustart, Reader-Neustart | Domain | Durability-Daemon-Speicher |
| `PERSISTENT` | System-Neustart | Domain | Disk (sqlite WAL) |

**Architektur:**

```
                  ┌─────────────────────────┐
                  │   zerodds-durability    │   Standalone-Binary
                  │                         │
  DataWriter ─────▶ DurabilityClient ─────▶│ ─── sqlite://data.db
  (TRANSIENT/                              │
  PERSISTENT QoS)                          │
                  │    DataReader          │
  Late-Joiner ◀─── DurabilityReplay ◀──────┤
                  └─────────────────────────┘
```

Modell folgt Cyclone's `dds_persist`-Pattern:
- **Standalone-Binary** `zerodds-durability-svc`, per systemd/Docker.
- **Eigenes BuiltIn-Topic** `DCPSDurability` für Service-Discovery
  (kein magischer Port).
- **Client-Code im Participant** meldet jedes
  TRANSIENT/PERSISTENT-Topic beim Service an.
- **Replay-Protocol** über normales RTPS (TRANSIENT_LOCAL-Mechanik
  wiederverwendet), der Service ist aus RTPS-Sicht nur ein Writer mit
  `TRANSIENT_LOCAL(KEEP_ALL)`.

**Backend:**

- **sqlite mit WAL-Journal-Mode** — in-process, kein Server,
  ACID-sicher, embeddable.
- Schema: `(participant_guid, topic_name, sequence_number,
  source_timestamp, key_hash, serialized_payload BLOB)`.
- Index auf `(topic_name, sequence_number)` für Replay-Sweep.
- `PRAGMA journal_mode=WAL; synchronous=NORMAL` (crash-safe ohne
  jedes Insert zu flushen).

**Crash-Recovery:**

- Service-Crash: WAL-Recovery bei Start.
- Writer-Crash: TRANSIENT überlebt (im Service). PERSISTENT überlebt
  System-Restart.
- Service-Disk-Full: `RESOURCE_LIMITS_EXCEEDED`-Event + deterministic
  drop-policy (oldest-first) dokumentiert.

**Deliverable:**
- `crates/durability-client/` — Client-Code im Participant
- `crates/durability-service/` — Standalone-Daemon-Logik
- `bin/zerodds-durability-svc` — Binary
- `docs/adr/0xxx-durability-service.md` — Architektur + Rationale
- `docs/deployment/durability-service.md` — Setup + Backup + Tuning
- Dedizierter Durability-Harness (siehe 2.3 `h_durability`)
- Crash-Recovery-Test-Suite (5 Szenarien: Service-Crash,
  Writer-Crash, Reader-Crash, Disk-Full, Corruption-on-Disk)

**Akzeptanz:**
- Publisher schreibt 10k Samples → Service-Kill -9 → Service-Start →
  Subscriber joint → alle 10k Samples in korrekter Reihenfolge.
- System-Reboot → PERSISTENT-Daten überleben.
- Interop: Cyclone-Writer mit `TRANSIENT` → ZeroDDS-Service lagert
  ein → Cyclone-Reader joint → Replay kommt an.

## Phase 2.3 — Interop + Perf Harness

### WP 2.3 Harness

**Ziel:** Jeder Marketing-Claim hat eine reproduzierbare, belegbare
Grundlage.

#### Harness-Scenarios

| Harness | Dimensionen | Was es belegt |
|---------|-------------|---------------|
| `h_throughput_small` | 64B/1kB, REL+BE, UDP/SHM/TCP/UDS, 1W-1R | Baseline msg/s + µs-Latenz |
| `h_throughput_large` | 16kB/1MB (fragmentiert), REL, UDP/SHM | Fragmentation + Reassembly-Cost |
| `h_history_depth` | KEEP_LAST(N) mit N=1,10,100,1k,10k; KEEP_ALL | Depth-Scaling, Memory vs. depth, Late-Joiner-Replay |
| `h_durability` | VOLATILE/TRANSIENT_LOCAL/TRANSIENT/PERSISTENT × {small,medium,large}-Backlog × Crash-Recovery | Durability-Leiter-Parity gegen Cyclone, Crash-Recovery-Zeit, Disk-Footprint |
| `h_many_topics` | 10/100/1k/10k Topics | Topic-Scaling + SEDP-time-to-match + RSS |
| `h_many_participants` | 2/10/50/200 Participants | Discovery-Amplifikation, Multicast-Loop, RSS |
| `h_large_data_stream` | 1 MB/s → 1 GB/s Saturation | Throughput-Saturation + Drop-Rate |
| `h_fanout` | 1W → 10/100/1k Readers | Reader-Scaling, pro-Reader-Cost |
| `h_mixed_workload` | 10W×10R mit gemischten QoS | Realistischer Workload + Matcher-Cost |
| `h_container_ipc` | L3 Isolation, UDS vs. SHM vs. UDP-localhost | Containerized-Deployment-Vergleich |
| `h_isolation_sweep` | h_throughput_small × L0-L4 | Isolation-Overhead quantifiziert |

#### Jeder Harness liefert

1. **Abrufbar**: `just bench-<name>` → single command
2. **Reproduzierbar**: `benches/harness/<name>/config.toml` fixiert Workload, Dockerfile pinnt Versionen
3. **Dokumentiert**: `docs/perf/<name>.md` mit
   - Method-Section (Workload, Warmup, Messfenster, Runs)
   - Raw-Data-Pfad (`benches/data/<name>/<date>.json`)
   - Plots (p50/p95/p99/p99.9 als HDR-Histogram)
   - Flamegraph-Link
   - Known-Gaps-Section (wo wir schlechter sind und warum)
4. **Vergleichbar**: 3-fach gegen ZeroDDS/Cyclone/FastDDS, gleicher Workload, pcap-verifiziert
5. **Regressions-gated**: CI lädt Baseline-JSON, p95 ±10 % Toleranz, fail-on-regression

#### Methodik (Disziplin)

- **Warmup** 10 s oder 100k Samples
- **Messfenster** ≥ 60 s oder ≥ 10M Samples
- **n=5 Runs**, **Median + MAD** (robust, kein Mean)
- **Isolated bench-host** mit `isolcpus`, `IRQ-pinning`, `cpu-governor=performance`, kein NUMA-Cross
- **HDR-Histogram** für Latenz, nicht min/max/avg
- **pcap-capture pro Run** zur Wire-Verifikation
- **Flamegraphs** pro Run (`cargo flamegraph`)
- **Raw-Data im Repo** (JSON, nicht nur PNG)

#### Deliverables

- `benches/harness/` — 10 Benches, single-command je
- `testing/isolation/docker/` — L3-Compose-Files pro Transport
- `testing/isolation/host/` — L1/L2-Scripts (fork/exec, setuid)
- `docs/perf/v1.2-baseline.md` — Überblick + Navigation
- `docs/perf/<harness>.md` — 10 Einzel-Reports
- `docs/perf/v1.2-vs-cyclone.md` — Head-to-Head
- `docs/perf/v1.2-vs-fastdds.md` — Head-to-Head
- `docs/perf/known-gaps.md` — Transparente Schwachpunkte
- `docs/perf/methodology.md` — Shared Method-Section

## Abhängigkeiten

```
2.0a ┐
2.0b ├─ 2.1 ─ 2.2a ─ 2.2b ─ 2.2c ─ 2.3
2.0c ┘

2.0a blockt nichts (nur Perf-relevant)
2.0b blockt 2.3 SHM/TCP/UDS-Benches
2.1 blockt 2.2a (QoS-Matcher braucht DCPS-Entities)
2.2a blockt 2.2b (History-Behavior hängt an QoS)
2.2b blockt 2.2c (Durability-Service baut auf HistoryBackend-Trait)
2.2c blockt 2.3 h_durability
```

## Out of Scope v1.2 → v1.3

- DDS-Security (AuthN/AC/Crypto-Plugins) — eigener Milestone
- rocksdb-Backend — sqlite reicht für v1.2; rocksdb als Alternative v1.3 falls Perf-Claim es verlangt
- FastDDS 3.x — bleibt bei 2.14.x LTS für v1.2
- Monitor-Topic (`DCPSParticipant`, `DCPSTopic` etc.) — v1.3
- Multi-Instance-Durability-Service mit HA/Replikation — v1.3+

## Bench-Hardware

Zwei fixierte Hosts für alle Messungen, gemeinsames LAN `192.168.178.0/24`:

### `llvm` — Primary Bench Host (Baseline)

- SSH: `llvm@llvm` → `192.168.178.60`
- CPU: **AMD Ryzen Threadripper PRO 3955WX**, 24 Cores / 1 Thread per Core, 1 Socket, 1 NUMA
- RAM: 47 GiB
- Disk: 456 GB NVMe (230 GB frei)
- OS: Debian 12, Kernel 6.1.0-44
- **Bare-Metal** → Kernel-Tuning möglich (`isolcpus`, `cpu-governor=performance`, `IRQ-pinning`)
- **Rolle:** alle Baseline-Messungen, Flamegraph-Host, Harness-Primary-Peer

### `pivot` — Secondary Peer (Remote + Container-Sim)

- SSH: `root@pivot` → `192.168.178.173`
- CPU: **2× Intel Xeon E5-2640 v4**, 10 Cores / 2 Threads per Core = 40 logical, 2 Sockets, **2 NUMA Nodes**
- RAM: 128 GiB
- Disk: 451 GB (417 GB frei, rpool ZFS)
- OS: Debian 12, Proxmox Kernel 6.17 via **LXC-Container** (20 Threads zugewiesen)
- **Virtualisierung: LXC** → shared Kernel, **kein** isolcpus/governor-Tuning möglich
- **Rolle:** L4 Cross-Host Peer, Containerized-Deployment-Realismus, Discovery-Stress-Peer (viele Participants)

### Rollen-Matrix

| Harness | Primary | Secondary | Modus |
|---------|---------|-----------|-------|
| `h_throughput_small/large` | llvm | llvm (L0-L1) | intra-host, getuned |
| `h_history_depth` | llvm | llvm | intra-host |
| `h_many_topics/participants` | llvm | pivot | cross-host, realistisch |
| `h_large_data_stream` | llvm | llvm | intra-host, getuned |
| `h_fanout` | llvm (W) | llvm + pivot (R) | mixed |
| `h_mixed_workload` | llvm + pivot | beide | realistisches Lab |
| `h_container_ipc` | llvm | llvm (docker) | Container-Isolation |
| `h_isolation_sweep` | llvm | llvm | L0-L3 intra-host, L4 cross-host zu pivot |
| `h_durability` | llvm | pivot | Service auf pivot, Writer/Reader auf llvm |

### Tuning-Script

- `benches/hosts/llvm/setup.sh` — `isolcpus`, governor, IRQ-pin, `sysctl net.core.rmem_max`
- `benches/hosts/pivot/setup.sh` — nur Netz-Tuning (kein Kernel-Tuning in LXC)
- `benches/hosts/teardown.sh` — defaults wiederherstellen
- CI-Hook: nach jedem Bench-Run wird `benches/hosts/<host>/state.json` committed (CPU-freq, governor, load — reproducibility)

### Netz-Checks vor Bench-Start

- UDP Multicast zwischen llvm↔pivot (`smcroute` / `msend`/`mrecv`) verifizieren
- Route-MTU, keine Firewall-Drops
- Jitter-Baseline (`iperf3 -u`) dokumentieren pro Run in `state.json`

## Offene Entscheidungen

1. **Shared-Deps-Upgrade vor v1.2-Start?** `tokio`/`bytes`/`serde` auf neueste Major?
2. **FastDDS-Fixture-Version pinnen in CI** (Docker-Image mit fixierter FastDDS-Build), gleich zu Cyclone-Image.
3. **Durability-Service Deploy-Model**: Per-Domain-Instance oder ein Service für mehrere Domains?

## Akzeptanz v1.2

- [ ] Alle 4 Phasen abgeschlossen
- [ ] Workspace-Coverage ≥ 85 % R / ≥ 93 % L
- [ ] `cargo bench` läuft alle 10 Harness-Scenarios zu Ende
- [ ] 3 Head-to-Head-Reports vs. Cyclone + FastDDS veröffentlicht
- [ ] CI gated auf Bench-Regression
- [ ] Beispiel-App publishes+subscribes gegen beide Fremd-Stacks
- [ ] `docs/perf/methodology.md` + `known-gaps.md` vollständig
