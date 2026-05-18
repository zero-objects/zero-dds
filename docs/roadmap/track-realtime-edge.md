# Track Post-D — Real-Time-Edge (Detail)

**Status:** 📋 backlog (post-1.0)

**Trigger:** Real-Time-Lab-Setup auf nr3 oder externem HW-Partner
(STM32-Discovery, RP2040, ESP32, NXP-S32K) mit Latency-Validation-Plan.

## Items

1. **PREEMPT_RT-Linux-Latency-Beweis** (2 PW) — Cyclictest-Reports, p99
   roundtrip-Latenz unter Last, Plotting-Pipeline, dokumentiert in
   `docs/perf/preempt-rt-baseline.md`.
2. **STM32F4-Discovery Bare-Metal-Demo** (1 PW) — XRCE-Client als
   bare-metal-binary, Sensor-Data über UART, embedded-Showcase.
3. **RP2040 / ESP32 / S32K-Demo-Boards** (je 1 PW) — gleicher Pattern,
   für die häufigsten Microcontroller-Familien.
4. **TSN-LAN-Live-Test** (2-3 PW) — IEEE-802.1-AS-2020-Compliance auf
   real HW (z.B. iEi-Industriemainboard mit i210/i225 NIC), 802.1Qbv-
   Time-Aware-Shaper-Compat.
5. **AUTOSAR Classic eigentlich** (4-6 PW) — separat von Vertical-
   Compliance: die Bridge zu CAN-RTE selbst implementieren.
6. **DDS-XRCE-Mesh** (3 PW) — multiple XRCE-Clients über LoRaWAN/BLE/
   Zigbee-Gateways. Demo-Network mit 50+ constrained Nodes.

## Acceptance

- PREEMPT_RT-Bench-Result published als PDF unter zerodds.org/news/
- Bare-Metal-Demo-Boards laufen auf je einem realen Board, video-
  dokumentiert
- TSN-LAN-Test-Report
- AUTOSAR-Classic Bridge mit testfähigem Demo

## Out-of-Scope

- Eigene Hardware-Designs
- Eigene OS-Forks (kein eigener PREEMPT_RT-Patch-Set)
- Custom-Silicon (nicht unser Geschäft)
