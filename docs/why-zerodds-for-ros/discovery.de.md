# Discovery

← [Zurück zur Übersicht](index.md)

## Der Schmerz

DDS-Discovery ist die meistgemeldete Einzelquelle für „meine ROS-2-Nodes reden nicht" (**62 Reports** im Field-Scan). Drei strukturelle Probleme kehren wieder:

1. **Multicast-basierte Simple Discovery (SDP) ist fragil und laut.** Sie hängt an UDP-Multicast, das auf WiFi, in Docker und in verwalteten Firmen-/Uni-Netzen gedroppt oder rate-limitiert wird. Wo es funktioniert, wächst der Traffic mit der Zahl der Endpoints — und ROS 2 erzeugt viele interne Topics je Node, sodass Discovery-Traffic die *eigentlichen Daten* bei Flotten-Größe *übertönen* kann.
2. **Der Discovery-Server-„Fix" ist selbst fragil.** Ein Neustart des Servers oder eines Nodes lässt Endpoints häufig dauerhaft un-gematcht, bis alles in der richtigen Reihenfolge neu gestartet wird; er braucht Experten-XML und CLI-`SUPER_CLIENT`-Konfiguration, um überhaupt introspizierbar zu sein.
3. **Defaults entdecken zu viel.** Unverwandte Roboter im selben Netz finden sich gegenseitig und können ungewollte Bewegung auslösen.

### Jüngstes Beispiel

**[Fast-DDS#6401 — „Unexpected piggyback HB to all matched readers breaks EDP recovery loop after sleep/wake cycle"](https://github.com/eProsima/Fast-DDS/issues/6401)** (2026-05-18). Nach einem Sleep/Wake-Zyklus in einer Drei-Node-Simple-Discovery-Topologie schafft ein Node-Paar das Re-Match dauerhaft nicht, weil ein asynchroner Piggyback-Heartbeat an *alle* gematchten Reader gebroadcastet wird und die Re-Match-State-Machine eines dritten Nodes korrumpiert.

### Referenzliste (jüngste zuerst)

| Datum | Quelle | Problem |
|---|---|---|
| 2026-05-18 | [Fast-DDS#6401](https://github.com/eProsima/Fast-DDS/issues/6401) | EDP-Recovery bricht nach Sleep/Wake (Piggyback-HB) |
| 2026-05-11 | [Fast-DDS#6346](https://github.com/eProsima/Fast-DDS/issues/6346) | Remote-Reader/Writer in 3.5.0+ nicht mehr entdeckt |
| 2025-10-14 | [Fast-DDS#5872](https://github.com/eProsima/Fast-DDS/issues/5872) | DataReader bekommt keine Daten nach Discovery-Server-Neustart |
| 2025-06-23 | [rmw_cyclonedds#541](https://github.com/ros2/rmw_cyclonedds/issues/541) | Listener bekommt mit einer RMW keine Nachricht, mit anderer schon |
| 2022-10-05 | [ROS Discourse](https://discourse.openrobotics.org/t/proposed-changes-to-how-ros-performs-discovery-of-nodes/27640) | OSRF: Defaults entdecken zu viel *und* fluten das Netz |
| 2020-11-17 | [ROS Discourse](https://discourse.openrobotics.org/t/new-discovery-server/17383) | SDP-Traffic 93 % höher; übertönt Daten bei 50–200 Nodes |

## Wie ZeroDDS es löst

**Discovery ist direkte Unicast-Peer-Adressierung — kein Multicast, kein Server.**

- **Multicast-freie Unicast-Discovery.** Setze `ZERODDS_PEERS` auf die Peer-IPs (oder `ip:port`) und `ZERODDS_NO_MULTICAST=1`. ZeroDDS sendet SPDP an den Well-known-RTPS-Port jedes Peers (`7400 + 250·domain + 10 + 2·pid`). Kein Multicast-Paket verlässt jemals den Host, sodass WiFi-/Docker-/Subnetz-Multicast-Handling irrelevant ist.
- **Kein Discovery-Server zum Neustarten.** Weil Peers sich direkt adressieren, gibt es keinen separaten Server-Prozess, dessen Neustart das Mesh halb-gematcht zurücklässt — die ganze Klasse „DataReader bekommt keine Daten nach Server-Neustart" ([Fast-DDS#5872](https://github.com/eProsima/Fast-DDS/issues/5872)) existiert nicht.
- **Deterministisches Re-Match.** Das SEDP von ZeroDDS re-announct und re-matcht nach definiertem Schema; der Fehlermodus „dauerhaft un-gematcht nach Sleep/Wake" wird von direktem, idempotentem Peer-State getrieben, nicht von einem fragilen Piggyback-HB-Seiteneffekt.
- **Die „Listener bekommt keine Nachricht"-Klasse ist für uns ein bekannter, gefixter Bug.** [rmw_cyclonedds#541](https://github.com/ros2/rmw_cyclonedds/issues/541) ist die Keyed-vs-Keyless-EntityKind-Mismatch-Familie — ein Keyless-Typ, der mit einer WithKey-Entity-ID announct wird, wird vom Topic-Kind-Match des Peers still abgelehnt. ZeroDDS konsultiert `DdsType::HAS_KEY`, um den korrekten Entity-Kind zu emittieren — genau das ließ die ZeroDDS-↔-`rmw_cyclonedds`-Interop 20/20 bidirektional laufen.
- **Scope ist opt-in, nicht versehentlich.** Peers sind eine explizite Liste, sodass unverwandte Roboter im selben Netz sich per Default nicht gegenseitig entdecken.

## Warum es kein Schmerz mehr sein muss

Die Wurzel des Discovery-Clusters ist *Indirektion und Broadcast*: Multicast, das du nicht kontrollierst, plus ein Server (oder Piggyback-Seiteneffekte), dessen State desyncen kann. ZeroDDS ersetzt beides durch **explizite, direkte Unicast-Peer-Adressierung** — dasselbe, was Teams am Ende mit Discovery-Servern und XML von Hand zusammenbauen, aber als First-Class-Out-of-the-Box-Modus ohne Extra-Prozess.

## Selbst reproduzieren

```bash
# Multicast-freie Discovery cross-vendor (ZeroDDS-Sub ↔ Cyclone-Talker,
# Multicast auf beiden voll deaktiviert): erwarte matched=1, 20/20 Samples.
crates/ros2-rmw/interop/run_multicast_free_xvendor.sh

# rmw_zerodds gegen einen echten rmw_cyclonedds-Talker/Listener auf rt/chatter.
crates/ros2-rmw/interop/run_interop.sh
```

→ [Zurück zur Übersicht](index.md) · Weiter: [Multicast / WiFi](multicast-wifi.md)
