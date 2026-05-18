# roundtrip-1us — Baseline-Numbers

D.5-Tool: `tools/bench-suite/src/bin/roundtrip_1us.rs` — busy-poll UDP-
Roundtrip-Latenz, hdrhistogram-basiert.

## Reference-Run llvm vanilla (2026-05-03)

Host: llvm.sandra-kessler.eu — Debian 12 / Linux 6.1.0-44 (vanilla, **kein**
preempt_rt). UDP-Loopback (127.0.0.1:17400 ↔ 17401), 64-Byte-Payload,
5 000 Warmup + 100 000 Samples, free-running busy-poll, **kein**
CPU-Pinning, **kein** `chrt -f`.

| Quantil  | Latenz |
| -------- | ------ |
| min      | 18 µs  |
| p50      | 25 µs  |
| p90      | 27 µs  |
| p99      | 40 µs  |
| p99.9    | 85 µs  |
| p99.99   | 324 µs |
| p99.999  | 929 µs |
| max      | 995 µs |

Das ist **Linux-UDP-Loopback ohne RT-Tuning**. Der Standard-Scheduler
(CFS) führt zu sporadic context-switches, das p99-Tail wandert in den
zwei- bis dreistelligen µs-Bereich.

## CI-Gate-Profil (D.5-Plan)

Ziel laut `docs/PHASE5_PLAN.md` (Cluster D.5):

| Quantil | Schwelle |
| ------- | -------- |
| p99     | < 5 µs   |
| p99.9   | < 20 µs  |
| p99.99  | < 100 µs |

Erforderliche Host-Konfiguration:

* Linux 6.x mit `PREEMPT_RT`-Patches (oder Mainline-RT-Kernel
  ab 6.12)
* `isolcpus=2-7 nohz_full=2-7 rcu_nocbs=2-7` Boot-Args
* Pong und Ping je `taskset -c 2/3 chrt -f 80 ...`
* HW-Anforderung: 1 GbE direkt verbunden, kein Switch im Pfad
* `tx-queue-len=10000` + `net.core.busy_poll=50`

Unter diesen Bedingungen sind sub-µs RTTs auf 64 B-Payloads erreichbar
(siehe Apex.AI's Apex-OS-Performance-Reports zu Vergleich). Auf
ZeroDDS' aktuellem Stack (ohne DCPS-Discovery, reines UDP) ist das das
Floor; jeder oberhalb liegende Software-Layer addiert messbar.

## Folgende Sub-Sprints

* **D.5b** (aktuell): `--use-dcps`-Flag pumpt durch DcpsRuntime statt
  rohes UDP. Verwendet `zerodds-c-api`-FFI-Hub + best-effort QoS. Erste
  Live-Smokes zeigen:
  - Discovery + Bidirektionales Matching (writer + reader): ✓
  - Pub-Sub-Echo-Roundtrip via DCPS-Stack: ✓
  - Latenzen aktuell 5-14 s p50 unter 200 Hz Last auf vanilla Linux
    (kein RT-Tuning, kein `chrt -f`). D.4-Phase-C atomic HistoryCache
    sollte das massiv druecken.
  - Stabilisierungs-Pause + Queue-Drain nach `wait_for_matched` ist
    Pflicht, sonst schlucken die ersten Samples die Discovery-Latenz.
  - Real-Numbers stehen aus, sobald llvm preempt_rt + isolcpus laeuft
    und der parallele D.4-Refactor in den Hot-Path durchgesickert ist.
* **D.5c**: Apex.AI `performance_test --communication ZeroDDS` mit
  voller Plugin-Pipeline (Phase-2 nach typisierter Plugin-Layer).
* **D.5d**: Cross-Vendor Side-by-Side gegen `ddsperf` (Cyclone) und
  `LatencyTestPublisher` (FastDDS) — gleicher Host, gleiche Workload,
  gleiches Histogramm-Format.

## D.5b — Aufrufbeispiel

```bash
# Pong (Echo-Endpoint)
roundtrip-1us --role pong --use-dcps --dcps-domain 0 \
              --dcps-topic latency --max-runtime 60

# Ping (Mess-Endpoint)
roundtrip-1us --role ping --use-dcps --dcps-domain 0 \
              --dcps-topic latency \
              --rate 100 --warmup 200 --samples 5000 \
              --hgrm /tmp/dcps-rtt.hgrm
```

Topic-Default = `roundtrip-{pid}` damit parallele Test-Runs sich nicht
stoeren. Pong und Ping muessen den gleichen `--dcps-topic` und
`--dcps-domain` benutzen.

## Reproduktion

Siehe Doc-Comment in `tools/bench-suite/src/bin/roundtrip_1us.rs`.
Kurzform:

```bash
# Pong-Seite (echo)
cargo run -p zerodds-bench-suite --release --bin roundtrip-1us -- \
  --role pong --bind 0.0.0.0:7400

# Ping-Seite (measure)
cargo run -p zerodds-bench-suite --release --bin roundtrip-1us -- \
  --role ping --remote 192.0.2.10:7400 --bind 0.0.0.0:7401 \
  --warmup 5000 --samples 100000 \
  --hgrm /tmp/zerodds-rtt.hgrm
```

Die `.hgrm`-Datei laesst sich mit
[`HdrHistogramVisualizer`](https://github.com/HdrHistogram/HdrHistogramVisualizer)
oder dem `hdrplot`-Skript visualisieren (Custom-V2-Text-Format mit
Wert,Perzentil,Count-Tripeln).
