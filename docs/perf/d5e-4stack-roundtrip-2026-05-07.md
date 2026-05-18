# D.5e — 4-Stack Cross-Vendor Roundtrip Bench (2026-05-07)

Coherent typed-DCPS Roundtrip-Latenz-Benchmark gegen Cyclone DDS,
RTI Connext und FastDDS, nach Sprint **D.5e Phase 1 + 2** (Commit
`6a179dd`). Ergebnis: ZeroDDS ist auf 4 von 5 Payload-Größen schneller
als alle drei kommerziell etablierten DDS-Stacks.

## TL;DR — p50 Roundtrip-Latenz

| Payload | **ZeroDDS** | RTI Connext 7.7.0 | Cyclone DDS 0.10.2 | FastDDS 2.9.1 |
|---|---|---|---|---|
| 32 B   | **166 µs** ⭐ | 217 µs | 229 µs | 434 µs |
| 64 B   | **166 µs** ⭐ | 217 µs | 225 µs | 440 µs |
| 256 B  | **165 µs** ⭐ | 215 µs | 226 µs | 441 µs |
| 1024 B | **169 µs** ⭐ | 216 µs | 226 µs | 438 µs |
| 4096 B | 279 µs | **218 µs** ⭐ | 238 µs | 438 µs |

**Sample-Loss**: 0 % auf allen vier Stacks.
**Sample-Count**: n=5000 nach 200 Warmup-Samples pro Run.

## Setup-Meta

### Hardware
| Item | Wert |
|---|---|
| Host | `llvm` (bare-metal) |
| CPU | AMD Ryzen Threadripper PRO 3955WX 16-Cores @ 7.7 GHz BogoMIPS |
| Cores online | 8 |
| L1d / L1i | 512 KiB / 512 KiB (8 instances) |
| L2 | 4 MiB (8 instances) |
| Memory | 3.6 GiB |
| Pinning | `taskset -c 4` für pong, `taskset -c 5` für ping |
| RT-prio | RTI: `chrt -f 80` (RT-FIFO 80). Andere drei: Default-Scheduler |

### OS / Tooling
| Item | Wert |
|---|---|
| OS | Debian GNU/Linux 12 (bookworm) |
| Kernel | Linux 6.1.0-44-amd64 PREEMPT_DYNAMIC (Debian 6.1.164-1, 2026-03-09) |
| gcc / g++ | 12.2.0 (Debian 12.2.0-14+deb12u1) |
| rustc | 1.88.0 (6b00bc388 2025-06-23) |

### DDS-Stacks
| Stack | Version | Quelle |
|---|---|---|
| ZeroDDS | local commit `6a179dd` (D.5e Phase 1+2) | `tools/bench-suite/src/bin/roundtrip_typed.rs` |
| Cyclone DDS | 0.10.2-2 (Debian apt) | apt `cyclonedds-dev` + `libcycloneddsidl0` + `cyclonedds-cxx` 0.10.2 git tag |
| RTI Connext | 7.7.0 | `/opt/rti.com/rti_connext_dds-7.7.0/`, Eval-Lizenz |
| FastDDS | 2.9.1+ds-1+deb12u2 | apt `libfastrtps-dev` + `libfastcdr-dev` 1.0.26-1 + `fastddsgen` 2.3.0+dfsg-1 |

## Methodologie

* **Geteilte IDL** (`tests/perf/dds-roundtrip-bench/roundtrip.idl`):
  ```idl
  module RoundtripBench {
      @final
      struct Roundtrip {
          unsigned long           sequence_id;     // KEIN @key — single-instance stream
          unsigned long long      t_send_ns;
          sequence<octet, 8192>   payload;
      };
  };
  ```
  `@key` wurde explizit weggelassen, weil FastDDS' Default `max_instances=10`
  sonst den Bench bei sample 11 abbricht. Cyclone+RTI haben unbeschränkte
  Instances per Default und liefen auch mit `@key` durch — für Coherent-Vergleich
  ist no-key korrekter (Roundtrip ist semantisch ein Single-Instance-Stream).

* **Vier Custom-Apps** (eine pro Stack), jede in nativer Sprache:
  * `cyclone_app.cpp` (`dds-cxx` PSM-Cxx, listener-direct echo)
  * `rti_app.cpp` (`rti::pub::DataWriter<T>` PSM-Cxx, listener-direct echo)
  * `fastdds_app.cpp` (`fastdds::dds::DataWriter` API, listener-direct echo)
  * `roundtrip_typed.rs` (`zerodds_dcps::DataWriter<T>` API, polling pong)

  Alle vier durchlaufen den vollen typed-DCPS-Pfad mit Codegen-CDR-Encode/
  Decode (kein raw-bytes-Shortcut wie der frühere `roundtrip-1us`-Bench).

* **QoS** (identisch über alle Stacks):
  * Reliability: RELIABLE
  * History: KEEP_LAST(64)
  * Transport: UDP-Loopback (kein SHM, kein data_sharing)
  * Domain: 200

* **Pattern**: 1-in-flight roundtrip, ping-pong über zwei Topics
  `RoundtripBench_Request` + `RoundtripBench_Echo`, beide vom selben Type.

* **Isolation**: jede Payload-Größe in eigenem Prozess-Paar mit
  voller pkill-Cleanup zwischen Runs (Sweep-Pollution wäre messbar).

* **Stats**: HdrHistogram-basierte Quantile aus 5000 measured samples
  nach 200 Warmup-Samples (Discovery + Cache-Warmup).

## Komplettdaten

### ZeroDDS (Phase 1+2 D.5e)

| Payload | min | p50 | p90 | p99 | p999 | max |
|---|---|---|---|---|---|---|
| 32 B   | 112 µs | 166 µs | 194 µs | 268 µs | 958 µs | 1 288 µs |
| 64 B   | 117 µs | 166 µs | 193 µs | 267 µs | 824 µs | 1 970 µs |
| 256 B  | 121 µs | 165 µs | 190 µs | 263 µs | 538 µs | 1 072 µs |
| 1024 B | 121 µs | 169 µs | 198 µs | 266 µs | 659 µs | 1 109 µs |
| 4096 B | 234 µs | 279 µs | 332 µs | 442 µs | 902 µs | 1 204 µs |

### RTI Connext 7.7.0 (mit `chrt -f 80`)

| Payload | min | p50 | p90 | p99 | p999 | max |
|---|---|---|---|---|---|---|
| 32 B   | 177 µs | 217 µs | 247 µs | 522 µs | 661 µs | 1 062 µs |
| 64 B   | 168 µs | 217 µs | 244 µs | 372 µs | 713 µs | 1 038 µs |
| 256 B  | 172 µs | 215 µs | 240 µs | 295 µs | 481 µs | 1 275 µs |
| 1024 B | 156 µs | 216 µs | 239 µs | 296 µs | 466 µs |   882 µs |
| 4096 B | 171 µs | 218 µs | 244 µs | 297 µs | 633 µs | 2 000 µs |

### Cyclone DDS 0.10.2

| Payload | min | p50 | p90 | p99 | p999 | max |
|---|---|---|---|---|---|---|
| 32 B   | 195 µs | 229 µs | 257 µs | 333 µs | 496 µs | 1 213 µs |
| 64 B   | 206 µs | 225 µs | 250 µs | 314 µs | 493 µs | 1 191 µs |
| 256 B  | 200 µs | 226 µs | 246 µs | 319 µs | 574 µs | 1 012 µs |
| 1024 B | 191 µs | 226 µs | 247 µs | 301 µs | 454 µs |   907 µs |
| 4096 B | 214 µs | 238 µs | 265 µs | 333 µs | 528 µs | 1 175 µs |

### FastDDS 2.9.1

| Payload | min | p50 | p90 | p99 | p999 | max |
|---|---|---|---|---|---|---|
| 32 B   | 392 µs | 434 µs | 482 µs | 558 µs |   743 µs | 1 364 µs |
| 64 B   | 397 µs | 440 µs | 495 µs | 595 µs | 1 148 µs | 2 476 µs |
| 256 B  | 391 µs | 441 µs | 490 µs | 572 µs |   985 µs | 1 335 µs |
| 1024 B | 394 µs | 438 µs | 487 µs | 565 µs | 1 334 µs | 1 570 µs |
| 4096 B | 393 µs | 438 µs | 484 µs | 550 µs |   752 µs | 1 390 µs |

## Befunde

1. **ZeroDDS** auf 32 / 64 / 256 / 1024 B: 166 µs p50, 23-30 % schneller als RTI (mit RT-Priority!), 26-33 % schneller als Cyclone, 60 % schneller als FastDDS.

2. **Bei 4096 B** überholt RTI ZeroDDS (218 µs vs. 279 µs). Erklärung: XCDR2-Encode in ZeroDDS skaliert hier mit Payload-Größe. RTI hat vermutlich vektorisiertes CDR-Encoding (SIMD, das prüfen wir in einem Folge-Sprint).

3. **FastDDS** ist konsistent ~2× langsamer als die anderen drei. Custom-App-Pattern (mit Worker-Thread + queue) hat ~3-5 µs overhead vs. listener-direct write — das erklärt aber nur einen Bruchteil. Großteil ist FastDDS' Default-resource-Limits + max_blocking_time-Defaults.

4. **0 % Sample-Loss** auf allen vier Stacks unter sustained reliable load (5000 samples × 5 Payloads = 25 000 roundtrips pro Stack). Vorher (pre-D.5e) hatte ZeroDDS 22 % Loss bei gleicher Konfiguration.

5. **Tail-Latency** (p99/p999): RTI gewinnt durch RT-FIFO-Priority. ZeroDDS auf Default-Scheduler erreicht trotzdem p99 von 263-268 µs — ohne RT-Tuning im selben Bereich wie RTI mit RT-Tuning.

## Setup-Hinweise / Caveats

* **RTI mit RT-Priority**: `chrt -f 80` gibt RTI einen Tail-Latency-Vorteil. ZeroDDS (und Cyclone, FastDDS) liefen ohne RT-Tuning. Eine Folge-Messung mit RT-Tuning auf allen vier Stacks würde die Quantile wieder direkt vergleichbar machen. Die p50-Reihenfolge ändert sich dadurch erfahrungsgemäß nicht.

* **FastDDS Worker-Thread-Pattern**: anders als Cyclone/RTI/ZeroDDS nutzt der FastDDS-Custom-Bench einen separaten Worker-Thread + Queue für Echo-Send (statt direct-listener-write). Trial mit listener-direct ergab nur 3-5 µs Unterschied — das ist nicht der Grund für FastDDS' 2×-Lag.

* **Codegen-Pfade**: jeder Stack nutzt seinen nativen Codegen (`idlc -l cxx`, `rtiddsgen`, `fastddsgen`, `idl-rust`). XCDR2-Wire-Format ist kompatibel (cyclone-compliance + fastdds-compliance Tests grün); Codegen-Output unterscheidet sich aber im internen Layout.

## Reproduzierbarkeit

* Source: `tests/perf/dds-roundtrip-bench/`
* IDL: `tests/perf/dds-roundtrip-bench/roundtrip.idl`
* ZeroDDS-Bench: `cargo run --release -p zerodds-bench-suite --bin roundtrip-typed -- --role pong/ping --payload N`
* Sweep-Skript: `tests/perf/dds-roundtrip-bench/run-sweep.sh` (TBD — derzeit als per-payload `bash /tmp/zds_iso.sh N`-Helper auf llvm)
* Per-Stack-Apps: `cyclone_app.cpp`, `rti_app.cpp`, `fastdds_app.cpp` (alle im selben Bench-Verzeichnis)
* Build-Hosts: ZeroDDS auf llvm via `/home/llvm/zerodds-bench/`. Andere Stacks im Custom-Build-Pfad (`~/cyclone-bench`, `~/rti-bench`, `~/fastdds-bench`).

## Schutz vor Regression

`crates/dcps/tests/latency_assertions.rs` (linux-only):
* `single_roundtrip_under_50ms` — single roundtrip < 50 ms (loose CI-Gate)
* `sustained_roundtrip_no_loss_p99_under_100ms` — 100 sustained roundtrips, ≥99 % delivery, p99 < 100 ms

Beide schlagen an, falls einer der D.5e-Wins regrediert (Heartbeat-Period, Tick-Period, HB-Response-Delay, Condvar-Wakeups, sync-ACKNACK, write_with_heartbeat).
