# CI-Test-Images — Roadmap

Ziel: Volle Interop-Suite + Speed-Tests bei jedem CI-Run, ohne manuelle Triggers.

## Ist-Stand (2026-05-01)

**Vorhanden:**
- `ci/Dockerfile.rust` — CI-Image mit Rust 1.85 + Cyclone DDS + Fast-DDS-Dev-Pakete
  installiert. Image-Build triggert nur bei Dockerfile-Änderung.
- `live-interop`-Stage in GitLab-CI: SPDP-Discovery gegen Cyclone+FastDDS.
  **Manual-Trigger only** mit `allow_failure: true`.
- SSH-Bench-Host `llvm@llvm` mit nativen FastDDS+Cyclone-Installs (kein Docker-
  Overhead, stabile Mess-Runs).

**Lücken:**
1. Interop ist nur SPDP-Discovery, kein Pub/Sub-Wire-Roundtrip mit QoS-Compat-Matrix.
2. Kein Speed-Test im CI — Performance-Regressionen rutschen durch.
3. Manual-Trigger heißt: pro Default-Run laufen die wichtigsten Vendor-Tests nicht.
4. Bench-Host wird nicht automatisch von CI angesteuert.

## Plan

### Welle CI-1 — Auto-Interop bei jedem CI-Run (~1 Tag)

**Ziel:** `live-interop`-Stage von manual auf automatic stellen, Multi-Vendor-Pub/Sub-Roundtrip dazu.

- Erweitere `tests/interop/`:
  - `pub_sub_roundtrip.sh` — ZeroDDS-Pub → Cyclone-Sub, Cyclone-Pub → ZeroDDS-Sub, FastDDS analog
  - `qos_matrix.sh` — Reliability x Durability x History-Combos
  - `late_joiner.sh` — TransientLocal + Lifespan late-attach
- GitLab-CI:
  - `live-interop` von `when: manual` zu `when: on_success` für `main` + Default-Branches
  - `allow_failure: false` (echter Gate)
  - Timeout 10 min (jetzt unlimitiert)
- Image-Erweiterung: `ddsperf` + `zerodds-rtps-tools` (Cyclone) + `fastdds_perf` (FastDDS)
  vorinstalliert.

### Welle CI-2 — Speed-Test bei jedem CI-Run (~1 Tag)

**Ziel:** Criterion-Bench-Suite (8 Crates / 28 individuelle benches) automatisiert mit Regression-Alert.

- Neue Stage `bench`:
  - `cargo bench --workspace --no-run` (compile-only sanity)
  - Auf `main`: `cargo bench --workspace -- --save-baseline pre`
  - Auf PRs: `cargo bench --workspace -- --baseline pre` mit Regression-Threshold 10%
- Bench-Output als Artifact (HTML-Reports)
- Bench-Regression-Detection: parsed Criterion-Output; bei `> +10% time` als Failure.

### Welle CI-3 — SSH-Bench-Host für stabile Mess-Runs (~0.5 Tag)

**Ziel:** Stabile Mess-Umgebung für Apex.AI performance_test ohne Docker-Overhead.

- GitLab-CI nutzt SSH-Runner-Tag für Native-Bench-Job:
  - `ssh llvm@llvm "cd /opt/zerodds-bench && ./run_perf.sh"`
  - Output: latency.json + throughput.json
- `tests/perf/run_apex_ai.sh` — Apex.AI performance_test gegen ZeroDDS, Cyclone, FastDDS
  parallel, gleiches Topic
- Resultate in `docs/perf/baseline-<git-sha>.md` mit Latenz + Throughput

### Welle CI-4 — Soak-Tests (24h, ~0.5 Tag Setup, dann nightly)

**Ziel:** Memory-Leak + Discovery-Stabilität unter 24h-Last.

- GitLab-CI Schedule (nightly): Pipeline mit Soak-Job
  - `tests/soak/24h_pubsub.sh` — ZeroDDS pub/sub mit 100 endpoints, monitor heap+rss
  - heaptrack profile + valgrind (massif) on exit
- Pivot-host als dedicated Soak-Runner

## Container-Image-Layer (current → target)

| Layer | Current | Target Welle |
|---|---|---|
| Rust + cargo-tools | ✓ | — |
| Cyclone DDS dev | ✓ | — |
| Fast-DDS dev | ✓ | — |
| ddsperf | ✓ | — |
| Apex.AI performance_test | — | CI-2 / CI-3 |
| valgrind + heaptrack | — | CI-4 |
| RTI Connext (eval) | — | TS-2 (separate image) |
| ASAN/MSAN nightly variant | — | TS-1 follow-up |

## Welche Welle zuerst?

**Empfehlung: CI-1 (Auto-Interop).** Höchster Impact: holt die existing
Cyclone+FastDDS-Tests aus dem Manual-Bucket, fängt Wire-Drift bei jedem PR.
Niedriger Aufwand: Image hat bereits alles nötige.

CI-2 (Speed-Test) als unmittelbarer Follow-up für Bench-Regression-Detection.

CI-3 (SSH-Bench) und CI-4 (Soak) als langfristige Welle.

## CI-1 Status — abgeschlossen 2026-05-01

`live-interop`-Job in `.gitlab-ci.yml` umgestellt:

* `when: manual + allow_failure: true` → `when: on_success + allow_failure: false`
  für `main` + `feat/wp-0.7a-*`-Branches (echter Gate).
* Andere Feature-Branches + Merge-Requests bleiben `manual`/`allow_failure: true`
  (verhindert dass Multicast-Probleme auf Entwicklungs-Branches den Default
  rotmachen).
* 15 min Timeout statt unlimitiert.

**Neuer Job führt drei Suiten:**

1. **SPDP-Discovery** (legacy smoke, 30 s) — bewahrt das alte Verhalten.
2. **Cross-Vendor-Pub/Sub-Roundtrip** via `tests/interop/xv_pub_sub_roundtrip.sh`
   — bidirektional ZeroDDS↔Cyclone mit Sample-Delivery-Check (≥5 Samples
   pro Richtung, sonst Fail).
3. **Cargo-Live-Tests** mit `--features live-interop`:
   - `fastdds_qos_matrix` — RxO-Matrix Reliability×Durability
   - `fastdds_live_sub` + `fastdds_live_pub` — bidirektionaler Pub/Sub
   - `cyclone_live_wlp` — Liveliness-Protocol cross-vendor
   `--test-threads=1` für Multicast-Isolation.

**Artifacts:** `interop-artifacts/{demo.out,cyclone.log,fastdds.log,
cargo_live.log,xv-roundtrip-out/}` — 7 Tage retention.

**Verifikation lokal:** Shell-Syntax (`bash -n`) + cargo check mit
`--features live-interop` ok. Live-Run findet auf dem Linux-CI-Runner
statt (macOS-Multicast-Loopback unzuverlässig); Erstlauf wird neue
Findings produzieren — die werden in `plan.md` dokumentiert.

**Workspace-Regression-Check:** 6879 passed / 0 failed / 0 clippy-warnings
nach allen Änderungen.

## CI-2 Status — abgeschlossen 2026-05-01

Neue Stage `bench` zwischen `test` und `docs` mit drei Jobs:

* **`bench-compile`** — `cargo bench --workspace --no-run`. Läuft auf
  jedem Branch + MR. Fängt Bench-Compile-Errors (z.B. API-Breaks in
  benchmarked Pfaden) sofort, ohne 5-10 min Bench-Run-Cost.
* **`bench-main`** — nur auf `main`. Voller Bench-Run mit
  `--save-baseline pre`, archiviert `target/criterion/` + `bench-output.log`
  als 30-Tage-Artifakt. Manueller Re-Run via `RUN_BENCHES=true`-Pipeline-Var.
* **`bench-compare`** — Manueller Trigger pro Feature-Branch / MR (oder
  `RUN_BENCH_COMPARE=true`-Var). Lädt Baseline von letzter erfolgreicher
  `bench-main`-Pipeline auf main via GitLab-API
  (`jobs/artifacts/<ref>/download?job=bench-main`), läuft Bench, vergleicht
  via `tests/perf/check_bench_regressions.py`. **Fail bei Regression > 10%
  mit nicht-überlappenden 95%-Confidence-Intervals** (Anti-Flap).
  `allow_failure: true` während Lernphase.

**Manueller Trigger statt automatisch** weil:
1. Shared Runner (glr1) — Bench-Runs untereinander + Build-Last verfälschen
   Messungen.
2. Auto-Run auf jedem PR-Push wäre vermeidbare Reibung.

**Use-Case:** vor Merge eines perf-relevanten PR.

**Bench-Targets (8 Suiten):** rtps:writer_dispatch, rtps:decode_hotpaths,
cdr:encode_decode_hotpaths, idl:parse_hotpaths, xml:parse_hotpaths,
amqp-bridge:decode_hotpaths, xrce:decode_hotpaths, transport-udp:vectored_send.

**Parser-Verifikation (`tests/perf/check_bench_regressions.py`):**

| Szenario | Erwartet | Ergebnis |
|---|---|---|
| Identische Daten | PASS, alle flat | ✓ |
| 30% Regression, CIs nicht überlappend | FAIL exit=1 | ✓ |
| 50% Improvement, CIs nicht überlappend | PASS, 1 improvement | ✓ |
| 5% Regression < threshold | PASS, flat | ✓ |
| 30% Regression mit überlappenden CIs (Rauschen) | PASS, flat (Anti-Flap) | ✓ |

## CI-3 Status — abgeschlossen 2026-05-01

`bench-llvm`-Job: SSH zum Bare-Metal-Host `llvm@llvm`, läuft
`tests/perf/llvm_bench_runner.sh` nativ (kein Docker-Overhead).

**Was läuft pro Run:**

1. **Criterion-Suite** voll mit `--save-baseline llvm-<sha>` — 8 Suiten
   auf 24-Core-Bare-Metal, stabile Mess-Umgebung.
2. **ddsperf-Latenz** — 60 s Cyclone ping/pong, 1 KB samples. Geparst
   in `latency_cyclone.json` (min/mean/p50/p90/p99/max in µs, median
   über alle Sekunden).
3. **ddsperf-Throughput** — 60 s Cyclone pub/sub, 1 KB samples. Geparst
   in `throughput_cyclone.json` (kS/s + Mb/s, median + max, lost-count).
4. **Markdown-Summary** `bench-summary.md` mit allen Werten.

**Trigger:** manuell pro Branch (oder `RUN_BENCH_LLVM=true`-Pipeline-Var);
kein Auto-Run weil bench-Host shared workload hat.

**Setup-Doku:** `docs/test-harness/llvm-host-setup.md` (Inventar +
SSH-Deploy-Key + GitLab-Variablen `LLVM_BENCH_SSH_KEY` / `LLVM_BENCH_HOST_KEY`).

**Real-World-Sanity-Check 2026-05-01 (8 s mini-Run):**

| Metrik | Wert |
|---|--:|
| ddsperf ping/pong, 1 KB, median latency | 136 µs |
| ddsperf pub/sub, 1 KB, median rate | 57.3 kS/s |
| ddsperf pub/sub, 1 KB, median bandwidth | 470 Mb/s |
| samples lost | 0 |

Regex-Format gegen echtes ddsperf-2.x output validiert (sowohl Latency-
als auch Throughput-Parser).

**Folge-Welle CI-3b (offen):** ZeroDDS-eigener `ddsperf`-Adapter
(aktuell nur Cyclone-Self als Reference); Apex.AI `performance_test`
Cross-Vendor-Setup (~1-2 PT).

## CI-4 Status — abgeschlossen 2026-05-01

`soak-pivot`-Job: SSH zu `bench@pivot` (LXC, 20 Cores, 128 GB RAM),
führt `tests/perf/soak_runner.sh` aus. Default 24 h, override via
`SOAK_RUNTIME_SECS`-Pipeline-Var.

**Was läuft pro Run:**

1. Workspace-Clone, `cargo build --release` der Shapes-Demo-Examples
2. Subscriber starten, dann Publisher (Late-Joiner-Loss vermeiden)
3. RSS+sample-count alle 60 s in `rss-timeline.csv` schreiben
4. Sample-Stillstand-Detektion: > 5 × Intervall ohne neue Samples = FAIL
5. Auswertung: median(early-steady) vs median(late-steady) — bei
   Wachstum > 25 % Memory-Leak-FAIL

**Steady-State-Definition:** nach Startup-Cutoff (10 min oder runtime/4),
die ersten 50 % vs die letzten 50 % der Steady-State-Phase. Frühe
Implementation hat median über die ganze Steady-State-Phase berechnet —
Bug: bei langsamen Leaks zog der wachsende Median den Vergleich mit dem
End-Wert klein. Behoben mit early/late-Window-Vergleich; verifiziert
mit drei synthetischen Szenarien (stable=PASS, leak +30%/+12%=FAIL,
no-samples=FAIL).

**Trigger:** ausschließlich Schedule (nightly 02:00 UTC) oder manuell
mit `RUN_SOAK=true`. 26 h Job-Timeout. Artefakte 90 Tage retention.

**Setup-Doku:** `docs/test-harness/pivot-host-setup.md`
(User-Anlage, SSH-Deploy-Key, Schedule-Konfiguration).

**Folge-Welle CI-4b (offen):** heaptrack/valgrind-Profile,
Multi-Endpoint-Soak (100 Endpoints), Cross-Vendor-Soak (ZeroDDS-Sub vs
Cyclone-Pub 24 h).
