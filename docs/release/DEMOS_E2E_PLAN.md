# 51/51 Wire-E2E Demo Plan

> **Ziel:** Alle 51 Demo-Items in `examples/` werden Wire-E2E demoable
> mit Linux+Mac+Win-Packaging, konfigurierbaren Bridges, Manuals und
> Deployment-Anleitungen.

**Definition Wire-E2E-Demoable:**
1. Launch-Script existiert (Docker-Compose, justfile, shell-script).
2. Mindestens 2 separate Prozesse kommunizieren über Wire (UDP/TCP/Shm).
3. Mindestens einer ist ZeroDDS-DCPS-Process.
4. Bridge-Demos: anderes Ende ist echter externer Broker/Server.
5. Verifikations-Schritt sichtbar (subscriber-output, log-grep, assert).

**Audit-Ausgangsbasis:** 1/51 ready (`00-base/03-rust-tui` versteckt).

---

## Phase 0 — Spec-Foundation

Pro Bridge/FFI/Deployment-Pattern eine Vendor-Spec im Stil von
`docs/specs/zerodds-xcdr2-bindings-conformance-1.0.md`:

| # | Spec | Scope |
|---|------|-------|
| S1 | `zerodds-ws-bridge-1.0` | WebSocket-Daemon: Topic-Mangling, Frame-Format, Config-Schema |
| S2 | `zerodds-mqtt-bridge-1.0` | MQTT-5-Daemon: Topic-Map, QoS-Translation, Retain-Semantik |
| S3 | `zerodds-coap-bridge-1.0` | CoAP-Daemon: URI-Mapping, Observe-Mode |
| S4 | `zerodds-amqp-bridge-daemon-1.0` | AMQP-Daemon (extends dds-amqp-1.0 PIM) |
| S5 | `zerodds-grpc-bridge-1.0` | gRPC-Daemon: Service-Map, Path-Translation |
| S6 | `zerodds-corba-bridge-1.0` | CORBA-Daemon: Repository-ID-Map, IIOP-Connect |
| S7 | `zerodds-ros2-bridge-1.0` | ROS-2-Bridge (RTPS-direkt via rmw + topic-mangling) |
| S8 | `zerodds-ffi-loader-1.0` | Cross-Lang ABI: Pub/Sub-Surface für FFI-Loader |
| S9 | `zerodds-deployment-1.0` | Linux/Mac/Win Packaging-Konventionen |

## Phase 1 — Bridge-Daemons + FFI-Foundation

Pro Bridge: Daemon-Binary in `crates/<bridge>-bridge/src/bin/zerodds-<x>-bridged.rs`.

| # | Item | Aufwand |
|---|------|---------|
| D1 | `zerodds-ws-bridged` (TCP-Listen + DDS-Pump) | 2-3 PT |
| D2 | `zerodds-mqtt-bridged` (Mosquitto-Connect + DDS-Pump) | 1-2 PT |
| D3 | `zerodds-coap-bridged` (UDP-Listen + DDS-Pump) | 1-2 PT |
| D4 | `zerodds-amqp-bridged` (RabbitMQ-Connect + DDS-Pump) | 1-2 PT |
| D5 | `zerodds-grpc-bridged` (HTTP/2-Server + DDS-Pump) | 1-2 PT |
| D6 | `zerodds-corba-bridged` (IIOP-Server + DDS-Pump) | 2-3 PT |
| D7 | `zerodds-ros2-bridged` (RTPS direkt — rmw shim aktivieren) | 1-2 PT |

FFI-Loader-Patterns:

| # | Item | Aufwand |
|---|------|---------|
| F1 | Python ctypes (sample first) | 2-3 PT |
| F2 | Java JNI | 1-2 PT |
| F3 | C# P/Invoke | 1-2 PT |
| F4 | C++ header (libzerodds.h) | 1-2 PT |
| F5 | TS N-API + Browser WASM | 1-2 PT |
| F6 | Flutter dart:ffi | 1-2 PT |

## Phase 2 — Hero-Promotion

| # | Item |
|---|------|
| H1 | `00-base/03-rust-tui` als Hero-Demo: README, justfile, tmux-launcher, examples/README-Promotion |

## Phase 3 — dds-chat Chapters Pub/Sub-Splits

Per Chapter `chXX_*.rs` aufteilen in `chXX_pub.rs` + `chXX_sub.rs` + Launcher-Script.

| # | Items |
|---|------|
| C1 | ch01 (Template) |
| C2-C9 | ch02-ch09 |
| C10-C15 | ch10-ch15 (LIB-ONLY-Items: kreative Wire-Variante schreiben) |

## Phase 4 — dds-chat integrations Wire-up

Pro Bridge: Demo-Binary nutzt Daemon (D1-D7) + Docker-Compose-Service.

| # | Item |
|---|------|
| I1 | integrations/websocket → ws-bridged + Browser-Client |
| I2 | integrations/mqtt → mqtt-bridged + Mosquitto |
| I3 | integrations/amqp → amqp-bridged + RabbitMQ |
| I4 | integrations/coap → coap-bridged + libcoap-server |
| I5 | integrations/grpc → grpc-bridged + gRPC-Client |
| I6 | integrations/corba → corba-bridged + omniORB |
| I7 | integrations/ros2 → ros2-bridged + ROS-2 Talker |

## Phase 5 — dds-chat ports/* Live-Pub/Sub

Per Sprach-Port: Live-Pub/Sub via FFI-Loader + Cross-Process-Test.

| # | Items |
|---|------|
| P1-P10 | python-cli/-gui, java-cli/-backend, csharp-cli/-wpf, cpp-tui/-qt6, ts-node/-browser |

## Phase 6 — dds-chat apps Live

| # | Item |
|---|------|
| A1 | embedded-mcu (UART-/Ethernet-Wire-Stack) |
| A2 | flutter-mobile via ws-bridged |
| A3 | qt-desktop via dds-cpp |
| A4 | web-spa via ws-bridged |

## Phase 7 — dds-warehouse Stations Multi-Process

| # | Items |
|---|------|
| W1-W10 | 10 Stations mit Server+Client/Agent |
| WO | Orchestrator als Multi-Process-Launcher |

## Phase 8 — DM.2 perf-camera-dds Full Impl

| # | Item |
|---|------|
| PC1 | Flutter publisher |
| PC2 | Qt6 subscriber |
| PC3 | bridge-config.yaml |

## Phase 9 — DM.3 otel Real Demo

| # | Item |
|---|------|
| OT1 | talker.rs in crates/observability-otlp/examples/ |

## Phase 10 — Packaging Linux/Mac/Win

| # | Item |
|---|------|
| PK1 | cargo-dist setup |
| PK2 | Linux .deb / .rpm |
| PK3 | Mac homebrew formula |
| PK4 | Win MSI / scoop |
| PK5 | Docker-Images für alle Daemons |

## Phase 11 — Manuals + READMEs

| # | Item |
|---|------|
| M1 | Per Daemon: man-page (`man/zerodds-*-bridged.1`) |
| M2 | Per Demo: README mit Linux/Mac/Win Deployment-Sections |
| M3 | Top-Level: `examples/DEMOS-INDEX.md` mit Status-Matrix |

---

## Reihenfolge (Dependency-Aware)

```
Phase 0 (Specs S1-S9)
    ↓
Phase 1 (Daemons D1-D7 + FFI F1-F6)
    ↓
Phase 2 (Hero H1)  ← parallel-safe ab Phase 0
    ↓
Phase 3 (Chapters C1-C15)  ← parallel zu Phase 2
    ↓
Phase 4 (Integrations I1-I7)  ← braucht D1-D7
    ↓
Phase 5 (Ports P1-P10)  ← braucht F1-F6
    ↓
Phase 6 (Apps A1-A4)  ← braucht D1 + F6
    ↓
Phase 7 (Stations W1-W10)  ← parallel ab Phase 0
    ↓
Phase 8 (perf-camera PC1-PC3)  ← braucht D1
    ↓
Phase 9 (otel OT1)  ← parallel ab Phase 0
    ↓
Phase 10 (Packaging PK1-PK5)  ← braucht alle Daemons + Demos
    ↓
Phase 11 (Manuals M1-M3)  ← braucht alle vorherigen
```

## Aufwands-Schätzung

- Phase 0: ~5 PT (9 Specs)
- Phase 1: ~20 PT (Daemons + FFI)
- Phase 2: ~0.5 PT
- Phase 3: ~3-5 PT (Pub/Sub-Splits)
- Phase 4: ~10 PT (Compose + Connectivity)
- Phase 5: ~10-15 PT (10 FFI-Replikationen)
- Phase 6: ~5 PT (4 Apps)
- Phase 7: ~5-10 PT (10 Stations)
- Phase 8: ~10-15 PT (perf-camera)
- Phase 9: ~0.5 PT
- Phase 10: ~5 PT (Packaging)
- Phase 11: ~5 PT (Manuals)

**Total: ~80-100 PT autonomer Arbeit.** Mit Parallel-Sub-Agents
realistisch in 1-2 intensive Tagen umsetzbar.

## Commit-Strategie

Pro Phase ein oder mehrere logische Commits. Spec-Commits separat von
Impl-Commits. Nach jeder Phase Verifikation (Build+Test) + Tracker-
Update.
