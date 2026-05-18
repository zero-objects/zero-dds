# D.5b-Refit Follow-up — DCPS-Roundtrip nach Per-Socket-Threads

**Datum:** 2026-05-07
**Refit-Commit:** `871f79a refactor(dcps): per-socket recv-threads + dedicated tick-thread (D.5b)`
**Bench-Host:** `llvm.sandra-kessler.eu` — Linux 6.1.0-44 vanilla, Debian 12,
**kein** PREEMPT_RT, **kein** CPU-Pinning, **kein** `chrt -f`.
**Loopback:** `127.0.0.1`, UDP, gleicher Pfad bei beiden Stacks.

## Setup

ZeroDDS-Source frisch nach llvm rsynchronisiert
(`~/projects/zerodds-fresh/`). Build:

```
cargo build --release -p zerodds-bench-suite --bin roundtrip-1us
# Finished `release` profile [optimized + debuginfo] target(s) in 28.01s
```

Bench-Run pro Payload-Größe:

```
roundtrip-1us --role pong --bind 127.0.0.1:17500 \
              --use-dcps --dcps-domain 200 --dcps-topic rt1us-<SZ> \
              --payload <SZ> --max-runtime 25 &

roundtrip-1us --role ping --remote 127.0.0.1:17500 \
              --bind 127.0.0.1:17501 \
              --use-dcps --dcps-domain 200 --dcps-topic rt1us-<SZ> \
              --payload <SZ> --warmup 200 --samples 5000 \
              --max-runtime 25
```

`--use-dcps` pumpt durch `zerodds-c-api`-FFI-Hub +
`DcpsRuntime` (= der Refit-Pfad mit per-Socket-Recv-Threads).

## Roh-Daten

### ZeroDDS DCPS — Roundtrip post-refit (n=5 000 pro Run)

| Payload | min | p50 | p90 | p99 | p99.9 | p99.99 | max |
|--------:|----:|----:|----:|----:|------:|-------:|----:|
| 32 B    | 131 µs | **184 µs** | 205 µs | 259 µs | 476 µs | 1.46 ms | 1.46 ms |
| 64 B    | 135 µs | **178 µs** | 201 µs | 261 µs | 461 µs | 1.15 ms | 1.15 ms |
| 256 B   | 141 µs | **189 µs** | 213 µs | 269 µs | 432 µs | 1.12 ms | 1.12 ms |
| 1024 B  | 126 µs | **187 µs** | 210 µs | 258 µs | 304 µs | 413 µs  | 413 µs  |
| 4096 B  | 218 µs | **252 µs** | 283 µs | 368 µs | 540 µs | 947 µs  | 947 µs  |

p50 plateau bei ~178-189 µs für Payloads bis 1 kB; 4 kB hat die
zweite UDP-Fragmentation-Round trip overhead.

### Cyclone DDS — Roundtrip auf demselben Host (n=6 000-7 000 pro Run)

| Payload | min | p50 | p90 | p99 | max |
|--------:|----:|----:|----:|----:|----:|
| 32 B    | 45.3 µs | **71.1 µs** | 75.8 µs | 94.6 µs | 492 µs |
| 64 B    | 36.9 µs | **66.2 µs** | 71.8 µs | 90.9 µs | 419 µs |
| 256 B   | 45.4 µs | **67.6 µs** | 75.0 µs | 91.8 µs | 149 µs |
| 1024 B  | 38.6 µs | **66.8 µs** | 73.8 µs | 90.7 µs | 136 µs |
| 4096 B  | 45.5 µs | **73.2 µs** | 81.0 µs | 102.8 µs | 357 µs |

(Stabil mit Vorgabe-Messung von 2026-05-07 ohne Refit.)

## Side-by-Side

### p50

| Payload | ZeroDDS DCPS | Cyclone DCPS | ZeroDDS / Cyclone |
|--------:|-----------:|-----------:|------------------:|
| 32 B    | 184 µs | 71 µs | **2.6x** |
| 64 B    | 178 µs | 66 µs | **2.7x** |
| 256 B   | 189 µs | 68 µs | **2.8x** |
| 1024 B  | 187 µs | 67 µs | **2.8x** |
| 4096 B  | 252 µs | 73 µs | **3.4x** |

### p99

| Payload | ZeroDDS DCPS | Cyclone DCPS | ZeroDDS / Cyclone |
|--------:|-----------:|-----------:|------------------:|
| 32 B    | 259 µs | 95 µs | 2.7x |
| 64 B    | 261 µs | 91 µs | 2.9x |
| 256 B   | 269 µs | 92 µs | 2.9x |
| 1024 B  | 258 µs | 91 µs | 2.8x |
| 4096 B  | 368 µs | 103 µs | 3.6x |

### Vom UDP-Floor (Linux-Userspace) gerechnet

| Stack | DCPS-Overhead über ~41 µs Floor |
|---|---:|
| Cyclone DDS | **~26 µs** |
| ZeroDDS post-refit | **~137 µs** |
| ZeroDDS pre-refit | ~5 000 µs (5 ms) |

## Vorher-Nachher

| Metric | Pre-Refit (vor 871f79a) | Post-Refit | Speedup |
|---|---|---|---|
| ZeroDDS DCPS p50 (64 B) | **5-14 ms** | **178 µs** | **~50-80x** |
| ZeroDDS / Cyclone p50 (64 B) | ~75-200x | **2.7x** | — |

## Honest Read

**Was der Refit gebracht hat:**

* Single-Thread `event_loop` mit drei sequenziellen blocking-recv()s
  (50 ms tick_period-Timeouts) → vier dedizierte Threads (3 recv-loops
  + 1 tick-loop). Receive-Latenz ist jetzt OS-Wakeup statt
  Polling-Tick.
* Recv-Socket-Timeouts 50 ms → 1 s (nur noch für
  Stop-Flag-Check-Granularität, nicht für Tick-Rhythmus).
* p50-Roundtrip von 5-14 ms auf ~178 µs — **~50-80x Speedup**.
* Apples-to-apples (DCPS vs DCPS): von ~100x langsamer als Cyclone
  auf **~2.7x langsamer**. Wir sind in der Liga, aber nicht gleichauf.

**Was noch fehlt für Cyclone-Niveau:**

* **Phase 4 — Listener-Callback statt Polling**: das Bench-Tool
  ruft `zerodds_reader_take()` aktiv auf. Cyclone's `ddsperf` nutzt
  Listener-Callbacks die im Recv-Thread sofort feuern. Erwartet:
  ~50-100 µs raus, p50 in die 80-130 µs Region.
* **Lock-Free History-Cache**: `slot.lock()` ist heute `Mutex`, mit
  4 parallel laufenden Threads gibt's echte Contention. Refit zu
  RwLock oder lock-free Queue erwartet ~50-100 µs.
* **io_uring** (Phase 5+): die letzten ~10-20 µs auf Cyclone-
  Niveau (~67 µs p50 vanilla).

Realistisches Ziel: **80-120 µs p50 auf vanilla Linux**, das wäre
~1.5-1.8x Cyclone — akzeptabel für eine moderne Pure-Rust-
Implementierung mit voller Spec-Coverage.

**CI-Gate-Ziel "p99 < 5 µs"** bleibt physisch nur mit PREEMPT_RT +
isolcpus + chrt erreichbar (Linux-Floor ist ~41 µs vanilla).

## Reproduktion

```
ssh llvm@llvm 'cd ~/projects/zerodds-fresh && \
  cargo build --release -p zerodds-bench-suite --bin roundtrip-1us'
```

Sweep-Skript inline oben. Roh-Output siehe Konsolen-Logs vom
2026-05-07 ab ~01:50 CEST.
