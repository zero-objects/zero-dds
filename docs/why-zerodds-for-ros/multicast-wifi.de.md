# Multicast / WiFi

← [Zurück zur Übersicht](index.md)

## Der Schmerz

DDS-Discovery defaultet auf UDP-Multicast, und der Daten-Pfad lehnt sich an Broadcast-Annahmen, die drahtlose Netze nicht einhalten (**34 Reports**). Auf WiFi:

- Multicast wird von Access Points rate-limitiert oder gedroppt, sodass Discovery still scheitert — der kanonische „funktioniert an meinem Schreibtisch, stirbt im Labor"-Fehler.
- Wo Multicast *erlaubt* ist, kann Discovery-Traffic über WiFi fragmentieren und sich wie ein selbstverschuldeter Mini-DDoS verhalten, der sekundenlange Aussetzer verursacht, die Multi-Roboter-Setups crashen.
- Unkonfigurierte Multi-Interface-Hosts announcen *jedes* Interface und streamen dann Punktwolken und LiDAR an Adressen, die off-network routen und Uplinks tagelang sättigen.

Die eigene Schlussfolgerung der Community: out of the box auf gewöhnlichem WiFi zu funktionieren ist „das Minimum Viable Product" für ROS 2 — und Stock-DDS reißt diese Latte.

### Jüngstes Beispiel

**[turtlebot4#673 — „Configuring Fast DDS Discovery Server to use TCP to bypass firewall UDP flood protection"](https://github.com/turtlebot/turtlebot4/issues/673)** (2026-02-04). Um einen TurtleBot 4 in einem verwalteten drahtlosen Netz zum Laufen zu bringen, müssen Nutzer einen Discovery-Server aufsetzen *und* den Transport auf TCP umstellen, rein um die UDP-Flood-Protection des Netzes zu umgehen, die bei DDS-Traffic auslöst.

### Referenzliste (jüngste zuerst)

| Datum | Quelle | Problem |
|---|---|---|
| 2026-02-04 | [turtlebot4#673](https://github.com/turtlebot/turtlebot4/issues/673) | Braucht Discovery-Server + TCP gegen WiFi-UDP-Flood-Protection |
| 2025-11-05 | [Cyclone-WiFi-Gist](https://gist.github.com/robosam2003/d5fcfaf4bfd55298d86c1460cb7fc60c) | Hand-getuntes XML, damit Cyclone auf Enterprise-WiFi+Ethernet läuft |
| 2025-08-15 | [arXiv 2508.11366](https://arxiv.org/html/2508.11366v1) | Ganzes Paper über Optimierung von ROS-2-Comms für Wireless |
| 2025-02-10 | [eProsima „ROS 2 Easy Mode"](https://www.eprosima.com/news/forget-packet-loss-forget-discovery-hassles-meet-ros-2-easymode) | Vendor liefert einen „Easy Mode", um Discovery-/Packet-Loss-Schmerz zu verstecken |
| 2022-11-25 | [ROS Discourse](https://discourse.openrobotics.org/t/ros2-wifi-multicast-multi-robot-and-igmp-snooping/28516) | Multicast über WiFi → 1-s-Aussetzer → Drohnen-Crashes |
| 2022-05-24 | [ROS Discourse](https://discourse.openrobotics.org/t/unconfigured-dds-considered-harmful-to-networks/25689) | Unkonfiguriertes DDS flutet Netze tagelang |

## Wie ZeroDDS es löst

**Entferne die Multicast-Abhängigkeit komplett, und announce nur das Interface, das du meinst.**

- **Null Multicast auf dem Draht.** `ZERODDS_NO_MULTICAST=1` + `ZERODDS_PEERS` gibt volle Discovery über reines Unicast-UDP. Es gibt nichts, woran das IGMP-Snooping, die Multicast-Rate-Limitierung oder die UDP-Flood-Protection des AP auslösen könnten.
- **TCP-Transport ist nativ, kein Workaround.** Wo ein Netz nur TCP durchlässt, hat ZeroDDS einen First-Class-TCP-Transport — du wählst ihn, du schraubst keinen Discovery-Server dran, um dorthin zu kommen.
- **Interface-Pinning für Multi-Homed-Hosts.** `ZERODDS_INTERFACE=<ip>` bindet Send/Receive und announct genau ein Interface über alle Transporte (UDP/TCP/SHM/UDS), sodass ein Host mit echter NIC plus Docker-/VM-Virtual-Interfaces nie an Adressen announct oder streamt, die off-network routen — der „unconfigured DDS considered harmful"-Fehler kann nicht passieren.
- **Ehrlich über den einen verbliebenen WiFi-Gotcha.** Idle 802.11-Power-Save auf einem WiFi-*Client* kann latenz-sensitive Unicast-Discovery-Frames droppen, bis die NIC geweckt ist. Wir haben das mit einem sauberen A/B-Packet-Capture root-caused; es ist ein OS/AP-Power-Management-Artefakt, das jeden DDS-Vendor identisch betrifft, und die Mitigation liegt auf der OS/AP-Ebene, nicht im Stack. Siehe [`../interop/ros2-c3-large-data-wifi-followup.md`](../interop/ros2-c3-large-data-wifi-followup.md).

## Warum es kein Schmerz mehr sein muss

Jeder WiFi-Fehler im Korpus führt zurück auf *die Abhängigkeit von Multicast-/Broadcast-Verhalten, das die Wireless-Ebene nicht garantiert*, plus *das Announcen von Interfaces, die du nicht meintest*. Die default-fähige Unicast-Discovery von ZeroDDS plus Interface-Pinning entfernt beides — dasselbe Ergebnis, das Teams nach Tagen XML-Tuning und Discovery-Server-Deployment erreichen, verfügbar als Zwei-Umgebungsvariablen-Setup.

## Selbst reproduzieren

```bash
# Large-Data über eine echte WiFi-Strecke, multicast-frei, byte-genau:
crates/ros2-rmw/interop/run_wifi_largedata.sh

# Multicast-freie Cross-Vendor-Discovery (kein Multicast-Paket überhaupt emittiert):
crates/ros2-rmw/interop/run_multicast_free_xvendor.sh
```

→ [Zurück zur Übersicht](index.md) · Weiter: [QoS stilles No-Match](qos-silent-fail.md)
