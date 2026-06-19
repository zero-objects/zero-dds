# Migration / Alternative Middleware

← [Zurück zur Übersicht](index.md)

## Der Schmerz

Das klarste Signal, dass DDS-Schmerz real ist: 2023 adoptierte das ROS-Projekt offiziell eine **alternative Middleware** (Zenoh, `rmw_zenoh`), weil DDS als alleinige Middleware für einen großen Teil der Community nicht out of the box funktionierte (**7 Reports**, plus dies ist die Schlussfolgerung, auf die der Rest des Korpus zeigt).

- Der offizielle Alternative-Middleware-Report nennt netzweite Crashes durch DDS-Multicast-Packet-Storms, DDS, das auf verwalteten Netzen nicht out of the box funktioniert, und die Notwendigkeit von Experten-, anwendungsspezifischer DDS-Konfiguration.
- Zenoh wurde als die meistempfohlene Alternative ausgewählt.

### Jüngstes / Flaggschiff-Beispiel

**[ROS 2 Alternative middleware report](https://discourse.openrobotics.org/t/ros-2-alternative-middleware-report/33771)** (2023-09-27, OSRF). Die kanonische Aussage, warum DDS allein als unzureichend erachtet wurde, und die Entscheidung, eine Nicht-DDS-Middleware hinzuzufügen.

### Referenzliste

| Datum | Quelle | Punkt |
|---|---|---|
| 2023-09-27 | [ROS 2 Alternative middleware report](https://discourse.openrobotics.org/t/ros-2-alternative-middleware-report/33771) | Offiziell: DDS allein unzureichend → Zenoh adoptieren |
| 2023-10-30 | [Eclipse newsroom](https://newsroom.eclipse.org/eclipse-newsletter/2023/october/eclipse-zenoh-selected-alternate-ros-2-middleware) | Zenoh als alternative ROS-2-Middleware ausgewählt |
| 2024-06-12 | [ZettaScale news](https://www.zettascale.tech/news/zenoh-experimental-support-lands-in-ros-2/) | Zenoh-Experimental-Support landet in ROS 2 |
| 2024-07-03 | [arXiv 2407.03091](https://arxiv.org/abs/2407.03091) | Middleware-Vergleich für Multi-Roboter-Mesh-Netze |
| 2025-01-03 | [ROS Discourse](https://discourse.openrobotics.org/t/rmw-zenoh-binaries-for-rolling-jazzy-and-humble/41395) | rmw_zenoh-Binaries für Rolling/Jazzy/Humble ausgeliefert |

## Die ZeroDDS-Position

**Du musst DDS nicht verlassen, um dem DDS-Schmerz zu entkommen.**

Zenoh fixt die Ergonomie (Router-/Broker-Style-Discovery, funktioniert auf WiFi und in der Cloud), indem es den RTPS-Draht verlässt — was bedeutet, dass eine Zenoh-Flotte kein natives DDS mehr spricht und das Bridging zurück zu bestehenden DDS-Systemen eine separate Komponente ist. Für die große installierte Basis an DDS-Robotern, -Sensoren und -Tooling ist das eine echte Kostenstelle.

**ZeroDDS geht den anderen Weg: behebt die Gründe, aus denen Leute DDS verließen, und bleibt dabei auf dem RTPS-Draht.**

- **Discovery-Ergonomie wie Zenoh, ohne RTPS zu verlassen.** Multicast-freie Unicast-Peers (keine Broadcast-Storms, funktioniert auf WiFi/Docker/Cloud) — aber der Draht ist weiterhin natives RTPS 2.5, sodass ZeroDDS-Nodes direkt mit der bestehenden Fast-DDS- / Cyclone- / OpenDDS- / Connext-Flotte interoperieren (verifiziert 20/20 mit echtem `rmw_cyclonedds`).
- **Die strukturellen Fixes, kein Workaround.** Laute QoS-Fehler, kein stiller Large-Data-Cap, variabel-große Zero-Copy-SHM, robotik-passende Defaults, volle DDS-Security — die spezifischen Cluster, die dieser Trail dokumentiert.
- **Standard-erhaltend.** Ein vollständiger, auditierter OMG-DDS-Spec-Stack bedeutet, dass bestehendes DDS-Tooling, Type-Systeme und Security-Modelle weiterarbeiten; es gibt kein neues Protokoll zu bridgen.
- **Memory-safe, MCU-bis-Server.** Pure-Rust, `forbid(unsafe_code)`-sicherer-Kern, `no_std + alloc` für Embedded — eine Eigenschaft, die weder die etablierten C++-Stacks noch eine Separate-Protokoll-Middleware bietet.

## Warum das die bessere Migration ist

Die Wahl, in die die Community gedrängt wurde, war „DDS behalten und den Schmerz behalten" *oder* „Zenoh adoptieren und native DDS-Interop verlieren". ZeroDDS ist die dritte Option: **den DDS-Standard und die Wire-Interop behalten und den Schmerz verlieren** — sodass ein Team `rmw_zerodds` auf einem Roboter drop-in einsetzen, mit dem Rest seiner DDS-Flotte unverändert interoperieren und die Verbesserung inkrementell validieren kann.

## Selbst validieren

Dieser ganze Trail ist ein Satz falsifizierbarer, reproduzierbarer Aussagen. Starte irgendwo:

```bash
crates/ros2-rmw/interop/run_interop.sh                 # live ROS-2-Interop
crates/ros2-rmw/interop/run_multicast_free_xvendor.sh  # cross-vendor, kein Multicast
crates/ros2-rmw/interop/run_largedata.sh               # Large-Data, byte-genau
```

→ [Zurück zur Übersicht](index.md)
