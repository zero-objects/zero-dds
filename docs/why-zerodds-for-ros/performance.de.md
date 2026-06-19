# Performance / Latenz / CPU

← [Zurück zur Übersicht](index.md)

## Der Schmerz

Jenseits von Totalausfällen hat DDS in ROS 2 scharfe Performance-Kanten (**19 Reports**): große Nachrichten, die unverwandte Arbeit blockieren, Latenz, die bei niedrigen Publish-Raten *schlechter* wird, und CPU/Bandbreite, die für Traffic verbraucht werden, der nicht existieren sollte.

- **Das Publizieren einer großen Nachricht blockiert alle Callback-Groups** — ein großes Sample stallt den ganzen Executor.
- Latenz steigt für niederfrequente Daten (Warm-Path-Annahmen).
- Selbst alternative Middlewares zeigen hohe Latenz / verlorene Nachrichten bei hohen Publish-Raten, das ist also nicht auf einen Vendor beschränkt.
- Stacks senden unnötige Pakete durchs Netz und verbrennen Bandbreite und CPU.

### Jüngstes Beispiel

**[rmw_cyclonedds#559 — „Publishing large message blocks all callback groups"](https://github.com/ros2/rmw_cyclonedds/issues/559)** (2026-03-03). Ein einziges großes Publish blockiert jede Callback-Group — ein Head-of-Line-Stall, der unverwandte Teile der Anwendung durch die Middleware koppelt.

### Referenzliste (jüngste zuerst)

| Datum | Quelle | Problem |
|---|---|---|
| 2026-03-03 | [rmw_cyclonedds#559](https://github.com/ros2/rmw_cyclonedds/issues/559) | Großes Publish blockiert alle Callback-Groups |
| 2025-04-12 | [cyclonedds#2256](https://github.com/eclipse-cyclonedds/cyclonedds/issues/2256) | Latenz *steigt* für niederfrequente Daten |
| 2024-06-07 | [rmw_zenoh#198](https://github.com/ros2/rmw_zenoh/issues/198) | Hohe Latenz / verlorene Nachrichten bei hoher Pub-Rate (sogar auf Zenoh) |
| 2024-04-19 | [rmw_cyclonedds#489](https://github.com/ros2/rmw_cyclonedds/issues/489) | Sendet unnötige Pakete durchs Netz |

## Wie ZeroDDS es löst

**Niedrige, konsistente Latenz aus einem event-getriebenen Kern; Large-Data auf eigenem Pfad, sodass es nicht alles andere stallt.**

- **Gemessene niedrige Latenz.** Roundtrip-Latenz auf Loopback ist **p50 = 40 µs / p99 = 83 µs** (256 B, 200 Samples, 0 verloren). Die Zahl ist aus den offenen `latency_ping`- / `latency_pong`-Examples reproduzierbar — kein Slide.
- **Event-getrieben, kein Busy-Poll.** Empfangs- und Warte-Pfade nutzen Listener/Condvars/Waker, nie Spinloops, sodass es kein „Latenz wird schlechter im Idle / niederfrequent"-Warm-Path-Artefakt und keinen verbrannten Kern beim Warten gibt.
- **Large-Data auf einem dedizierten Fragmentierungs-Pfad.** Große Samples gehen durch den DATA_FRAG- / NACK_FRAG-Pfad mit expliziten Caps; sie monopolisieren nicht den Small-Message-Fast-Path, was der Mechanismus hinter „ein großes Publish blockiert alles" ist.
- **Kein gratuiter Traffic.** Multicast-frei Unicast bedeutet, dass Pakete an die Peers gehen, die sie angefragt haben — kein Broadcast-Discovery-Chatter, der Bandbreite und CPU auf jedem Node verbraucht.
- **Effizient by construction.** Pure-Rust, `no_std + alloc`-Kern (~1,6 MB auf Cortex-M4F) — derselbe Code-Pfad vom MCU bis zum Server, ohne Garbage-Collector oder schwergewichtige Runtime.

> **Ehrlicher Status:** die Loopback-Latenz, der WiFi-Durchsatz (10,8 MiB/s) und die Scaling-Zahlen sind verifiziert. Head-to-head-Latenz-/Durchsatz-Vergleichstabellen *gegen jeden Vendor auf identischer Hardware* werden noch produziert — genau die Art Messung, die wir Open-Source-Validatoren fahren und publizieren sehen wollen.

## Warum es kein Schmerz mehr sein muss

Der Performance-Cluster ist *Kopplung* (Large-Data stallt Small-Data), *Warm-Path-Annahmen* (Idle-Latenz) und *gratuiter Traffic*. ZeroDDS hält Pfade entkoppelt, bleibt event-getrieben und sendet nur Unicast an echte Peers — sodass Performance vorhersehbar ist statt voller scharfer Kanten.

## Selbst reproduzieren

```bash
# Loopback-RTT-Verteilung (p50/p90/p99), 256 B, 200 Samples:
crates/dcps/examples/latency_ping   # + latency_pong
```

→ [Zurück zur Übersicht](index.md) · Weiter: [Migration](migration.md)
