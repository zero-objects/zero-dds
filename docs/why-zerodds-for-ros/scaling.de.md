# Scaling / Flotten / viele Nodes

← [Zurück zur Übersicht](index.md)

## Der Schmerz

ROS 2 erzeugt viele Participants und viele interne Topics je Node, sodass eine Flotte oder ein großer Einzelroboter die Discovery- und Matching-Kosten von DDS vervielfacht (**16 Reports**). Fehler zeigen sich als:

- Ein Discovery-Server, der jenseits weniger hundert Participants **unresponsiv** wird.
- Speicher, der explodiert, wenn viele Reader/Writer matchen, oder wenn Distros mischen.
- Deadlocks, wenn viele Reader und Writer unter reliable TCP matchen.
- Offene Fragen in der Community, wie viele Participants eine RMW überhaupt erlaubt.

### Jüngstes Beispiel

**[autoware#6759 — „Fix [rmw_cyclonedds_cpp]: rmw_create_node: failed to create domain, error"](https://github.com/autowarefoundation/autoware/issues/6759)** (2026-01-24). Ein voller Self-Driving-Stack trifft auf Domain-/Participant-Creation-Fehler — Scaling-Limits, die in einem der größten realen ROS-2-Deployments auftauchen.

### Referenzliste (jüngste zuerst)

| Datum | Quelle | Problem |
|---|---|---|
| 2026-01-24 | [autoware#6759](https://github.com/autowarefoundation/autoware/issues/6759) | Participant-/Domain-Creation scheitert in großem Stack |
| 2025-09-09 | [ROS Discourse](https://discourse.openrobotics.org/t/how-many-dds-participants-are-currently-used-allowed-by-rmw/49976) | Wie viele Participants erlaubt eine RMW überhaupt? |
| 2025-04-17 | [Fast-DDS#5767](https://github.com/eProsima/Fast-DDS/issues/5767) | Discovery-Server unresponsiv mit vielen Participants |
| 2025-01-15 | [rmw_fastrtps#797](https://github.com/ros2/rmw_fastrtps/issues/797) | Cross-Distro-Sub/Pub erschöpft den gesamten Speicher |
| 2024-12-04 | [Fast-DDS#5235](https://github.com/eProsima/Fast-DDS/issues/5235) | Discovery-Server-Deadlock mit vielen matchenden Endpoints |

## Wie ZeroDDS es löst

**Kein zentraler Server zum Überlasten, gebundener Peer-State und gemessene All-to-all-Discovery.**

- **Kein Discovery-Server-Bottleneck.** Multicast-freie Discovery ist Peer-to-Peer-Unicast — es gibt keinen einzelnen Server-Prozess, der bei Skalierung unresponsiv werden oder deadlocken könnte ([Fast-DDS#5767](https://github.com/eProsima/Fast-DDS/issues/5767), [#5235](https://github.com/eProsima/Fast-DDS/issues/5235)).
- **Gebundener, expliziter Peer-State.** `ZERODDS_MAX_PEER_PARTICIPANTS` capt, wie viele Participants pro Peer expandiert werden, sodass Discovery-State gebunden und vorhersehbar ist statt open-ended.
- **Gemessene All-to-all-Discovery.** Der Scaling-Harness (`ZERODDS_SCALE_N`) bringt all-to-all, multicast-freie Meshes hoch: ~50 Participants in **~2,9 s**, 100 in **~19,9 s**. Das sind ehrliche aktuelle Zahlen auf einem einzelnen Host — der Punkt ist, dass die Kurve gemessen ist und der Mechanismus (Unicast, kein Server) keinen zentralen Choke-Point hat.
- **Memory-safe Matching.** Die Cross-Distro-„erschöpft den gesamten Speicher"-Klasse ([rmw_fastrtps#797](https://github.com/ros2/rmw_fastrtps/issues/797)) kommt von ungebundenem Wachstum bei fehlerhafter/mismatchter Discovery; ZeroDDS parst mit expliziten Bounds und DoS-Caps.

## Warum es kein Schmerz mehr sein muss

Scaling-Schmerz konzentriert sich auf *den Discovery-Server* und auf *ungebundenen Discovery-State*. ZeroDDS entfernt den zentralen Server (Peer-to-Peer-Unicast) und bindet Peer-Expansion explizit, sodass das Hinzufügen von Robotern lineare, lokale Unicast-Kosten addiert, statt einen geteilten Choke-Point Richtung Klippe zu laden.

> **Ehrlicher Status:** Large-Fleet-Zahlen (hunderte echte Nodes) werden noch gesammelt. Die Single-Host-All-to-all-Kurve oben ist verifiziert; wir wollen Community-Runs auf echten Flotten — siehe [Selbst validieren](index.md#validate-it-yourself).

## Selbst reproduzieren

```bash
# All-to-all, multicast-frei, N Participants:
ZERODDS_SCALE_N=50 <scaling harness>     # ~2,9 s
ZERODDS_SCALE_N=100 <scaling harness>    # ~19,9 s
```

→ [Zurück zur Übersicht](index.md) · Weiter: [Docker / Cloud](docker-cloud.md)
