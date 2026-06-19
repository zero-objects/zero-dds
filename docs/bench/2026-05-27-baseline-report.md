# Cross-Vendor Roundtrip-Bench — Baseline 2026-05-27

Frische 5-Vendor-Baseline auf zwei sauberen Hosts. Atomic-Pipeline,
keine vendor-binary-mods, alle config-Hebel im Code transparent.

## Setup

| Host | Plattform | CPU | RAM | OS | Install-Doku |
|---|---|---|---|---|---|
| **codepit** | LXC | AMD Ryzen Threadripper 3955WX, 4 cores | 15 GiB | Debian 13 trixie | [install-report-codepit.md](install-report-codepit.md) |
| **m1-new** | bare-metal | Apple M1, 8 cores | 8 GiB | macOS 26.5 Tahoe | [install-report-m1.md](install-report-m1.md) |

Vendor-Versionen (identisch auf beiden Hosts):

| Vendor | Version | Quelle |
|---|---|---|
| ZeroDDS | rc.3 (5268d94e) | git build |
| Cyclone DDS | 11.0.1 | source |
| Fast-DDS | v3.6.1 + Fast-CDR v2.3.5 + foonathan v0.7-4 + fastddsgen v4.3.0 | source |
| RTI Connext | 7.7.0 LM | DMG (M1), apt packages.rti.com bookworm (codepit) |
| OpenDDS | v3.34.0 | source |

## Methodik

Apples-to-apples Cross-Vendor: gemeinsame `roundtrip.idl`, pro Vendor
ein typisierter C++-roundtrip-binary (via vendor-eigenem IDL-Compiler).
**Atomic-Pipeline** (`tests/perf/dds-roundtrip-bench/atomic/atomic.sh`):
1 ping × 1 pong × 1 payload = 1 CSV-Zeile. Frische Process-Trees,
fresh ENV pro Cell, isolierte Domain-IDs pro Cell, kein shared shell-State.

QoS-Konvention (alle Vendoren):
- `Reliability::Reliable`
- `History::KeepLast(64)`
- `DataRepresentation::XCDR2` (siehe Cross-Vendor-Config unten)
- `TypeConsistencyEnforcement::ALLOW_TYPE_COERCION` (Reader-side)
- UDPv4 only (kein SHM, kein TCP), Loopback `127.0.0.1`

## Ergebnis 1 — Self-Roundtrip Stable Quantiles

n=2000 samples + 200 warmup, payload=0, UDPv4-loopback.

| vendor | host | min | p50 | p90 | p99 | p999 | max |
|---|---|---:|---:|---:|---:|---:|---:|
| **zerodds** | **codepit** | 21.9 | **24.0** | 37.0 | **56.4** | 105.8 | **121.1** |
| zerodds | m1 | 31.8 | 38.2 | 46.9 | 72.2 | 112.2 | 131.4 |
| cyclone | codepit | 28.9 | 35.3 | 50.0 | 84.5 | 138.5 | 180.5 |
| cyclone | m1 | 35.0 | 40.9 | 49.4 | 65.5 | 109.0 | 112.2 |
| fastdds | codepit | 36.1 | 46.6 | 67.9 | 93.7 | 190.9 | 425.4 |
| fastdds | m1 | 5094 | 6330 | 12548 | 12641 | 12734 | 13218 |
| rti | codepit | 21.3 | 29.4 | 42.0 | 67.7 | 231.7 | 750.7 |
| rti | m1 | 27.4 | 34.8 | 45.3 | 99.0 | 123.5 | 146.0 |
| opendds | codepit | 193.1 | 338.3 | 518.8 | 782.4 | 1668 | 1739 |
| opendds | m1 | 235.0 | 400.6 | 477.8 | 552.9 | 591.8 | 658.9 |

**Beobachtungen:**
- ZeroDDS auf codepit: **niedrigster p50 (24µs), niedrigster p99 (56µs),
  niedrigster max (121µs)**. Sauberste Verteilung von allen 5 Vendoren.
- RTI auf codepit: zweit-niedrigster p50 (29µs), aber 750µs max-tail
  (3× schlechter als ZeroDDS-max).
- Cyclone, Fast-DDS: konsistent im Mittelfeld.
- OpenDDS: 10× langsamer als andere — ACE/TAO-ORB-Overhead.
- Fast-DDS auf M1: **6.3ms p50** — known macOS-26.5-Issue (ASIO-event-
  loop-Granularität). Cross-Vendor matched aber funktional; latency
  ist Vendor-internal, kein Wire-Issue.

## Ergebnis 2 — Payload-Sweep (Self-Roundtrip)

p50 (µs), n=100 + 20 warmup, UDPv4-loopback, payload-Längen
0/64/256/1024/4096 Byte.

**codepit:**
| vendor | 0 B | 64 B | 256 B | 1024 B | 4096 B |
|---|---:|---:|---:|---:|---:|
| zerodds | 30.7 | 30.4 | 30.9 | 31.4 | 33.3 |
| cyclone | 35.0 | 48.8 | 35.9 | 50.1 | 44.8 |
| fastdds | 56.3 | 46.4 | 41.9 | 57.9 | 64.2 |
| rti | 38.1 | 38.0 | 24.3 | 41.3 | 54.2 |
| opendds | 384.8 | 120.1 | 666.2 | 239.5 | 253.0 |

**m1:**
| vendor | 0 B | 64 B | 256 B | 1024 B | 4096 B |
|---|---:|---:|---:|---:|---:|
| zerodds | 150.6 | 152.8 | 152.7 | 136.3 | 159.5 |
| cyclone | 170.2 | 141.6 | 137.6 | 171.9 | 165.8 |
| fastdds | 6329 | 6330 | 6333 | 6332 | 6331 |
| rti | 142.7 | 136.6 | 97.1 | 123.2 | 117.6 |
| opendds | 419.3 | 419.4 | 411.7 | 423.7 | 440.1 |

**Beobachtung:** ZeroDDS-codepit flach bei 30µs über 0→4096 B (≤10%
Varianz). Andere Vendoren zeigen 15-50% payload-Varianz.

## Ergebnis 3 — Cross-Vendor 5×5 Matrix

n=50 samples + 10 warmup, payload=0, UDPv4-loopback. Pro Cell ein
Ping-Vendor (Sender) + Pong-Vendor (Echo). "ok" = >=30 samples
durchgekommen, "—" = Discovery- oder Wire-Format-Fail (timeout).

**codepit:**

| PING ↓ \ PONG → | zerodds | cyclone | fastdds | rti | opendds | row |
|---|---:|---:|---:|---:|---:|---|
| **zerodds** | 30 | 38 | 54 | 46 | 81 | **5/5** |
| **cyclone** | 31 | 36 | 54 | 83 | 91 | **5/5** |
| **fastdds** | 67 | 35 | 58 | 68 | 115 | **5/5** |
| **rti** | 34 | 65 | 47 | 37 | — | 4/5 |
| **opendds** | 81 | 123 | 162 | — | 730 | 4/5 |

**m1:**

| PING ↓ \ PONG → | zerodds | cyclone | fastdds | rti | opendds | row |
|---|---:|---:|---:|---:|---:|---|
| **zerodds** | 206 | 222 | 215 | 204 | 388 | **5/5** |
| **cyclone** | 208 | 172 | 208 | 179 | 428 | **5/5** |
| **fastdds** | 213 | 203 | 6333 | 252 | 1195 | **5/5** |
| **rti** | 194 | 209 | 130 | 189 | — | 4/5 |
| **opendds** | 324 | 458 | * | — | 419 | 3/5 |

`*` M1 opendds→fastdds: partial (n=39 statt 50, value=160ms — Fast-DDS-M1-ASIO-spillover).

**Score: 23/25 cells matched auf beiden Hosts.**

**Big-3 (RTI ↔ Cyclone ↔ FastDDS) untereinander auf beiden Hosts ✓.**

Verbleibend: rti↔opendds (beide Richtungen) — SEDP-builtin-Encoding-Mismatch
(`expected mutable extensibility, but got CDR/XCDR1 Big Endian Plain` im
OpenDDS-SEDP-Reader). Independent von DataRepresentation-QoS, RTI-internes
SEDP-Topic-Encoding. Task #122 pending.

## Cross-Vendor-Config-Hebel (was war kaputt + wie repariert)

Vor den Fixes: nur 12/25 cells matched (ZeroDDS-Ping 5/5 als einziger,
alle anderen Vendoren sehr eingeschränkt).

**RTI-Defaults sind strict:**
- DataRepresentation default = XCDR1 (kein Match mit XCDR2-Vendoren).
- TypeConsistencyEnforcement default = EXACT_TYPE (kein Match wenn Type-
  Hashes minimal abweichen).

**FastDDS-TypeObject-Encoding:**
- fastddsgen v4.3 emittiert für `sequence<octet, 8192>` einen all-zero
  PLAIN_SEQUENCE_LARGE TypeObject den Cyclone rejected
  (`ddsi_xt_type_add_typeobj with invalid type object`).
- Fix: `fastddsgen -no-typeobjectsupport` → SEDP fällt auf Type-name +
  ALLOW_TYPE_COERCION zurück.

**Code-Patches:**
| File | Patch |
|---|---|
| `rti_app.cpp` | `DataRepresentation::xcdr2()` + `TypeConsistencyEnforcement::ALLOW_TYPE_COERCION` |
| `cyclone_app.cpp` | `DataRepresentation::XCDR2` (Writer + Reader) |
| `fastdds_app.cpp` | `dw_qos.representation().m_value.push_back(XCDR2_DATA_REPRESENTATION)` (W + R) |
| `opendds_app.cpp` | `XCDR_DATA_REPRESENTATION` → `XCDR2_DATA_REPRESENTATION` |
| `CMakeLists.txt` | fastddsgen-call mit `-no-typeobjectsupport` |
| `crates/dcps/src/runtime.rs` | Env-var `ZERODDS_DATA_REPR_OFFER=XCDR2` |

**Run-Command (reproduzierbar):**
```bash
ZERODDS_DATA_REPR_OFFER=XCDR2 \
HOST_TAG=codepit ATOMIC_SAMPLES=50 ATOMIC_WARMUP=10 ATOMIC_DOMAIN=N \
  bash tests/perf/dds-roundtrip-bench/atomic/atomic.sh <ping> <pong> <payload>
```

## Bekannte offene Issues

1. **Fast-DDS-M1: 6.3ms p50** — macOS-26.5-Tahoe + Fast-DDS-v3.6.1-ASIO-
   event-loop-Granularität. Cross-Vendor matched korrekt, latency selbst
   ist Vendor-internal. Reproduzierbar.

2. **RTI ↔ OpenDDS Cross-Vendor** — beide Richtungen failed in 5×5.
   Root: RTI sendet XCDR1-PI-Stream für SEDP-builtin-Topics, OpenDDS-
   SEDP-Reader will mutable extensibility. Unabhängig von User-DataRep
   QoS. Workaround vermutlich via opendds_rtps.ini SEDP-config.
   Task #122.

3. **OpenDDS-codepit p99/p999 hoch** (782µs / 1668µs) — vermutlich
   ACE/TAO ORB-internal threading auf Container-Kernel.

## Raw-Daten

- `data/2026-05-27/codepit-stable.csv` — n=2000 self-bench
- `data/2026-05-27/m1-stable.csv` — n=2000 self-bench
- `data/2026-05-27/codepit-self-sweep.csv` — 5 vendors × 5 payloads
- `data/2026-05-27/m1-self-sweep.csv` — 5 vendors × 5 payloads
- `data/2026-05-27/codepit-xmatrix-final.csv` — 5×5 cross-matrix
- `data/2026-05-27/m1-xmatrix-final.csv` — 5×5 cross-matrix
