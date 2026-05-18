# Post-1.0 Backlog

**Goal:** Sammelpunkt für alles was sinnvoll ist aber nicht in das
1.0-Release passt. Wird nach 1.0-final pro Quartal in Mini-Phasen
priorisiert (1.1, 1.2, 1.3, ...).

**Status:** 📋 backlog

## Tracks

| # | Track | Detail-Doku | Estimate | Trigger |
|---|---|---|---|---|
| Post-A | Cloud-Native | [`track-cloud-native.md`](track-cloud-native.md) | 4-6 PW | Industry-Demand bekannt |
| Post-B | Security-Compliance | [`track-security-compliance.md`](track-security-compliance.md) | 6-8 PW | CRA-Deadline 2027 |
| Post-C | Industry-Verticals | [`track-industry-verticals.md`](track-industry-verticals.md) | 12+ PW | per-Vertical erste Pilot-Kunden |
| Post-D | Real-Time-Edge | [`track-realtime-edge.md`](track-realtime-edge.md) | 6-10 PW | wenn Mikro-Profile-Audit (RC2-E) Mängel zeigt |

## Track Post-A — Cloud-Native (kurz)

**Items:**
- Kubernetes Operator: CRDs für Participant, Topic, Bridge-Daemon-Set
- Helm-Chart für die 7 Bridge-Daemons + Persistence-Service
- Istio/Linkerd-Sidecar-Annotations
- OpenTelemetry-Grafana-Dashboards published
- Knative-Eventing-Source / -Sink für DDS-Topics

**Trigger:** wenn 1.0-Adoption Cloud-Use-Cases bringt (Telemetry-
Aggregation in K8s-Clustern).

**Estimate:** 4-6 PW.

## Track Post-B — Security-Compliance (kurz)

**Items:**
- CRA-Compliance-Dokumentation (EU Cyber Resilience Act ab 2027)
- SLSA-3-Build-Provenance (Sigstore-signed builds)
- Sigstore-Cosign zusätzlich zu minisign
- FIPS-140-3-Mode für rustls (FIPS-Crypto-Subset opt-in)
- Threat-Model in `docs/security/threat-model.md` (STRIDE)
- Pen-Test (extern beauftragt)
- Authenticode-Cert für Windows (wenn Cost akzeptabel ist)

**Trigger:** CRA-Deadline (Dezember 2027 für Open-Source-Komponenten),
Industry-Pilot-Kunden mit Compliance-Requirements.

**Estimate:** 6-8 PW.

## Track Post-C — Industry-Verticals (kurz)

**Items:**
- **Automotive — AUTOSAR Classic Platform**: Bridge zu AUTOSAR-RTE,
  AUTOSAR-Adaptive (DDS-AAP)
- **Aerospace — DO-178C-Profile**: Tool-Qualification-Doku, Modified-
  Condition / Decision-Coverage-Reports, Safety-Manual
- **Automotive — ISO 26262 (ASIL-B)**: Safety-Argument-Tree
- **Industrial — IEC 61508 (SIL-2)**: Same für Industrieanlagen
- **Defense — IPMS / NATO-AEP-2025**: Wenn Demand
- **Medical — IEC 62304**: Software-Lifecycle-Doku

**Trigger:** per-Vertical erste Pilot-Kunden mit Compliance-Forderung.
Vor dem ersten Pilot ist die Investition vergeudet.

**Estimate:** 12+ PW pro Vertikale, parallel-fähig wenn mehrere
gleichzeitig anlaufen.

## Track Post-D — Real-Time-Edge (kurz)

**Items:**
- PREEMPT_RT-Linux-Latency-Bound-Beweis (Cyclictest-Reports)
- AUTOSAR Classic — eigentliche Implementierung, nicht nur Doku
- Bare-Metal-MCU-Demo-Boards: STM32, RP2040, ESP32, NXP-S32K
- TSN-LAN-Live-Test auf echter HW (nicht emuliert)
- DDS-XRCE-Mesh: mehrere XRCE-Clients über LoRaWAN/BLE/Zigbee

**Trigger:** wenn Mikro-Profile-Audit (RC2-E) Mängel zeigt, oder
HW-Partner liefert Ressourcen für richtige Lab-Tests.

**Estimate:** 6-10 PW.

## Was eindeutig NICHT in den Backlog kommt

- Eigener Blog
- LinkedIn / Twitter / Mastodon / Discord / Slack
- "ZeroDDS Foundation" als Non-Profit (premature, frühestens nach 5+
  unabhängigen Maintainern)
- Eigene Crypto/Coin/Token (lol)
- Web-Frontend-Framework eigener Bauart (DDS ≠ Webdev)
- Rewrite in einer anderen Sprache (Rust ist die Wahl)

## Reprioritization-Cadence

Nach jedem Minor-Release (1.1, 1.2, 1.3, ...) wird der Backlog
re-priorisiert basierend auf:

1. Pilot-Kunden-Feedback (welche Vertical brennt am stärksten)
2. CRA / Compliance-Deadline-Approach
3. Cloud-Native-Adoption-Datapoints
4. Konkurrenz-Bewegung (RTI / Cyclone / Fast-DDS Roadmaps)
