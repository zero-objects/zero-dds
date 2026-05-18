# Phase-5 Plan

**Stand:** 2026-05-02
**Vorgaenger:** `docs/PHASE4_CLOSEOUT.md` (Sprint 9-17 abgeschlossen).

Phase 5 fokussiert auf **Real-Time, Distribution, Operability** —
keine neuen Protokoll-Stacks, keine neuen Spec-Conformance-Punkte.
Drei Cluster, parallelisierbar:

```
Cluster D — Real-Time + Latency
    ↓ blockt nichts
Cluster E — Build- und Distro-Adapter   ── parallel zu D
    ↓ blockt nichts
Cluster F — Tooling + Operability       ── parallel zu D+E
```

**Total Phase-5:** ~65-90 PW menschen-aequivalent.

---

## 1. Cluster D — Real-Time + Latency

**Aufwand:** ~25-35 PW.
**Ziel:** ZeroDDS auf 1µs-Latenz-Pfade hardenern — fuer Industrial-
Automation, Avionik und Trading-Workloads. Mission ist nicht
"Konkurrenz schlagen", sondern **deterministische Tail-Latenz** unter
Last; das ist ein Differenzierungs-Feature, das die Phase-3-Stacks
allein nicht liefern.

### D.1 — Hot-Path no_alloc (WP 5.D.1)
* `crates/foundation::buffer::PoolBuffer` als Static-Pool mit
  Compile-Time-Caps (`MAX_PARTICIPANTS` / `MAX_ENDPOINTS` /
  `MAX_FRAGMENTS`) — Pool-Allocator pro Domain.
* Hot-Path `Writer::write` + `Reader::take` ohne `Box`/`Vec`-Reallocs
  in den Standard-Pfaden (Klein-Sample <= 1.5kB).
* Lint `clippy::disallowed-methods` fuer `Vec::with_capacity` /
  `Box::new` in `crates/dcps`/`crates/rtps` Hot-Path-Modulen.
* Bench-Baseline: `tools/bench-suite` p99/p999/p9999-Histograms,
  Soak-Test ueber 24h ohne Heap-Wachstum.

### D.2 — ARM-Crypto-Extensions + AES-NI (WP 5.D.2)
* `security-crypto::aes_gcm_hw`: AES-NI auf x86_64 (`x86_64::__cpuid`)
  + ARMv8-AES (`std::arch::is_aarch64_feature_detected!("aes")`).
* Fallback: existing `aes-gcm` softimpl bleibt.
* `tools/perf::aes-gcm` Bench: 100MB Throughput-Vergleich SW vs. HW
  auf llvm (x86_64) + pivot (aarch64).

### D.3 — RT-Scheduling-Profile (WP 5.D.3)
* `crates/dcps::scheduler` mit `SchedulerProfile`-Enum:
  `{ Default, RealtimeFifo { prio }, RealtimeRR { prio }, Deadline { runtime, deadline, period } }`.
* Linux-Backend via `sched_setscheduler`(2) +
  `SCHED_DEADLINE`-Variant via `sched_setattr`(2).
* CPU-Pinning per `sched_setaffinity`(2), Helper
  `set_isolcpu_mask(&[u32])`.
* Doc `docs/REALTIME_DEPLOYMENT.md`: kernel-Tuning (isolcpus,
  nohz_full, rcu_nocbs), preempt_rt-Note.

### D.4 — Lock-Free History-Cache (WP 5.D.4)

Wegen der Reichweite des Refactors in drei Phasen geliefert:

**Phase A (Sprint 21, abgeschlossen 2026-05-02):** Atomare Stats
parallel zum `BTreeMap`-Storage. `HistoryCacheStats` mit
`AtomicUsize`/`AtomicU64`/`AtomicI64` fuer `len`/`evicted`/
`max_sn`/`min_sn`. `cache.stats() -> Arc<HistoryCacheStats>` liefert
ein lock-free pollbares Handle. Acquire/Release-Ordering pro Atom;
cross-field-Konsistenz nicht garantiert (Tear akzeptabel fuer
Monitoring). Schreib-Pfad bleibt `&mut self` — der eigentliche
`BTreeMap`-Storage ist weiter durch den umgebenden
ReliableWriter-Lock geschuetzt. Tests inkl. Reader-Writer-Thread-
Crossover gruen.

**Phase B (Sprint 21, abgeschlossen 2026-05-03):** Per-Slot-Locking
auf `user_writers`/`user_readers`. Vorher
`Mutex<BTreeMap<EntityId, Slot>>`, jetzt
`RwLock<BTreeMap<EntityId, Arc<Mutex<Slot>>>>`. Hot-Path
(`write_user_sample`, `handle_user_datagram`) nimmt read-lock
(cheap), klont Slot-Arc, gibt read-lock frei, nimmt per-Slot-
Mutex. Parallele Writes auf verschiedene Slots laufen damit ohne
globale Contention. Vier neue Helper auf `DcpsRuntime`
(`writer_slot`, `reader_slot`, `*_slots_snapshot`, `*_eids`)
kapseln das Pattern. ~45 Call-Sites systematisch migriert.
547 dcps-Tests + 8040 Workspace-Tests gruen.

**Phase C (Sprint 21, abgeschlossen 2026-05-03):** Lock-Free-Read-
HistoryCache via Copy-on-Write. Foundation-Primitive
`zerodds_foundation::rcu::RcuCell<T>` mit `Mutex<Arc<T>>`-Layout —
Reader nehmen den Mutex nur fuer einen Refcount-Inc (Sub-µs),
danach lebt der Arc-Snapshot unabhaengig. Schreiber kopieren-on-
write. Neuer Typ `zerodds_rtps::history_cache::LockFreeReadHistoryCache`
mit gleicher Public-API wie `HistoryCache` aber `&self` statt
`&mut self` fuer Mutationen. Atomare Stats aus Phase A weiter genutzt.
17 Tests (7 RcuCell + 10 LockFreeReadHistoryCache) inkl.
Concurrent-Reader-Writer-Smoke.

Trade-Off in Phase C: Insert ist O(n) wegen BTreeMap-Klon.
Akzeptabel fuer Discovery-Caches und Monitoring-Pfade; fuer write-
heavy Reliable-Writer-Caches bleibt der klassische `HistoryCache`
besser. Folge-Optimierung mit `im::OrdMap` (persistente
Datenstruktur, O(log n) Insert) ist eine Crate-Dep-Frage und nicht
mehr in D.4 Phase C eingelagert.

### D.5 — Latency-Bench-Suite (WP 5.D.5)
* `tools/bench-suite::roundtrip-1us` mit busy-poll-Reader gegen
  busy-poll-Writer.
* Vergleichs-Profile: ddsperf (Cyclone) + FastDDS-LatencyTest auf
  identischer Hardware (llvm bare-metal).
* CI-Gate: p99 < 5µs, p999 < 20µs, p9999 < 100µs auf llvm-Profil
  Reference (Linux 6.x preempt_rt, isolcpu=2-7).

---

## 2. Cluster E — Build- und Distro-Adapter

**Aufwand:** ~15-20 PW.
**Ziel:** ZeroDDS aus Source-Repos und Binary-Pakete heraus
installierbar machen — fuer ROS-2-User, Debian/Ubuntu-Admins,
Cargo-User, conda-forge-Maintainer. Phase-4 hat die Rust-seitigen
Adapter geliefert; Phase-5 macht den letzten Yard zum
ame
nt-cmake/dpkg/cargo-publish-Pfad.

### E.1 — `rmw-zerodds-shim` cbindgen + ament-cmake (WP 5.E.1)
* `crates/rmw-zerodds-shim/`: extern-`C`-Wrapper, der
  `ros2-rmw`-Crate auf rmw-API mappt.
* `cbindgen.toml` + `build.rs`-Hook fuer `rmw_zerodds.h` (rmw-Ret-
  Codes, rmw_node_t, rmw_publisher_t, rmw_subscription_t).
* `ament-cmake`-Package `rmw_zerodds` mit
  `package.xml`, `CMakeLists.txt`, `ros2 launch`-Smoketest.
* Distro-Targets: ROS-2 Humble (ament_cmake 1.5.x) + Iron + Jazzy.
* CI-Job `ci/jobs/rmw-distro-build.yml` baut alle drei Distros in
  separaten Docker-Layern.

### E.2 — Debian/RPM-Paketierung (WP 5.E.2)
* `pkg/debian/`: Source-Package + `debian/rules` mit
  `dh-cargo`-Wrapper.
* Binary-Pakete: `libzerodds`, `libzerodds-dev`, `zerodds-tools`,
  `librmw-zerodds`.
* `pkg/rpm/zerodds.spec`: RHEL-9 + Fedora-39+.
* Repository-Hosting: `apt.zerodds.org` + `yum.zerodds.org` (CI-
  signed mit GPG, Reprepro/createrepo_c-basiert).

### E.3 — `cargo publish` auf crates.io (WP 5.E.3)
* Workspace-Crate-Reihenfolge bestimmen (DAG-Sort der internen
  Deps).
* Pro Crate `package.metadata.docs.rs` + Top-Level-`README.md` mit
  Spec-Mapping und Safety-Klassifikation.
* `cargo workspaces` als Tool fuer atomar-koordinierte Versions-
  Bumps.
* License-Audit: alle 87 Crates Apache-2.0; `cargo deny check
  licenses` clean.
* Erste Publikation als `0.1.0`-Pre-Release (yanked fall-back).

### E.4 — Yocto/Buildroot Layer (WP 5.E.4)
* `meta-zerodds/`: Yocto-Layer mit `zerodds_%.bb`-Recipe.
* Buildroot `package/zerodds/` mit Kconfig-Optionen
  (`BR2_PACKAGE_ZERODDS_RTPS_ONLY`, `_WITH_SECURITY`,
  `_WITH_BRIDGES`).
* Cross-Compile-Targets: aarch64-unknown-linux-musl + armv7-musl.
* Smoketest auf QEMU-aarch64 in CI.

---

## 3. Cluster F — Tooling + Operability

**Aufwand:** ~25-35 PW.
**Ziel:** Production-Operability — Recording, Replay, Chaos-
Engineering, Distributed-Tracing, Live-Dashboards. Diese Capability-
Schicht ist haeufiges Differenzierungs-Argument bei Vendor-
Auswahlen, gerade weil RTI/Cyclone/FastDDS hier Lichtjahre
auseinanderliegen.

### F.1 — Recording/Replay-Format (WP 5.F.1)
* `crates/recorder`-Erweiterung: kompaktes Binary-Format
  `.zddsrec` mit Header + Sample-Stream-Frames.
* Header: ParticipantSet + Topic-Set + Type-Schemas (XTypes
  TypeObject) + Time-Base-Anchor.
* Frames: `(timestamp, participant_idx, topic_idx, sample_kind,
  cdr_payload)`.
* Replay-Modus: `tools/replay`-CLI re-injects in einen Live-
  Domain (mit Time-Scale + Topic-Filter).
* Format-Doc `docs/specs/zddsrec-1.0.md` als interne Spec.

### F.2 — Chaos-Test-Suite (WP 5.F.2)
* `tools/chaos`-CLI mit Sub-Commands:
  - `packet-loss --rate 0.1 --burst 5`
  - `latency --jitter 50ms`
  - `partition --duration 30s --groups A,B`
  - `clock-skew --skew 100ms`
  - `endpoint-flap --interval 5s`
* Backend: Linux `tc qdisc` + iptables fuer Network-Chaos;
  `LD_PRELOAD`-Shim fuer `clock_gettime`-Skew.
* CI-Profil `ci/jobs/chaos-soak.yml`: 30min Chaos-Soak gegen
  reliable+transient_local-Profile, Pass-Kriterium = no Sample-Loss
  + no Deadlock.

### F.3 — OpenTelemetry-Instrumentierung (WP 5.F.3)
* `crates/foundation::tracing` als Span-Source: Pub/Sub/Read/Write
  als Spans, RTPS-Submessage-Encode/Decode als Events.
* OTLP-Exporter (gRPC + HTTP) — wiederverwendbar aus
  `grpc-bridge`-Crate.
* Beispiel-Konfig: `examples/otel/jaeger-compose.yml`.
* Histogram-Metriken: `dds.write.latency`, `dds.read.latency`,
  `dds.heartbeat.rtt`, `dds.discovery.match.duration`.

### F.4 — Tauri-Dashboard (WP 5.F.4)
* `tools/dashboard`-Erweiterung: Tauri-2.0-App mit
  - Live-Topic-Liste + Pub/Sub-Counts
  - Per-Endpoint-Latency-Histogramm
  - Discovery-Graph (Participants + Endpoints) als d3-Force-Layout
  - Recording-Start/Stop + Replay-Trigger
* Datenquelle: `monitor`-Crate via Built-in-Topics
  (`DCPSParticipant`/`DCPSPublication`/`DCPSSubscription`).
* Build-Targets: macOS (universal) + Linux AppImage + Windows
  MSIX.

### F.5 — Live-Interop-Matrix-Dashboard (WP 5.F.5)
* CI-Job `ci/jobs/interop-matrix.yml` faehrt taeglich gegen
  Cyclone (latest) + FastDDS (latest) + RTI Connext eval (wenn
  Lizenz vorhanden) + OpenSplice (legacy 6.x).
* Matrix-Output als statische HTML-Seite auf `interop.zerodds.org`,
  pro Vendor + DDS-Profile (RTPS / Security / DLRL / XTypes /
  XML).
* Regressions-Alarm: GitLab-Webhook bei rotem Feld in Matrix.

---

## 4. Aufwands-Bilanz

| Cluster | Aufwand (PW) | Sequentialitaet |
|---|---|---|
| D — Real-Time | 25-35 | parallel |
| E — Distro | 15-20 | parallel |
| F — Tooling | 25-35 | parallel |
| **Total** | **65-90** | parallel |

Mit drei Spuren parallelisiert ist die End-to-End-Dauer ~max der
einzelnen Cluster + 10% Coordination = ~4-5 PM bei 1.5 FTE.

---

## 5. Sprint-Phasing

Wie Phase-4: ein Sprint = ein Cluster-WP = ein produktreifer
Release. Keine MVPs, keine Demo-Stufen.

```
Sprint 18: D.1 Hot-Path no_alloc                       ── ~6-8 PW
Sprint 19: D.2 + D.3 HW-Crypto + RT-Scheduling          ── ~7-9 PW
Sprint 20: D.4 + D.5 Lock-Free + Latency-Bench          ── ~7-10 PW
Sprint 21: E.1 rmw-zerodds-shim                         ── ~6-8 PW
Sprint 22: E.2 + E.3 Debian/RPM + crates.io             ── ~5-7 PW
Sprint 23: E.4 Yocto/Buildroot                          ── ~3-5 PW
Sprint 24: F.1 + F.2 Recording + Chaos                  ── ~8-11 PW
Sprint 25: F.3 + F.4 OTel + Tauri-Dashboard             ── ~8-11 PW
Sprint 26: F.5 Live-Interop-Matrix                      ── ~3-5 PW
```

Cross-Sprint-Abhaengigkeiten:
* Sprint 25 (F.4 Tauri-Dashboard) konsumiert Sprint 24 (F.1
  Recording-Format).
* Sprint 22 (E.3 crates.io) braucht Sprint 21 (E.1 rmw-zerodds-
  shim) als Konsumenten-Beweis.
* Cluster-D-WPs sind innerhalb des Clusters frei umsortierbar.

---

## 6. Phase-5-Acceptance

Phase 5 ist abgeschlossen, wenn:

* `tools/bench-suite::roundtrip-1us` p99 < 5µs auf llvm-Profil grün.
* `rmw_zerodds.so` laedt in ROS-2 Humble + Iron + Jazzy + besteht
  jeweils talker/listener + lifecycle-Demo.
* `apt install zerodds-tools` und `cargo install zerodds-cli`
  funktionieren von einem leeren System.
* `tools/replay` spielt eine `.zddsrec`-Aufzeichnung mit Time-
  Scale 0.1x bis 10x deterministisch ab.
* `tools/chaos packet-loss --rate 0.3` ueber 30min auf reliable
  Profil ohne Sample-Loss + ohne Deadlock.
* `tools/dashboard` zeigt einen Live-Domain mit > 50
  Participants + > 200 Endpoints unter < 100ms UI-Refresh.
* Live-Interop-Matrix gegen Cyclone latest + FastDDS latest
  durchgaengig grün.

Cross-Refs: `docs/PHASE4_CLOSEOUT.md`,
`docs/PHASE3_CLOSEOUT.md`,
`docs/REALTIME_DEPLOYMENT.md` (wird in Sprint 19 angelegt),
`project_ros2_architecture_decision.md`,
`project_security_posture.md`.
