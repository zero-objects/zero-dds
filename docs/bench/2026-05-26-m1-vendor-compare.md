# M1 Vendor-Compare — stable runner (2026-05-26)

Final M1-bench mit allen heutigen Refits (commit 14fc3062 → 8d49a701)
und race-fix (ebf7dab1) im zerodds-roundtrip wait_for_peers.

3 vendoren × 3 payloads × N=15 = 135 saubere runs. RTI separat
nachgezogen (`docs/bench/data/2026-05-26-m1-rti.csv`, 45 runs N=15)
mit `NDDSHOME`/`RTI_LICENSE_FILE` env-vars die im ersten stable-run
durch `env -i` weggepatcht wurden.

## Headline — alle 4 Vendoren

| Vendor | 0 B p50/CV | 4 KB p50/CV | 8 KB p50/CV | p99 @ 0B | p99/p50 |
|---|---|---|---|--:|--:|
| **cyclone** | **33.25 / 0.95%** | **34.79 / 1.58%** | **36.67 / 1.95%** | 96.08 | 2.89-2.99× |
| **zerodds** | 38.00 / 2.28% | 39.00 / 2.71% | 40.17 / 1.43% | **99.08** | **2.58-2.64×** |
| **rti** | 41.79 / 0.93% | 42.79 / 0.94% | 42.96 / 0.84% | 111.12 | 2.66-2.90× |
| fastdds | 6327 / 0.13% (broken) | 6330 / 1.38% | 6337 / 0.49% | 12687 | 2.00× |

**Positionierung auf M1:**
- **Cyclone gewinnt p50** — konstant 3.5-4.8 µs (12-14%) vor uns
- **ZeroDDS ist #2** — schlägt RTI bei allen Payloads (3.8-4.8 µs vor RTI)
- **ZeroDDS hat das beste p99/p50** — 2.58-2.64× vs Cyclone 2.89-2.99×, RTI 2.66-2.90×
- **RTI hat das beste CV** — 0.84-0.94% (sehr stabil, aber dadurch nicht schneller)

## M1 vs codepit Vergleich

| Vendor | 4 KB self M1 | 4 KB self codepit | M1/cp |
|---|--:|--:|--:|
| zerodds | 39.00 µs | 33.92 µs | 1.15× (M1 langsamer) |
| cyclone | 34.79 µs | 37.27 µs | **0.93× (M1 schneller)** |
| fastdds | 6330 | 44.30 | 143× (M1 setup broken) |

**Cyclone gewinnt auf M1, ZeroDDS gewinnt auf Linux.**

## Phase-Drilldown (separater Doku)

Siehe [2026-05-26-m1-phase-drilldown.md](2026-05-26-m1-phase-drilldown.md):

- Send-Floor 6.76 µs (macOS-kernel, beide Vendoren teilen es)
- Receive-Pfad ist der Cyclone-Gap-Hot-Spot:
  - `handle[reader+lock]`: 1.11 µs (vs Cyclone-Schätzung ~0.3)
  - `handle[dispatch (mpsc+waker)]`: 1.42 µs (vs Cyclone direct-callback ~0.4)
- Erwarteter Gesamt-Win bei parking_lot+crossbeam: ~2 µs/way → par mit Cyclone

## p99/p50-Diskussion

Wir haben **niedrigeres p99/p50-Verhältnis** als Cyclone bei allen Payloads:

| Payload | zerodds | cyclone |
|--:|--:|--:|
| 0 B | 2.61× | 2.89× |
| 4 KB | 2.64× | 2.91× |
| 8 KB | 2.58× | 2.99× |

Heißt: Tail-Spikes sind bei uns relativ niedriger. Absolute p99-Werte
sind allerdings höher als Cyclone (99-104 vs 96-110 µs) — weil unser
p50 höher ist. p999 ist mixed (zerodds besser bei 8 KB, Cyclone besser
bei 0 B).

## Cross-Vendor M1 (zerodds als ping/pong gegen die anderen)

In dieser N=15-Bench nicht gemessen (nur self-cells). Cross-Vendor-
Daten auf M1 kommen aus der 25.5-Connected-Matrix
([2026-05-25-m1-connected-roundtrip-matrix.md](2026-05-25-m1-connected-roundtrip-matrix.md)):
- zd↔cy @ 0B: 37.9 / 35.7 µs (symmetrisch)
- zd↔rt @ 0B: 46.2 / 34.3 µs (rt→zd schneller als rt-self!)

## RTI auf M1 — Issue gefixt

Der erste stable-runner-Lauf hatte `Abort trap: 6` für alle RTI-Cells.
**Root-Cause: nicht shmem-Issue sondern fehlende env-vars.**
Stable-runner war ohne `NDDSHOME`/`RTI_LICENSE_FILE` exec'd; RTI-Pong
wirft "RTI LICENSE ERROR" + abort.

Fix: env-vars im Script gesetzt, separater Lauf (`/tmp/m1-rti.csv`)
brachte saubere 45/45 ok mit konsistenten 41-43 µs Werten und sehr
niedrigem CV 0.84-0.94%.

**RTI ist auf M1 langsamer als auf Linux**:
- Linux codepit (heute): 24.49 µs @ 0B
- M1 (heute): 41.79 µs @ 0B
- M1/Linux: 1.71× langsamer

Auf Linux ist RTI Goldstandard, auf macOS Platz 3 hinter Cyclone+ZeroDDS.

## Bench-Setup

- Host: M1 Mac (8C/8GB, macOS 15.5)
- N=15 runs pro cell mit retry-on-timeout (max 2×) + 6s pong-settle
- 2000 samples + 200 warmup pro run
- QoS: RELIABLE, KEEP_LAST(64), XCDR1
- Commit-Stand: 8d49a701

## Daten

- `data/2026-05-26-m1-vendor-compare.csv` — 180 zeilen, 135 ok + 45 RTI-aborts

## Was als nächstes (Hebel-Ideen)

| Hebel | M1-Win Erwartung | Aufwand |
|---|---|---|
| parking_lot::Mutex | -0.3 µs/way | klein |
| crossbeam-channel | -0.7 µs/way | mittel (API) |
| collect_in_order_for prealloc | -0.1 µs/way (done) | - |
| kqueue-Waitset | -1.5 µs/way | hoch |

Kumuliert könnten alle vier den M1-Gap zu Cyclone vollständig schließen.
