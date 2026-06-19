# TSN-Hardware-Evaluierung

Stand 2026-06-10. Frage: Womit lässt sich der eigentliche TSN-Vorteil
(deterministische, eng begrenzte End-to-End-Latenz + Jitter) **messen**,
und was haben wir dafür?

## Warum Host-Benches nicht reichen

Der `tests/perf/tsn-latency`-Bench misst den Host-Socket-Pfad
(AF_PACKET vs. UDP). Die bounded-latency-Garantie von TSN entsteht aber
nicht im Host, sondern im **Netz**:

- **802.1Qbv Time-Aware-Shaper** (Linux: `taprio`-qdisc) — zeitgesteuerte
  Sende-Gates pro Traffic-Klasse.
- **ETF** (Earliest TxTime First, `SO_TXTIME` + `etf`-qdisc) — Frames zu
  exakten Zeitpunkten senden.
- **802.1AS / gPTP** — geteilte Netz-Zeitbasis (linuxptp `ptp4l`/`phc2sys`).
- Ein **TSN-fähiger Switch** (oder direkt verbundene TSN-NICs), der diese
  Gates entlang des Pfads durchsetzt.

Erst damit zeigt sich der Unterschied zu Best-Effort-Ethernet/UDP unter
Last (konkurrierender Cross-Traffic).

## Ist-Stand: codepit (LXC)

| Voraussetzung            | codepit | Befund                                            |
|--------------------------|---------|---------------------------------------------------|
| Physische TSN-NIC        | ✗       | `eth0` ist selbst ein veth (`@if53`), LXC-Bridge  |
| `sch_taprio`-Kernelmodul | ✗       | fehlt in `6.17.2-2-pve` (`modprobe: not found`)   |
| `sch_etf` / `SO_TXTIME`  | ?       | ungeprüft nutzbar (LXC schränkt qdisc oft ein)    |
| PHC / `/dev/ptp*`        | ✗       | kein PTP-Hardware-Clock                            |
| linuxptp (`ptp4l`)       | ✗       | nicht installiert                                 |
| Isolierte/RT-CPUs        | ✗       | 4 vCPU, keine Isolation                            |

**Fazit:** codepit kann den AF_PACKET-Pfad funktional + als Host-Baseline
messen, aber **keine** TSN-Scheduling-/bounded-latency-Eigenschaften.

## Was echte TSN-Messung braucht

Minimaler realistischer Aufbau (zwei Endpunkte + Direktverbindung,
ohne Switch reicht für 802.1Qbv/ETF zwischen zwei NICs):

1. **2× TSN-fähige NIC** mit Hardware-Launch-Time + PHC, z. B.
   - Intel **i210** (1 GbE, etabliert, taprio+ETF+LaunchTime),
   - Intel **i225/i226** (2.5 GbE, neuere Gen),
   - oder NXP/TI-Eval-Boards für Embedded-Targets.
2. **Bare-Metal-Linux** (kein LXC) mit Mainline-Kernel inkl.
   `sch_taprio` + `sch_etf`, NIC-`tc`-Offload.
3. **linuxptp** (`ptp4l` + `phc2sys`) für gPTP zwischen den Knoten.
4. Optional ein **TSN-Switch** für Mehr-Hop + Cross-Traffic-Tests.
5. RT-Tuning (`isolcpus`, `tuned` realtime) — siehe `tests/perf/rt-tuning/`.

### Messplan, sobald HW vorhanden

- `taprio`/`etf` auf der NIC konfigurieren (Gate-Schedule pro PCP).
- `tsn-latency`-Bench um Cross-Traffic (Best-Effort-Flut) erweitern und
  zeigen: TSN-Klasse hält p99/max-Latenz, Best-Effort nicht.
- gPTP-Offset/Stabilität mitloggen.

## Offene Beschaffungsfrage

Kein TSN-fähiges Gerät im aktuellen Lab-Inventar identifiziert. Bevor
Cross-Vendor-TSN-Interop oder bounded-latency-Benches sinnvoll sind,
ist eine i210/i225-NIC (+ Bare-Metal-Host wie `llvm`) der günstigste
Einstieg. Bis dahin: Host-Baseline (`tsn-latency`) + funktionaler
veth-E2E-Test decken den Software-Pfad ab.
