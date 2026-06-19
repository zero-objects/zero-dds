# Shared Memory

← [Zurück zur Übersicht](index.md)

## Der Schmerz

Same-Host-Transport sollte der einfache, schnelle Fall sein — zwei Prozesse auf einer Maschine, die Daten über Shared Memory austauschen. In der DDS-Praxis ist es eine der größten Fehler-Oberflächen (**52 Reports**): Segfaults, `/dev/shm`-Erschöpfung, Cross-User-Permission-Fehler, Mutex-Timeout-Races beim Init und das Fixed-Size-Pool-Modell (Iceoryx), das nicht zu variabel-großen Robotik-Daten passt.

- **Variabel-große Typen liefern null Samples und pinnen einen CPU-Kern auf 100 %** über den Iceoryx-PSMX-Pfad — das Fixed-Pool-Modell kämpft gegen Punktwolken und Bilder.
- Data-Sharing-Reader können bei der Shared-Segment-Init **ewig loopen**.
- SHM-Files bekommen die falschen Permissions (`umask`) und blockieren Cross-User-Zugriff.
- Init-Races erzeugen Mutex-Timeout-Fehler und falsche „segment may be insufficient"-Warnungen.

### Jüngstes Beispiel

**[rmw_cyclonedds#585 — „Variable-size types deliver zero samples over PSMX (iceoryx) and pin a core at 100 % CPU"](https://github.com/ros2/rmw_cyclonedds/issues/585)** (2026-06-02). Der Shared-Memory-Fast-Path liefert für variabel-große Typen *nichts* und verbrennt dabei einen vollen Kern — genau der Mismatch zwischen einem Fixed-Size-SHM-Pool und variabel-großen Robotik-Payloads.

### Referenzliste (jüngste zuerst)

| Datum | Quelle | Problem |
|---|---|---|
| 2026-06-02 | [rmw_cyclonedds#585](https://github.com/ros2/rmw_cyclonedds/issues/585) | Variabel-große Typen → 0 Samples + 100 % CPU über Iceoryx |
| 2026-03-21 | [Fast-DDS#6338](https://github.com/eProsima/Fast-DDS/issues/6338) | Data-Sharing-Reader loopt ewig in der Segment-Init |
| 2025-12-02 | [Fast-DDS#6206](https://github.com/eProsima/Fast-DDS/issues/6206) | Falsche „segment_size may be insufficient"-Warnung |
| 2025-11-10 | [Fast-DDS#6162](https://github.com/eProsima/Fast-DDS/issues/6162) | `umask` falsch auf SHM-Files → Cross-User-Zugriff blockiert |
| 2025-10-22 | [Fast-DDS#6117](https://github.com/eProsima/Fast-DDS/issues/6117) | SHM-`init_port`-Mutex-Timeout-Race |

## Wie ZeroDDS es löst

**Ein variabel-großer, längen-präfixierter SHM-Ring — und ein sicherer Kern, der nicht segfaulten kann.**

- **Variabel-groß by design.** Der Shared-Memory-Transport von ZeroDDS ist ein längen-präfixierter Ring, kein Fixed-Size-Pool. Variabel-große Payloads (Punktwolken, Bilder) fließen ohne hand-dimensionierten Pool, sodass der [#585](https://github.com/ros2/rmw_cyclonedds/issues/585)-Fehler „Fixed-Pool liefert null variabel-große Samples" nicht entsteht. Der einzige Größen-Knopf, `ZERODDS_SHM_MAX_DATAGRAM`, dimensioniert den Ring; die Kapazität folgt automatisch.
- **Keine Segfault-Klasse.** Der SHM-Pfad ist in Rust gebaut; der sichere Kern ist `forbid(unsafe_code)`, und die kleine `unsafe`-mmap/flock-Oberfläche ist isoliert und auditiert. Die Buffer-Overrun- und Use-after-free-Segfaults, die gegen C++-SHM-Transporte gemeldet werden, sind im sicheren Daten-Pfad nicht ausdrückbar.
- **Kein Busy-Wait, kein Init-Race-Livelock.** Warte-Pfade sind event-getrieben (Condvar/Notify), keine Spinloops, sodass die Fehlermodi „loopt ewig und pinnt einen Kern" und „Mutex-Timeout-Init-Race" wegdesignt sind.
- **Cross-Process-Korrektheit.** Atomics über Shared Memory mit wohldefinierter Cross-Process-Semantik; ein Crash-Recovery-Cleanup-Pfad behandelt einen toten Owner.

## Warum es kein Schmerz mehr sein muss

Der SHM-Cluster ist *Fixed-Size-Pools vs variable Robotik-Daten* plus *unsicheres C++-Plumbing, das segfaultet und live-lockt*. ZeroDDS nutzt einen variabel-großen Ring und eine memory-safe Implementierung mit event-getriebenen Waits — sodass Same-Host-Zero-Copy der schnelle Pfad ist, der es sein sollte, für die Payloads, die Robotik tatsächlich sendet.

## Selbst reproduzieren

```bash
# Same-Host-SHM-Pfad mit einem großen variabel-großen Sample:
cargo test -p zerodds-transport-shm
# (und die largedata-Examples mit dem same-host-shm-Feature)
```

→ [Zurück zur Übersicht](index.md) · Weiter: [Scaling](scaling.md)
