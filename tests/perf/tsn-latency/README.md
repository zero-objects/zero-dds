# TSN-Roundtrip-Latenz-Bench

Misst die **Host-Transport-Pfad-Latenz** (Request → Echo → Request) des
live-AF_PACKET-Transports (DDS-TSN 1.0 Annex A, EtherType `0x88B5`,
`crates/transport-tsn/src/socket.rs`) gegen eine UDP-Baseline über
**dasselbe** veth-Paar im Root-Namespace.

## Was das ist — und was nicht

Das ist **keine** TSN-bounded-latency-Messung. Der eigentliche TSN-
Vorteil (deterministische, eng begrenzte Latenz + Jitter) entsteht im
**Netz**: ein TSN-Switch mit Time-Aware-Shaper (802.1Qbv / taprio) bzw.
ETF-Scheduling, plus gPTP-Zeitsynchronisation. Das misst man nur mit
echter TSN-Hardware.

Dieser Bench misst den **Host-Socket-Pfad** und dient als:

- **Baseline + Regressions-Guard** für den AF_PACKET-Transport,
- **ehrlicher Vergleich** AF_PACKET vs. UDP auf demselben Link.

## Ausführen

```bash
sudo tests/perf/tsn-latency/run.sh [COUNT]   # Default COUNT=20000
```

Braucht root (`CAP_NET_RAW` + `CAP_NET_ADMIN`), Linux. Legt ein veth-Paar
mit IPs an, fährt Echo-Server + Latenz-Client je einmal über AF_PACKET
und über UDP, gibt je ein JSON-Objekt aus (p50/p90/p99/max/Jitter in ns).

Einzelmodi (Beispiel-Binary direkt):

```text
tsn_latency tsn-pong  <iface>
tsn_latency tsn-ping  <iface> <peer-mac> <count>
tsn_latency udp-pong  <bind-addr>
tsn_latency udp-ping  <bind-addr> <peer-addr> <count>
```

## Baseline (codepit, LXC, veth, 6.17.2-pve, 4 vCPU, 2026-06-10)

5000 Samples, 200 Warmup, 64-Byte-Payload, 0 % Verlust:

| Transport            | p50      | p90      | p99      | max     | Jitter (p99−p50) |
|----------------------|----------|----------|----------|---------|------------------|
| TSN / AF_PACKET 0x88B5 | 11.95 µs | 22.49 µs | 29.58 µs | 281 µs  | 17.62 µs         |
| UDP (gleicher veth)  | 17.10 µs | 21.07 µs | 34.90 µs | 928 µs  | 17.79 µs         |

Beobachtung: Der Raw-Ethernet-Pfad liegt beim p50 **unter** UDP, weil er
den IP/UDP-Stack umgeht. Das ist erwartbar und bestätigt, dass der
AF_PACKET-Transport keinen Overhead-Nachteil gegenüber UDP hat. Die
hohen `max`-Werte stammen vom Scheduling-Jitter eines nicht-isolierten
LXC-Containers ohne RT-Tuning — auf isolierten/RT-getunten Cores
(`rt-tuning/`) deutlich enger.

**Kein TSN-Switch im Pfad** → keine Aussage über deterministische
End-to-End-Latenz. Dafür siehe `internal/tsn/hardware-eval.md`.
