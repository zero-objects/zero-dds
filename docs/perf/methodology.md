# ZeroDDS Benchmark-Methodik (v1.2)

Grundlage fuer alle zitierfaehigen Perf-Zahlen in Reports unter
`docs/perf/`. Die Methodik ist an die gleichen Standards
angelehnt, die Cyclone DDS + Fast-DDS fuer ihre Baselines nutzen
(Criterion, isolated host, n=100, warmup, median+MAD).

## Payload-Achse

Fix vorgegeben — Reports zwischen Runs nur vergleichbar wenn
**gleiche Groessen**:

| Stufe | Bytes | Use-Case |
|-------|-------|----------|
| 32 B | 32 | Minimal (control-msg) |
| 128 B | 128 | Sensor-struct (IMU, GPS-Fix) |
| 1 KiB | 1 024 | Small-message-steady-state |
| 4 KiB | 4 096 | DDS-typical (Position/Velocity + payload) |
| 16 KiB | 16 384 | Medium message |
| 64 KiB | 65 536 | UDP-DGRAM-cap — letzter single-datagram-Punkt |
| 256 KiB | 262 144 | Kleine Camera-Frame |
| 1 MiB | 1 048 576 | 720p-RGB-Frame |
| 4 MiB | 4 194 304 | 4K-RGB oder LIDAR-Scan |

Quelle: `tools/bench-suite/src/lib.rs::PAYLOAD_SIZES`.

## Host-Anforderungen

Siehe `docs/plans/milestone-v1.2.md` §Bench-Hardware fuer die
fixen Hosts. Zusammengefasst:

- **Primary: llvm** — AMD Threadripper 3955WX, 24C, bare-metal
  Debian 12. Kann kernel-getuned werden. Alle Baseline-Runs hier.
- **Secondary: pivot** — LXC, kein Kernel-Tuning. Fuer L4
  Cross-Host-Szenarien nach v1.2 Harness-Aufbau.

## Pre-Bench-Tuning (Primary Host)

Reproducibility-Rezept:

```bash
sudo ./benches/hosts/llvm/tune.sh on    # governor=performance,
                                         # no_turbo, swappiness=0,
                                         # net.core.{r,w}mem_max=32MiB
taskset -c 4-11 \                        # pin zu 8 physischen Cores
    cargo bench -p zerodds-bench-suite \
    --bench transports_e2e -- \
    --save-baseline v1.2-baseline

sudo ./benches/hosts/llvm/tune.sh off    # restore defaults
```

Warum:
- **governor=performance**: verhindert Freq-Scaling mid-bench, die
  Criterion als "noise" verbucht.
- **no_turbo=1**: Turbo-boost variiert je nach Workload. Deaktiviert
  = deterministische max-freq.
- **swappiness=0**: keine Pagefault-Storms, die zufaellige Latenz-
  Spikes verursachen.
- **net.core.*_max=32 MiB**: erlaubt Socket-Puffer fuer 4 MiB-
  Payloads ohne ENOBUFS.
- **taskset -c 4-11**: pinne den Bench auf 8 Cores im gleichen CCX.

## Criterion-Konfiguration

Default-Werte, die wir **nicht** override'n:
- `sample_size = 100` — genug fuer Stabilitaet, nicht zu lang.
- `warmup_time = 3 s` — gibt dem CPU-Cache Zeit, hot zu werden.
- `measurement_time = 5 s` — ausreichend fuer 100 samples @ ns-µs-
  Zeitskala. Pro Payload-Groesse individuell, in `bench_*`
  gesetzt (5 s unser Default).
- Reports: HTML + JSON in `target/criterion/`.

**Metrik**: wir zitieren Median (P50) mit MAD-basierter Streuung.
Mean kann durch Outlier (Scheduler-hiccups, Page-faults) verfaelscht
werden — Median ist robust.

## Baseline-Snapshot-Konvention

Jeder Release bekommt einen eingefrorenen Baseline:

- `cargo bench -- --save-baseline v1.2-baseline` schreibt nach
  `target/criterion/<bench>/<case>/v1.2-baseline/`.
- Den Inhalt exportieren wir als Report unter
  `docs/perf/baseline-<host>-<yyyy-mm-dd>.md`.
- Spaetere Refactors nutzen `--baseline v1.2-baseline` fuer
  Delta-Zahlen.

## Reporting-Konvention

Jeder Report enthaelt:

1. **Bench-Kopf**: Host, Datum, Commit-Sha, Rust-Version.
2. **Tuning-Status**: war `tune.sh on` aktiv? (Ohne Tuning sind
   die Zahlen nicht zitierfaehig.)
3. **Rohdaten**: Median pro (bench, payload-size) — als Tabelle +
   `target/criterion/`-Pfade.
4. **Interpretation**: Was bedeutet der Zahl? Was wurde *nicht*
   gemessen?
5. **Reproduce-Block**: der exakte Commandline, der die Zahlen
   erzeugt hat.

Template-Text in `docs/perf/baseline-template.md` (wird gemeinsam
mit dem ersten Baseline-Run erzeugt).

## Was wir bewusst NICHT in der Suite haben

- **Warmup-Spikes nicht publiziert**: Criterion ignoriert sie per
  Design. Wenn jemand sie braucht, liest er das raw-JSON.
- **Latenz-Ping-Pong**: kommt in WP 2.3 (eigener Harness mit
  Receiver-Thread).
- **Multi-Producer/Consumer**: in WP 2.1/2.3.
- **Cross-vendor-Vergleich (Cyclone/FastDDS)**: WP 2.3.

## Anti-Patterns

- ❌ "Mein Laptop lief 30 % schneller als dein Laptop" — nicht
  zitierfaehig ohne Host-Tuning, gleiche Binary, gleicher Commit.
- ❌ Mean statt Median bei Single-Run — ein Outlier verfaelscht alles.
- ❌ "Auf meinem System" ohne Rust-Version + Kernel + CPU.
- ❌ Ad-hoc-Payload-Groessen ausserhalb der 9-Punkt-Achse.

## Wo geht's weiter

- WP 2.3 Harness: E2E-Latenz-Ping-Pong + Cyclone/FastDDS head-to-
  head mit gleichen 9 Payload-Punkten.
- Auto-Report-Generator (criterion-JSON → Markdown) bei erstem
  Cross-Repo-Vergleich.
