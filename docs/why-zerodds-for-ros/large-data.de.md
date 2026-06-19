# Large-Data / Fragmentierung

← [Zurück zur Übersicht](index.md)

## Der Schmerz

Robotik bewegt große Payloads — Kamera-Frames, Punktwolken, Karten, Occupancy-Grids. UDP-Datagramme sind ~64 kB max, also muss DDS große Samples fragmentieren und zuverlässig reassemblieren. In der Praxis ist das eine wiederkehrende Fehler-Oberfläche (**29 Reports**):

- Nachrichten über einer internen Schwelle werden **still gedroppt** — die berüchtigte ~262-kB-Decke in manchen Fast-DDS-Configs — sodass eine Punktwolke einfach nie ankommt, ohne Fehler.
- Auf verlustbehafteten Strecken (WiFi) stallt ein einziges verlorenes Fragment die Reassembly: der Kernel-IP-Fragmentierungs-Buffer füllt sich, und auf manchen Stacks hört der Empfänger sekundenlang auf, Daten anzunehmen (ein vendor-agnostischer, Kernel-Level-Fehler).
- Große Nachrichten spiken Latenz und Bandbreite unvorhersehbar und blockieren unverwandte Callbacks, während das große Sample in flight ist.

### Jüngstes Beispiel

**[Fast-DDS#5686 — „FastDDS High Latency using Large Data"](https://github.com/eProsima/Fast-DDS/issues/5686)** (2025-03-05). Das Aktivieren des Large-Data-Pfads produziert hohe, inkonsistente Latenz — der Large-Message-Transport verhält sich sehr anders als der Small-Message-Pfad, was genau die Art Überraschung ist, die „schick einfach das Bild" unzuverlässig macht.

### Referenzliste (jüngste zuerst)

| Datum | Quelle | Problem |
|---|---|---|
| 2025-03-05 | [Fast-DDS#5686](https://github.com/eProsima/Fast-DDS/issues/5686) | Hohe, inkonsistente Latenz auf dem Large-Data-Pfad |
| 2024-11-15 | [cyclonedds#2139](https://github.com/eclipse-cyclonedds/cyclonedds/issues/2139) | „Unusual performance" mit großen Nachrichten |
| 2024-04-19 | [ros2#1544](https://github.com/ros2/ros2/issues/1544) | Inkonsistente Bandbreite bei Bild-Übertragung |
| 2024-04-14 | [Fast-DDS#4684](https://github.com/eProsima/Fast-DDS/issues/4684) | Send-Buffer-Sizing bricht jenseits `net.core.wmem_max` |
| 2024-03-12 | [ROS Discourse](https://discourse.openrobotics.org/t/ros-2-and-large-data-transfer-on-lossy-networks/36598) | Large-Data auf verlustbehafteten Netzen: Reassembly stallt |

## Wie ZeroDDS es löst

**Kein stiller Cap, selektiver Retransmit und ein Transport, der nicht bei einem einzigen verlorenen Fragment einbricht.**

- **Kein stiller Drop.** Der Reassembly-Cap von ZeroDDS ist **16 MiB by default** (konfigurierbar via `ZERODDS_MAX_SAMPLE_BYTES`). Der alte „Samples über N Bytes verschwinden"-Fehler war ein 1-MiB-Phase-1-Cap, den wir gefunden und entfernt haben; 2- / 4- / 8-MB-Samples reassemblieren byte-genau durch den vollen DCPS-Stack.
- **Selektiver Fragment-Retransmit.** ZeroDDS implementiert DATA_FRAG / NACK_FRAG mit einem Fragment-Assembler, der DoS-Caps hat. Ein verlorenes Fragment triggert ein NACK_FRAG, das *nur die fehlenden Fragmente* neu anfordert, nicht das ganze Sample — verifiziert byte-identisch bei 30 % Packet-Loss. Der Reassembly-Buffer gehört der Anwendung, mit expliziten Caps, sodass der Kernel-IP-Fragmentierungs-Stall nicht zutrifft.
- **WiFi-sichere Fragment-Größe.** Application-Level-Fragmentierung an einer WiFi-sicheren MTU hält jedes Fragment innerhalb eines einzigen Link-Layer-Frames, sodass die Lossy-Network-Reassembly-Klippe by construction vermieden wird.
- **Variabel-große Zero-Copy für Same-Host.** Für den Same-Machine-Pfad hat ZeroDDS einen längen-präfixierten Shared-Memory-Ring (siehe [Shared Memory](shared-memory.md)) — variabel-groß, sodass Punktwolken und Bilder keinen hand-dimensionierten Fixed-Pool brauchen.

## Warum es kein Schmerz mehr sein muss

Der Large-Data-Cluster ist *stille Caps* + *Alles-oder-nichts-Reassembly* + *ein Large-Data-Pfad, der sich gar nicht wie der Small-Data-Pfad verhält*. ZeroDDS entfernt den stillen Cap, retransmittet auf Fragment-Granularität und hält Fragmentierung auf einem gut-getesteten Pfad — sodass „schick einfach die 4-MB-Punktwolke" der Default ist, der funktioniert, auch über verlustbehaftetes WiFi.

## Selbst reproduzieren

```bash
# 2/4/8-MB-Samples durch den vollen DCPS-Stack, byte-genau, multicast-frei:
crates/ros2-rmw/interop/run_largedata.sh

# Dasselbe, über eine echte WiFi-Strecke (Durchsatz ~10,8 MiB/s):
crates/ros2-rmw/interop/run_wifi_largedata.sh
```

→ [Zurück zur Übersicht](index.md) · Weiter: [Cross-Vendor-Interop](interop.md)
