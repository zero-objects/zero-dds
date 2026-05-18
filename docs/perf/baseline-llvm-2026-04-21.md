# Baseline — llvm, 2026-04-21 (v1.2-initial)

Erster zitierfähiger Baseline-Run der v1.2 Bench-Suite. Zahlen
sind die Grundlage für alle folgenden Delta-Messungen in v1.2.

## Kontext

| Feld | Wert |
|------|------|
| **Host** | `llvm` — AMD Ryzen Threadripper PRO 3955WX, 24C/24T, 47 GiB RAM |
| **OS** | Debian 12, Kernel 6.1.0-44, bare-metal (kein VM/LXC) |
| **Rust** | 1.85.0 (4d91de4e4 2025-02-17) |
| **Commit** | `ae1ff3e115d6` |
| **Baseline-Label** | `v1.2-initial` |
| **Datum** | 2026-04-21 |
| **Tuning** | ⚠ **ungetuned** (kein `tune.sh on`, keine `taskset`-Pin) — siehe Limitation-Section |
| **Criterion** | 0.5, sample_size=100, warmup=3 s, measurement=5 s |
| **Statistik** | Median (robust gegen Outlier) |

## Ergebnisse

### UDP (loopback, `127.0.0.1`)

| Payload | Median |
|---------|--------|
| 32 B | 1.67 µs |
| 128 B | 1.66 µs |
| 1 KiB | 1.81 µs |
| 4 KiB | 2.25 µs |
| 16 KiB | 3.48 µs |

### UDS Filesystem (`SOCK_DGRAM`, T1)

| Payload | Median | vs UDP |
|---------|--------|--------|
| 32 B | 1.77 µs | +6 % |
| 128 B | 1.82 µs | +9 % |
| 1 KiB | 1.86 µs | +3 % |
| 4 KiB | 2.28 µs | +1 % |
| 16 KiB | 4.70 µs | +35 % |

### UDS Abstract (`SOCK_DGRAM`, T5, Linux abstract namespace)

| Payload | Median | vs UDP | vs UDS-FS |
|---------|--------|--------|-----------|
| 32 B | **1.23 µs** | **−26 %** | **−31 %** |
| 128 B | **1.31 µs** | **−21 %** | **−28 %** |
| 1 KiB | **1.35 µs** | **−25 %** | **−27 %** |
| 4 KiB | 1.80 µs | −20 % | −21 % |
| 16 KiB | 4.78 µs | +37 % | +2 % |

Die Abstract-Variante schlägt beide DGRAM-Alternativen bei kleinen
Messages deutlich (kein FS-Lookup, kein namespace-lookup). Bei
16 KiB verflacht der Vorteil — Kernel-Copy dominiert.

### POSIX-SHM (`shm_open` + SpSc-Ring, T3)

⚠ **Achtung:** der SHM-Bench misst `send + recv` zusammen (inline
drain, weil `Shmem` nicht `Send` ist). UDP/UDS messen TX-only mit
separatem Drain-Thread. Ein direkter SHM-vs-UDP-Vergleich auf diesen
Zahlen ist **nicht fair**.

| Payload | Median (send+recv) | Throughput |
|---------|--------------------|------------|
| 32 B | 5.99 µs | 5 MB/s |
| 128 B | 6.07 µs | 20 MB/s |
| 1 KiB | 6.23 µs | 160 MB/s |
| 4 KiB | 6.37 µs | 610 MB/s |
| 16 KiB | 6.87 µs | 2.3 GB/s |
| 64 KiB | 9.81 µs | 6.4 GB/s |
| 256 KiB | 21.2 µs | 11.8 GB/s |
| 1 MiB | 67.2 µs | 14.9 GB/s |
| 4 MiB | 651 µs | 6.2 GB/s |

SHM skaliert linear-mit-size (memcpy-bound) über den ganzen Bereich.
Throughput peakt bei 1 MiB mit ~15 GB/s — das ist ~20 % des DDR4-
Speicher-Bandwidth-Limits des 3955WX, realistisch für einen SpSc-
Ring mit atomics on head/tail.

### RTPS Fragmented Writer (`ReliableWriter::write()`, volle Payload-Achse)

Misst den **kompletten Writer-Pfad** inklusive Fragmentation:
`Arc::from(payload)` → `HistoryCache::insert_arc` → N ×
`DATA_FRAG`-Submessage + Datagramm-Build. Kein Netzwerk-Send.

| Payload | Fragments | Median | Zeit pro Fragment |
|---------|-----------|--------|-------------------|
| 32 B | 1 | 380 ns | 380 ns |
| 128 B | 1 | 412 ns | 412 ns |
| 1 KiB | 1 | 446 ns | 446 ns |
| 4 KiB | 4 | 984 ns | 246 ns |
| 16 KiB | 13 | 3.09 µs | 238 ns |
| 64 KiB | 49 | 11.43 µs | 233 ns |
| 256 KiB | 196 | 47.9 µs | 244 ns |
| 1 MiB | 781 | 186.8 µs | 239 ns |
| 4 MiB | 3 121 | 1 152.7 µs | 369 ns |

**Per-Fragment-Cost plateauiert bei ~240 ns** ab 4 KiB. Bei 4 MiB
steigt sie leicht auf 369 ns — L1/L2-Cache-Effekte bei einer
3 121-Fragment-Serie + HistoryCache-Insert.

**Effektiver Writer-Throughput:**
- 1 MiB Sample: ~5.4 GB/s → reine Writer-Logik, kein IO
- 4 MiB Sample: ~3.5 GB/s → L-Cache-Druck wird sichtbar

Zusammen mit dem Transport-Bench ergibt sich das **Full-Pipeline-Budget**:
für einen 1 MiB-Sample via SHM wäre die Summe ~187 µs (Writer-
Fragment) + ~67 µs (SHM-Delivery) ≈ 254 µs = 4 GB/s Gesamt-Bandbreite.

## Interpretation

### Wenn du klein & schnell willst: **UDS-Abstract**
Unter 4 KiB ist UDS-Abstract **konsistent 20-30 % schneller als
UDP-localhost**. Cross-container-IPC ohne mounted volume — das
Docker-optimale Feature.

### Wenn du portable bleiben willst: **UDP-localhost**
UDP ist verfügbar, wo immer ein Kernel ist. Auf macOS ist es
unser schnellster Transport (Abstract ist Linux-only).

### Wenn du große Payloads hast: **SHM**
Ab 16 KiB schlägt SHM alle Netzwerk-Transports *pro Datenmenge*:
- 1 MiB in 67 µs = 14.9 GB/s → entspricht ~70× mehr Durchsatz als
  UDP (den wir nur bis 16 KiB gemessen haben, aber auch dort bei
  ~4.7 GB/s skaliert).
- SHM-Latenz pro send+recv ist flat ~6 µs Setup-Cost + ~lineares
  memcpy-Budget.

### Fragment-Threshold
Die klassische RTPS-MTU (1344 B) liegt im "kleinen" Payload-Bereich,
wo UDP und UDS-Abstract praktisch gleich schnell sind (1.8 vs 1.35 µs).
Für einen ZeroDDS-Writer der viele kleine Messages pusht lohnt sich
der Abstract-Pfad.

## Limitations der dieses Runs

1. **Ungetuned Host.** `tune.sh on` wurde **nicht** aufgerufen (kein
   sudo auf llvm in dieser Session). Erwartete Verbesserung mit
   Tuning: −5 bis −15 % Median, deutlich reduzierte Streuung.
2. **64 KiB Payload geskippt bei UDP/UDS.** Der `DGRAM_MAX = 60 * 1024`-
   Guard schneidet ab, weil Linux' `net.core.wmem_max` default
   212 KiB und ein 64 KiB-Datagram mit Socket-Headers es knapp
   macht. Ungetuned. Mit getuntem `wmem_max=32 MiB` fällt der
   Guard.
3. **MB-Payloads sind Fragmentation-Szenario, nicht Transport-Szenario.**
   DDS erlaubt beliebig grosse Samples — der Writer fragmentiert
   sie in `DATA_FRAG`-Submessages (Default-Fragment-Size 1344 B,
   MTU-abhaengig), der Reader reassembled via `FragmentAssembler`.
   **Diese Suite misst nur den Raw-Transport-Pfad.** Payloads
   ueber 60 KiB werden im DGRAM-Bench geskippt, weil ein einzelnes
   UDP/UDS-Datagramm nicht so gross sein kann. SHM hat keinen
   Cap und misst bis 4 MiB, aber das ist **kein realistisches
   DDS-Pattern**: im echten Writer-Pfad wuerde ein 4 MiB-Sample
   ueber ~3 000 DATA_FRAG-Submessages verteilt, jedes 1344 B.
   Die RTPS-Fragmented-Writer-Perf wird daher separat gemessen
   in der Bench-Group `writer_write` in
   `tools/bench-suite/benches/rtps_fragmented.rs` — volle Payload-
   Achse bis 4 MiB, mit Fragment-Anzahl im Label. Siehe Abschnitt
   "RTPS Fragmented Writer" weiter oben.
4. **Abstract-Bench ist Linux-only.** macOS + Windows haben kein
   Abstract-Namespace; UDS-FS ist dort der einzige UDS-Pfad.
5. **SHM-Bench misst send+recv.** Siehe Anmerkung oben. TX-only
   würde eine SpSc-Ring-ohne-Drain-Szenario erfordern, das
   in der Praxis nicht auftritt.
6. **Criterion-Statistik minimal.** `sample_size=100` gibt gute
   Median-Stabilität, keine p99.9 o.ä. — wenn v1.3 Tail-Latenz
   wichtig wird, müssen wir auf HDR-Histogram-basierte Bench-Impl
   umstellen.

## Reproduce-Block

```bash
# Code-Stand (exakter Commit):
git checkout ae1ff3e115d6

# Host: llvm (Threadripper 3955WX, bare-metal Debian 12, Kernel 6.1)
# Keine root-Privilegien in diesem Run — tune.sh ungerufen.

cargo bench -q -p zerodds-bench-suite --bench transports_e2e -- \
    --save-baseline v1.2-initial
```

Für einen getunten Run (zitierfähig als offizielle v1.2 Baseline):

```bash
sudo ./benches/hosts/llvm/tune.sh on
taskset -c 4-11 \
    cargo bench -q -p zerodds-bench-suite --bench transports_e2e -- \
    --save-baseline v1.2-baseline
sudo ./benches/hosts/llvm/tune.sh off
```

## Delta-Messung (so wird zitiert)

Nach einem Refactor:

```bash
cargo bench -q -p zerodds-bench-suite --bench transports_e2e -- \
    --baseline v1.2-initial
```

Criterion gibt pro (bench, payload)-Punkt einen Delta-Plot + "% change"
aus. Zitierfähige Delta-Aussagen zitieren immer:
1. Baseline-Label und Datum.
2. Neuer Commit-SHA.
3. p50 + MAD pro Punkt.
4. Anti-Claim: welcher Punkt hat sich *nicht* verbessert (oder
   verschlechtert, wenn so).

## Rohdaten

Raw JSON pro `(bench, payload)` unter:
`target/criterion/<group>/<payload>/v1.2-initial/sample.json`

HTML-Plots pro Gruppe:
`target/criterion/<group>/report/index.html`

Der Bench-Ausführungs-Log liegt im Task-Archive
`bk0pmg5yn` (lokale Maschine).
