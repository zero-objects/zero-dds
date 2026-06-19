# Konfigurations-Komplexität

← [Zurück zur Übersicht](index.md)

## Der Schmerz

DDS in ROS 2 gut zum Laufen zu bringen heißt routinemäßig, ein Teilzeit-DDS-Netzwerk-Ingenieur zu werden (**21 Reports**): hunderte XML-Knöpfe, Per-Vendor-Dialekte (Fast-DDS-Profile vs Cyclone-XML vs Connext-QoS), Kernel-Tuning (`rmem_max`, `ipfrag_*`) und versteckte Voraussetzungen, die nahezu unmöglich zu finden sind. Die ausgelieferten Defaults sind nicht die robotik-/WiFi-passenden, sodass „gut genug" Tage von Trial-and-Error kostet.

- Localhost-Only-Modus erfordert still, dass Multicast auf dem Loopback-Interface aktiviert ist (`ip link set lo multicast on`) — in der Praxis undokumentiert.
- Das richtige Netzwerk-Interface zu wählen oder einen Discovery-Server introspizierbar zu machen, braucht Experten-XML.
- Ein Binary-Install kann sogar per Default nach einem *bezahlten* Vendor suchen.

### Jüngstes Beispiel

**[ROS Discourse — „I'm done manually tuning DDS parameters!"](https://discourse.openrobotics.org/t/im-done-manually-tuning-dds-parameters/54415)** (2026-04-30). Ein langer, gut aufgenommener Thread: hunderte Knöpfe, Tage von Trial-and-Error und immer noch suboptimale Ergebnisse — eine repräsentative Aussage über die Konfigurations-Komplexitäts-Steuer.

### Referenzliste (jüngste zuerst)

| Datum | Quelle | Problem |
|---|---|---|
| 2026-04-30 | [ROS Discourse](https://discourse.openrobotics.org/t/im-done-manually-tuning-dds-parameters/54415) | „Done tuning DDS": hunderte Knöpfe, Tage verloren |
| 2025-12-09 | [ROS Discourse](https://discourse.openrobotics.org/t/dds-in-ros-2-consolidated-user-insights/51340) | OSRF „Consolidated User Insights" zum DDS-Schmerz |
| 2025-08-15 | [ros2#1716](https://github.com/ros2/ros2/issues/1716) | Jazzy auf Windows sucht nach *bezahltem* RTI Connext |
| 2025-04-04 | [rmw_cyclonedds#537](https://github.com/ros2/rmw_cyclonedds/issues/537) | `failed to create domain, error Error` |
| 2025-04-02 | [cyclonedds#2201](https://github.com/eclipse-cyclonedds/cyclonedds/issues/2201) | Netzwerk-Interface-Auswahl erfordert Config-Spelunking |

## Wie ZeroDDS es löst

**Robotik-passende Defaults out of the box, und Umgebungsvariablen statt XML-Dialekte.**

- **`ros_defaults()` funktioniert out of the box.** Ein einziges `RuntimeConfig::ros_defaults()` setzt die Representation-Offers (XCDR1 + XCDR2) und den 16-MiB-Reassembly-Cap, die ROS tatsächlich braucht — `rmw_zerodds` interopt mit einem echten ROS-2-Talker **20/20 ohne XML und ohne Umgebungs-Tuning**.
- **Konfiguration ist Umgebungsvariablen, kein XML-Dialekt.** Discovery (`ZERODDS_PEERS`, `ZERODDS_NO_MULTICAST`), Interface-Pinning (`ZERODDS_INTERFACE`), Sample-Caps (`ZERODDS_MAX_SAMPLE_BYTES`), Peer-Limits (`ZERODDS_MAX_PEER_PARTICIPANTS`) — flache, dokumentierte Knöpfe, kein verschachteltes Profile-XML, das du mit einem Parser debuggst.
- **Interface-Auswahl ist eine Variable.** Der „Netzwerk-Interface-Auswahl"-Schmerz ([cyclonedds#2201](https://github.com/eclipse-cyclonedds/cyclonedds/issues/2201)) ist `ZERODDS_INTERFACE=<ip>`, uniform über UDP/TCP/SHM/UDS angewandt.
- **Keine versteckte Loopback-Voraussetzung.** Unicast-Localhost-Discovery hängt nicht davon ab, dass Multicast auf `lo` aktiviert ist.
- **Kein Paid-Vendor-Fallback.** Der ganze Stack ist Open Source (Apache-2.0 / MIT); es gibt keinen proprietären Tier, zu dem ein Default-Install driften könnte.

## Warum es kein Schmerz mehr sein muss

Die Konfigurations-Steuer kommt von *Defaults, die für Data-Center-DDS getunt sind, exponiert über Per-Vendor-XML*. ZeroDDS liefert Defaults, die für den Robotik-/WiFi-Fall getunt sind, und exponiert die wenigen Knöpfe, die du wirklich brauchst, als flache Umgebungsvariablen — sodass das mediane Projekt null Konfiguration braucht, und der Rest eine Handvoll dokumentierter Variablen, kein Wochenende mit dem XML-Schema eines Vendors.

## Selbst reproduzieren

```bash
# Out-of-the-box-ROS-Interop ohne XML / ohne Env-Tuning:
crates/ros2-rmw/interop/run_interop.sh   # nutzt ros_defaults()
```

→ [Zurück zur Übersicht](index.md) · Weiter: [Scaling](scaling.md)
