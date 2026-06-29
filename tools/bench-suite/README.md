# ZeroDDS Bench-Suite

**Crate:** `zerodds-bench-suite` (tool, nicht runtime).
**Zweck:** Zitierfähige, reproduzierbare Perf-Messungen für alle
ZeroDDS-Transports über eine fixe Payload-Achse.

## Was hier ist

- `src/lib.rs` — Shared Harness: `PAYLOAD_SIZES`-Konstante (9 Punkte
  von 32 B bis 4 MiB), `make_payload()` deterministischer
  Payload-Gen, `size_label()` Labels für BenchmarkIds.
- `benches/transports_e2e.rs` — 4 Bench-Groups: `udp_send`,
  `uds_fs_send`, `uds_abstract_send` (Linux), `shm_send`. Jede
  misst `transport.send(locator, payload)` über die komplette
  Payload-Achse (mit DGRAM-Cap bei 64 KiB für UDP+UDS).
- `benches/rtps_fragmented.rs` — Writer-Dispatch mit Fragmentation
  ueber den ReliableWriter, multiple Reader-Proxies.
- `src/bin/roundtrip_1us.rs` — Sub-µs-Latency-Bench:
  Ping/Pong busy-poll auf UDP, Histogramm-Output mit p50/p90/
  p99/p999/p9999/p99999, optional `histlog`-V2-Format-Export
  (`--hgrm out.hgrm`).

## Wie man die Baseline erzeugt

Siehe `internal/perf/methodology.md` für Details. Kurzversion:

```bash
# Auf dem llvm-Host (bare-metal Debian 12, Threadripper 3955WX):
sudo ./benches/hosts/llvm/tune.sh on    # governor=performance,
                                         # net.core.*_max=32MiB, …

taskset -c 4-11 \
    cargo bench -p zerodds-bench-suite \
    --bench transports_e2e -- \
    --save-baseline v1.2-<date>

sudo ./benches/hosts/llvm/tune.sh off
```

Die erzeugten Reports liegen unter `target/criterion/` — raw JSON +
HTML-Plots. Baseline-Exports als Markdown unter
`internal/perf/baseline-<host>-<date>.md`.

## Design-Entscheidungen

Dokumentiert als Kommentare im Code. Kurzfassung:

- **Fix vorgegebene Payload-Achse**: 9 Punkte, keine ad-hoc-Größen.
  Reports zwischen Runs nur vergleichbar wenn identische Achse.
- **TX-only-Messung**: Receiver läuft im gleichen Prozess/Thread,
  drainiert stumm. Damit messen wir den reinen Sender-Pfad ohne
  OS-Scheduling-Noise vom Receiver. E2E-Ping/Pong-Latenz wird vom
  separaten `roundtrip-1us`-Binary erfasst.
- **DGRAM-Cap 60 KiB**: UDP + UDS-DGRAM sind per Kernel gecapt;
  Größen > 60 KiB werden in den DGRAM-Bench-Groups geskippt. SHM
  hat keinen Cap und läuft über alle 9 Punkte.
- **SHM-capacity 4×max_datagram**: verhindert Ring-Wrap-Spin-Loops
  im Bench. Bei realistischer Config ist capacity typisch 1 MiB;
  wir dimensionieren pro Größe, damit wir keine Wrap-Overheads in
  die Messung ziehen.
- **Consumer drain im SHM-Bench**: Criterion-iter-loop drainiert
  den Consumer nach jedem Send, damit der Ring nicht füllt. Das
  bedeutet, der Bench misst send+recv zusammen; der reine send-
  Pfad ohne Drain wäre durch den vollen Ring schnell gekappt.

## Roundtrip-1us — Latency-Bench

Sub-µs-Roundtrip-Bench fuer Latency-Messung auf RT-getuneten Hosts.
Laeuft als Zwei-Prozess-Setup:

```bash
# Pong-Prozess (CPU-pinned, RT-Prio empfohlen):
taskset -c 2 chrt -f 80 \
  roundtrip-1us --role pong --addr 0.0.0.0:7400 --payload 64

# Ping-Prozess (auf der Gegenmaschine):
taskset -c 3 chrt -f 80 \
  roundtrip-1us --role ping \
    --remote 192.0.2.10:7400 --bind 0.0.0.0:7401 \
    --payload 64 --warmup 5000 --samples 100000 \
    --hgrm out.hgrm
```

Output: stdout p50/p90/p99/p999/p9999/p99999 + min/max/n;
`--hgrm` schreibt das volle Histogramm im `histlog`-V2-Text-Format
(plotbar mit OSADL `cyclictest_plot`).

## Cross-Vendor-Vergleich

`tools/bench-suite` misst nur den ZeroDDS-Pfad. Cross-vendor-
Vergleiche gegen Cyclone DDS (`ddsperf`) + FastDDS
(`LatencyTestPublisher`) laufen ueber das Live-Interop-Harness in
`internal/interop/`.

## Regression-Gate

Die CI soll nach jedem Tag:

1. Die eingefrorene Baseline aus dem Repo laden.
2. `cargo bench -- --baseline <tag>` laufen.
3. Fail-on-regression: > 10 % Verschlechterung p50 auf irgendeinem
   (bench, size)-Punkt bricht den Build.

Aktuell manuell — Automatisierung ist Folge-Sprint-Material.
