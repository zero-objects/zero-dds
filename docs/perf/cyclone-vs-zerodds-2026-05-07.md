# Cyclone DDS vs ZeroDDS — Roundtrip-Latenz, llvm vanilla

**Datum:** 2026-05-07
**Bench-Host:** `llvm.sandra-kessler.eu` (AMD Threadripper / kleinere Variante,
Linux 6.1.0-44-amd64 vanilla, Debian 12 bookworm, **kein** PREEMPT_RT,
**kein** CPU-Pinning, **kein** `chrt -f`).
**Loopback:** `127.0.0.1` (UDP) — gleicher Pfad bei beiden Stacks.
**Cyclone-Version:** 0.10.2-2 (Debian-Paket, `ddsperf`).
**ZeroDDS-Tool:** `tools/bench-suite/src/bin/roundtrip_1us.rs`
(standalone-Build mit DCPS-Pfad herausgepatched, hdrhistogram 7.5).

> **Wichtig — keine apples-to-apples-Messung.** ZeroDDS misst hier den
> **rohen UDP-Transport-Roundtrip** (`std::net::UdpSocket` busy-poll, kein
> RTPS, kein DCPS, kein Discovery). Cyclone DDS misst **vollen DCPS-Stack**
> (Discovery + RTPS + Reader/Writer-Matching + Sample-Dispatch + Listener-
> Trigger). Der ZeroDDS-DCPS-Pfad (`--use-dcps`) liegt aktuell bei ca.
> 5–14 ms p50 unter CFS-Scheduler-Drift (Sprint D.5b laufend) und ist
> damit **schlechter als Cyclone**, nicht besser. Diese Messung zeigt:
>
> * Den **Linux-Userspace-UDP-Floor** auf diesem Host (~40 µs p50)
> * Das **DCPS-Overhead von Cyclone** über diesem Floor (~25 µs)
> * Das **Ziel für ZeroDDS-DCPS-Tuning** (Sprint D.5d+)

## Methodik

### ZeroDDS raw UDP

```
roundtrip-1us --role pong --bind 127.0.0.1:17400 --payload <SZ> &
roundtrip-1us --role ping --remote 127.0.0.1:17400 \
              --bind 127.0.0.1:17401 --payload <SZ> \
              --warmup 3000 --samples 50000
```

Free-running busy-poll, kein Rate-Limit. 3 000 Warmup-Samples verworfen,
50 000 Mess-Samples in HdrHistogram (Sub-µs-Resolution).

### Cyclone DDS

```
ddsperf -L -D 10 ping size <SZ> pong         # reliable
ddsperf -L -u -D 10 ping size <SZ> pong      # best-effort
```

Same-Process (`-L`), 10 s Mess-Dauer, Default-Topic `KS` mit
zusätzlichem `size <SZ>` Payload.

## Roh-Daten

### ZeroDDS raw UDP — Roundtrip (n=50 000)

| Payload | min | p50 | p90 | p99 | p99.9 | p99.99 | max |
|--------:|----:|----:|----:|----:|------:|-------:|----:|
| 32 B    | —   | **40.8 µs** | — | 82.2 µs | 101.7 µs | — | — |
| 64 B    | 38.8 µs | **41.1 µs** | 44.4 µs | 86.1 µs | 140.0 µs | 209.0 µs | 1.08 ms |
| 256 B   | —   | **40.8 µs** | — | 82.4 µs | 102.5 µs | — | — |
| 1024 B  | —   | **41.4 µs** | — | 82.4 µs | 105.1 µs | — | — |
| 4096 B  | —   | **43.3 µs** | — | 86.3 µs | 132.2 µs | — | — |

Plateau bei ~40-43 µs p50 unabhängig von Payload — der Roundtrip wird
vom Linux-Scheduler/Network-Stack dominiert, nicht von Userspace-Copy.

### Cyclone DDS — Roundtrip RELIABLE (DDS-Default)

| Payload | min | p50 | p90 | p99 | max | n |
|--------:|----:|----:|----:|----:|----:|--:|
| 12 B (KS)  | 30.6 µs | 62.6 µs | 70.9 µs | 86.2 µs | 468 µs | 7 036 |
| 32 B       | 39.3 µs | **63.3 µs** | 72.4 µs | 99.7 µs | 500 µs | 6 975 |
| 64 B       | 39.0 µs | **66.5 µs** | 74.6 µs | 93.0 µs | 335 µs | 6 774 |
| 256 B      | 31.6 µs | **71.1 µs** | 85.6 µs | 118.7 µs | 538 µs | 6 233 |
| 1024 B     | 41.9 µs | **71.9 µs** | 90.2 µs | 136.1 µs | 451 µs | 6 110 |
| 4096 B     | 39.4 µs | **68.2 µs** | 78.1 µs | 114.3 µs | 394 µs | 6 576 |

### Cyclone DDS — Roundtrip BEST-EFFORT (`-u`)

| Payload | min | p50 | p99 | max | n |
|--------:|----:|----:|----:|----:|--:|
| 32 B    | 39.5 µs | 66.3 µs | 105.3 µs | 201 µs | 6 742 |
| 64 B    | 39.9 µs | 67.2 µs | 115.5 µs | 831 µs | 6 611 |
| 256 B   | 46.1 µs | 71.0 µs | 103.1 µs | 514 µs | 6 361 |
| 1024 B  | 40.1 µs | 67.5 µs | 107.1 µs | 450 µs | 6 545 |

Best-Effort ist marginal langsamer als Reliable — vermutlich weil
Reliable Sample-Batching pipeline-effizienter macht.

## Side-by-Side (p50, gleiche Payload-Größe)

| Payload | ZeroDDS raw UDP | Cyclone reliable | Cyclone best-effort | Δ Cyclone-vs-ZeroDDS-floor |
|--------:|-----:|-----:|-----:|-----:|
| 32 B    | 40.8 µs | 63.3 µs | 66.3 µs | **+22.5 µs DCPS-Overhead** |
| 64 B    | 41.1 µs | 66.5 µs | 67.2 µs | **+25.4 µs** |
| 256 B   | 40.8 µs | 71.1 µs | 71.0 µs | **+30.3 µs** |
| 1024 B  | 41.4 µs | 71.9 µs | 67.5 µs | **+30.5 µs** |
| 4096 B  | 43.3 µs | 68.2 µs | — | **+24.9 µs** |

## Honest Read

1. **Linux-Userspace-UDP-Floor** auf diesem Host (vanilla, kein RT-Tuning):
   ~40 µs p50. Das ist die *Untergrenze* — alles darüber ist Stack-
   Overhead.

2. **Cyclone DDS' DCPS-Overhead** über dem Floor: ~22-30 µs, größtenteils
   stabil. Cyclone schafft p99 unter 100 µs für kleine Payloads — das
   ist eine reife Implementation.

3. **ZeroDDS-Implementierung steht vor zwei Aufgaben:**
   - Aktueller `--use-dcps`-Pfad: 5-14 ms p50 (CFS-Drift, Sprint D.5b).
     Muss in Cyclones Liga (~70 µs p50 vanilla) gebracht werden.
   - CI-Gate-Profil (p99 < 5 µs) ist auf vanilla Linux **nicht
     erreichbar** — braucht zwingend PREEMPT_RT + isolcpus + chrt.

4. **Was diese Messung NICHT zeigt:**
   - **FastDDS** ist nicht enthalten — `fastdds-tools`-apt-Paket hat
     kein Latency-Bench-Tool, separater Build von `LatencyTestPublisher`
     aus eProsima/FastDDS-statistics-backend nötig (Sprint D.5e).
   - **DCPS-vs-DCPS** apples-to-apples: ZeroDDS-DCPS ist aktuell
     hinter Cyclone, nicht vor. Sprint D.5d wird das auf gleiche
     Methodik bringen.
   - **Throughput** (MB/s) wurde nicht gemessen, nur Latenz.

## Reproduktion

Auf llvm:

```bash
# Cyclone (Debian-Paket vorhanden)
ddsperf -L -D 10 ping size 64 pong         # reliable
ddsperf -L -u -D 10 ping size 64 pong      # best-effort

# ZeroDDS (standalone, ~/bench-rt1us/)
./target/release/roundtrip-1us --role pong --bind 127.0.0.1:17400 --payload 64 &
./target/release/roundtrip-1us --role ping --remote 127.0.0.1:17400 \
    --bind 127.0.0.1:17401 --payload 64 --warmup 3000 --samples 50000
```

Standalone-Build des ZeroDDS-Tools (DCPS-Pfad herausgepatched):
`/tmp/rt1us-standalone/` lokal generiert via `awk` aus
`tools/bench-suite/src/bin/roundtrip_1us.rs`, `Cargo.toml` mit nur
`hdrhistogram = "7.5"` als Dependency.

## Nächste Sprints

* **D.5b** (laufend): ZeroDDS-DCPS-Pfad debuggen — CFS-Drift +
  HistoryCache-Lock-Profile, Ziel: p50 < 100 µs vanilla
* **D.5d** (next): Cyclone-Side-by-Side mit RT-Tuning auf llvm —
  Linux 6.x PREEMPT_RT + isolcpus=2-7 + chrt -f 80, p99 < 5 µs
  Erwartung
* **D.5e**: FastDDS LatencyTest-Build + Side-by-Side
* **D.5f**: Pure-Rust-Konkurrenten (dust-dds, RustDDS) Side-by-Side
