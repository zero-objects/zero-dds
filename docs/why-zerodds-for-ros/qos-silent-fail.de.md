# QoS stilles No-Match

← [Zurück zur Übersicht](index.md)

## Der Schmerz

DDS matcht einen Publisher und Subscriber nur, wenn ihre QoS kompatibel ist (Reliability, Durability, History, Deadline, Liveliness…). Wenn sie *nicht* kompatibel ist, ist das Spec-Verhalten, **still nicht zu matchen** — keine Daten, und oft kein Fehler, den die Anwendung je sieht (**36 Reports**). Das Ergebnis ist die demoralisierendste ROS-2-Debugging-Session, die es gibt: alles sieht verbunden aus, `ros2 topic list` zeigt das Topic, und keine einzige Nachricht kommt an.

- Ein Sensor-Treiber publiziert BEST_EFFORT; dein Node subscribt RELIABLE → kein Match, keine Nachricht, kein Log.
- `transient_local` (latched) nur auf einer Seite → stilles No-Match.
- Die Community-Position ist, dass QoS-Kompatibilität „zu strikt" ist und auf eine Weise scheitert, die für Nicht-Experten unsichtbar ist.

### Jüngstes Beispiel

**[ros2#1562 — „QoS compatibility is too strict, should be more user-friendly and flexible"](https://github.com/ros2/ros2/issues/1562)** (2024-05-10). Eine Anfrage auf Maintainer-Ebene, die anerkennt, dass das aktuelle QoS-Kompatibilitätsmodell stille Fehler produziert, die nutzerfeindlich sind, und um freundlicheres, sichtbareres Verhalten bittet.

### Referenzliste (jüngste zuerst)

| Datum | Quelle | Problem |
|---|---|---|
| 2024-05-10 | [ros2#1562](https://github.com/ros2/ros2/issues/1562) | QoS-Kompatibilität zu strikt; still, nutzerfeindlich |
| 2024-01-31 | [Stereolabs-Forum](https://community.stereolabs.com/t/help-with-qos-compatibility-issue-in-zed-ros2-wrapper-and-custom-node/4483) | ZED-Wrapper-QoS-Mismatch → keine Daten, Nutzer steckt fest |
| 2024-01-12 | [rviz#1122](https://github.com/ros2/rviz/issues/1122) | RViz: „requesting incompatible QoS" auf `/scan` |
| 2023-09-13 | [rmw_cyclonedds#473](https://github.com/ros2/rmw_cyclonedds/issues/473) | Lifespan funktioniert still nicht mit transient_local |
| 2023-08-29 | [rclcpp#2291](https://github.com/ros2/rclcpp/issues/2291) | Intra-Process-Type-Adaptation scheitert *still* bei Type-Mismatch |

## Wie ZeroDDS es löst

**Mach den Fehler laut, und fang ihn vor dem Launch ab.**

- **Laute No-Match-Events.** Wenn ein Endpoint entdeckt wird, die QoS aber inkompatibel ist, emittiert ZeroDDS ein `qos.incompatible.offered`- / `qos.incompatible.requested`-Event, das die genaue verletzende Policy nennt (via `qos_policy_id_name`-Helper), statt den Match still zu droppen. Der Unit-Test `incompatible_qos_match_emits_loud_warning` pinnt dieses Verhalten.
- **Statische Pre-Flight-Validierung.** Das `qos_check`-CLI berechnet die Publisher/Subscriber-Kompatibilität *vor* dem Launch und exitet non-zero bei einem Mismatch, mit der konkreten inkompatiblen Policy gemeldet — sodass ein CI-Job oder ein Launch-Wrapper „RELIABLE vs BEST_EFFORT" in dem Moment fängt, in dem es eingeführt wird, nicht nach einer Feld-Debugging-Session.
- **Richtige Defaults, sodass der Normalfall einfach matcht.** `RuntimeConfig::ros_defaults()` bietet die Representations und Caps, die ROS-Writer tatsächlich nutzen, sodass die häufigste stille-Mismatch-Ursache (Representation/Encoding) out of the box nicht entsteht.

## Warum es kein Schmerz mehr sein muss

Der Schmerz ist nicht, dass QoS Regeln hat — es ist, dass sie zu brechen *unsichtbar* ist. ZeroDDS behält die spec-korrekte Matching-Semantik (sodass Interop erhalten bleibt), wandelt das stille No-Match aber in ein benanntes, sichtbares Event und einen Pre-Launch-Check um. Der Bug ist nicht mehr „keine Daten, keine Ahnung", sondern „Zeile 12: RELIABLE requested, BEST_EFFORT offered."

## Selbst reproduzieren

```bash
# Statischer QoS-Kompatibilitäts-Check (Exit-Code + benannte verletzende Policy):
cargo run -p zerodds-qos --example qos_check -- <writer-qos> <reader-qos>
```

Der Laute-Warnung-Pfad ist durch `incompatible_qos_match_emits_loud_warning` in der DCPS-Test-Suite abgedeckt.

→ [Zurück zur Übersicht](index.md) · Weiter: [Large-Data](large-data.md)
