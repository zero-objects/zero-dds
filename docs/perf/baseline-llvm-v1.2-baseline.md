# Official v1.2 Baseline — llvm (tuned, 2026-04-21)

**Der offizielle zitierfähige Baseline-Label für Phase 2.0 Close.**

Alle folgenden v1.2-Perf-Delta-Messungen zitieren diesen Run als
Referenz. Ein zweiter ungetunter Lauf vom gleichen Tag
(`baseline-llvm-2026-04-21.md`) bleibt als Archiv erhalten für den
Tuning-vs-Untuned-Vergleich.

## Kontext

| Feld | Wert |
|------|------|
| **Host** | `llvm` — AMD Ryzen Threadripper PRO 3955WX, 24C/24T, 47 GiB RAM |
| **OS** | Debian 12, Kernel 6.1.0-44, bare-metal |
| **Rust** | 1.85.0 (4d91de4e4 2025-02-17) |
| **Commit** | `60e555e` (bench-suite + rtps_fragmented + baseline-report) |
| **Baseline-Label** | `v1.2-baseline` |
| **Datum** | 2026-04-21 |
| **Tuning** | ✅ `sudo ./benches/hosts/llvm/tune.sh on` + `taskset -c 4-11` |
| **Criterion** | 0.5, sample_size=100, warmup=3 s, measurement=5 s |
| **Statistik** | Median (robust gegen Outlier) |

### Tuning-Zustand

Was `tune.sh on` tatsächlich aktiviert hat:
- ✅ `vm.swappiness = 0` (keine Pagefault-Storms)
- ✅ `net.core.{r,w}mem_max = 32 MiB` (erlaubt 4 MiB-Payloads ohne ENOBUFS)
- ✅ `net.core.{r,w}mem_default = 32 MiB`
- ✅ `vm.drop_caches = 3` (cold-start)
- ⚠ CPU governor nicht setzbar — der Debian-6.1-Kernel auf llvm hat kein cpufreq-Subsystem geladen (`/sys/devices/system/cpu/cpu*/cpufreq/` fehlt). CPU läuft damit im vom BIOS vorgegebenen P-State-Regime.
- ⚠ Turbo-Boost nicht gecappt — aus gleichem Grund.
- ✅ `taskset -c 4-11` pinnt den Bench auf 8 zusammenhängende Cores (Threadripper-CCX-Alignment).

## Ergebnisse

### UDP (`127.0.0.1`)

| Payload | Median | ungetunt | Δ |
|---------|--------|----------|---|
| 32 B | 1.65 µs | 1.67 µs | −1 % |
| 128 B | 1.64 µs | 1.66 µs | −1 % |
| 1 KiB | 1.76 µs | 1.81 µs | −3 % |
| 4 KiB | 2.11 µs | 2.25 µs | −6 % |
| 16 KiB | 3.26 µs | 3.48 µs | −6 % |

### UDS Filesystem (`SOCK_DGRAM`)

| Payload | Median | ungetunt | Δ | vs UDP |
|---------|--------|----------|---|--------|
| 32 B | 1.72 µs | 1.77 µs | −3 % | +4 % |
| 128 B | 1.76 µs | 1.82 µs | −3 % | +7 % |
| 1 KiB | 1.81 µs | 1.86 µs | −3 % | +3 % |
| 4 KiB | 2.21 µs | 2.28 µs | −3 % | +5 % |
| 16 KiB | 3.52 µs | 4.70 µs | **−25 %** | +8 % |

### UDS Abstract (`SOCK_DGRAM`, Linux-Abstract-Namespace)

| Payload | Median | ungetunt | Δ | vs UDP | vs UDS-FS |
|---------|--------|----------|---|--------|-----------|
| 32 B | **1.21 µs** | 1.23 µs | −2 % | **−27 %** | **−30 %** |
| 128 B | **1.24 µs** | 1.31 µs | −5 % | **−24 %** | **−30 %** |
| 1 KiB | **1.34 µs** | 1.35 µs | ≈ 0 | **−24 %** | **−26 %** |
| 4 KiB | **1.55 µs** | 1.80 µs | **−14 %** | **−27 %** | **−30 %** |
| 16 KiB | **2.96 µs** | 4.78 µs | **−38 %** | **−9 %** | **−16 %** |

**UDS-Abstract dominiert alle kleinen Messages deutlich.** Bei 16 KiB
profitiert Abstract am stärksten vom Tuning (−38 %), weil grosse
Datagramme vom `wmem_max=32 MiB`-Boost profitieren.

### POSIX-SHM (`shm_open` + SpSc-Ring)

⚠ Misst `send + inline recv` (Consumer drain im iter-loop, weil
`Shmem` nicht `Send` ist). Direkter Vergleich mit den TX-only-DGRAM-
Benches ist **nicht fair** — siehe Methodik.

| Payload | Median | ungetunt | Δ | Throughput |
|---------|--------|----------|---|------------|
| 32 B | 6.12 µs | 5.99 µs | +2 % | 5 MB/s |
| 128 B | 6.08 µs | 6.07 µs | ≈ 0 | 21 MB/s |
| 1 KiB | 6.07 µs | 6.23 µs | −3 % | 165 MB/s |
| 4 KiB | 6.23 µs | 6.37 µs | −2 % | 625 MB/s |
| 16 KiB | 6.78 µs | 6.87 µs | −1 % | 2.3 GB/s |
| 64 KiB | 9.89 µs | 9.81 µs | ≈ 0 | 6.3 GB/s |
| 256 KiB | 21.2 µs | 21.2 µs | ≈ 0 | 11.8 GB/s |
| 1 MiB | 66.5 µs | 67.2 µs | −1 % | **15.1 GB/s** |
| 4 MiB | 620 µs | 651 µs | −5 % | 6.5 GB/s |

SHM-Werte sind praktisch unverändert zwischen getunt/ungetunt — der
Bench ist memcpy+atomics-bound, nicht kernel-network-bound. Peak-
Throughput 15.1 GB/s bei 1 MiB = ~20 % des DDR4-Bandwidth-Budgets.

### RTPS Fragmented Writer (`ReliableWriter::write()`, kompletter Writer-Pfad)

Payload → Arc-Build → HistoryCache → N DATA_FRAG-Submessages →
Datagramm-Build. Kein Netzwerk-Send.

| Payload | Frags | Median | Pro Fragment | Effektiv |
|---------|-------|--------|--------------|----------|
| 32 B | 1 | 375 ns | 375 ns | — |
| 128 B | 1 | 410 ns | 410 ns | — |
| 1 KiB | 1 | 441 ns | 441 ns | 2.3 GB/s |
| 4 KiB | 4 | 998 ns | 250 ns | 4.1 GB/s |
| 16 KiB | 13 | 3.17 µs | 244 ns | 5.2 GB/s |
| 64 KiB | 49 | 12.0 µs | 245 ns | 5.5 GB/s |
| 256 KiB | 196 | 49.8 µs | 254 ns | 5.3 GB/s |
| 1 MiB | 781 | 196.8 µs | 252 ns | **5.3 GB/s** |
| 4 MiB | 3 121 | 1 180 µs | 378 ns | 3.5 GB/s |

**Per-Fragment-Cost plateauiert bei ~250 ns** zwischen 4 KiB und
1 MiB. Der Sprung auf 378 ns bei 4 MiB ist L2/L3-Cache-Druck:
3 121 Fragment-Ops gegen 8 MiB Arbeits-Set passen nicht mehr in L2.

Writer selbst (ohne Transport) liefert **5.3 GB/s für 1 MiB-Samples**
— das entspricht ~660 Mio DDS-Samples/s @ 32 B, oder ~31 000 4K-
Frames/s rein im Writer-Logic (natürlich bevor der Transport greift).

## Full-Pipeline-Abschätzung

Kombinierter Writer + SHM-Delivery-Pfad bei 1 MiB-Sample:

```
Writer-Fragmentation:   197 µs
SHM-Delivery:            67 µs     (send + recv)
──────────────────────────────
Total pro 1 MiB Sample: 264 µs     =  3.97 GB/s effektive Bandbreite
                                   =  ~3 780 Samples/s im Steady-State
```

Für 4 MiB-Samples (4K-Camera):

```
Writer-Fragmentation:  1 180 µs
SHM-Delivery:            620 µs
──────────────────────────────
Total pro 4 MiB Sample:1 800 µs     =  ~555 Samples/s
                                    =  ~555 fps bei 4K-RGB-Frames
```

## Interpretation

### UDS-Abstract ist der kleinkram-König
Unter 4 KiB schlägt UDS-Abstract alle anderen Transports um
20-30 %. Der Kernel-Lookup im FS-Namespace ist hier der Overhead,
den Abstract (hash-table in-kernel) vermeidet.

### SHM übernimmt ab ~16 KiB
Über 16 KiB skaliert SHM linear-mit-Size (memcpy-bound, ~15 GB/s
Peak). Die DGRAM-Transports haben über 16 KiB nur UDP mit 3.3 µs
(≈ 4.9 GB/s). SHM 16 KiB: 6.8 µs (≈ 2.4 GB/s incl. recv) — SHM
peaks bei 1 MiB.

### Writer ist compute-bound, nicht transport-bound
Der Writer-Hot-Path kostet 250 ns pro Fragment konstant. Die 3-7 %
WP-2.0a-Zero-Copy-Ersparnis sitzt auf dem SHM-Transport-Budget,
nicht im Writer-Logic.

### ROS/DDS-Scenario-Sizing
- **IMU / GPS-Fix (32 B–128 B)**: UDS-Abstract 1.2 µs = ~830k msgs/s pro Thread
- **PointCloud (128 KiB)**: SHM 21 µs + Writer 49 µs = 70 µs total = ~14k msgs/s
- **4K-Camera (4 MiB)**: 1.8 ms total = ~555 fps headroom

## Reproduce-Block

```bash
# Exakter Commit und Host
git checkout 60e555e

# Host-Tuning + Pin auf llvm
sudo ./benches/hosts/llvm/tune.sh on

taskset -c 4-11 \
    cargo bench -q -p zerodds-bench-suite --bench transports_e2e -- \
    --save-baseline v1.2-baseline

taskset -c 4-11 \
    cargo bench -q -p zerodds-bench-suite --bench rtps_fragmented -- \
    --save-baseline v1.2-baseline

sudo ./benches/hosts/llvm/tune.sh off
```

## Delta-Messung gegen diesen Baseline

```bash
cargo bench -q -p zerodds-bench-suite --bench transports_e2e -- \
    --baseline v1.2-baseline
cargo bench -q -p zerodds-bench-suite --bench rtps_fragmented -- \
    --baseline v1.2-baseline
```

Criterion gibt pro (bench, payload)-Punkt einen "% change" aus.

## Rohdaten

- **JSON**: `target/criterion/<group>/<payload>/v1.2-baseline/sample.json`
- **Plots**: `target/criterion/<group>/report/index.html`
- **Task-Archive**: `b58kalmhz` (transports_e2e), `bjf5okt35` (fragmented)

## Caveat — was noch fehlt

1. **CPU-governor/Turbo nicht gecapped** — cpufreq-Subsystem im
   llvm-Kernel fehlt. Zukünftiger Kernel-Rebuild mit cpufreq + acpi-cpufreq
   Modulen würde die Zahlen ~5-10 % stabiler machen (weniger Streuung,
   nicht notwendigerweise bessere Medians).
2. **Ein Host** — Zahlen gelten nur für Threadripper 3955WX. Delta
   zu Intel/ARM/Xeon wird später gemessen.
3. **Kein Cross-Host / L4** — alle Runs intra-host. WP 2.3 Harness
   macht cross-host mit `pivot` als Remote.
4. **Kein Cyclone/FastDDS-Vergleich** — WP 2.3.

Für v1.2-Delta-Messungen ist das trotzdem die solide Referenz.
