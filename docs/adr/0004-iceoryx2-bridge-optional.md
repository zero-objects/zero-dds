# 0004 — Iceoryx2 als optional Backend (Build + Config)

- **Status:** accepted
- **Datum:** 2026-05-04
- **Autoren:** @sandra
- **Kontext:** crates/flatdata, docs/specs/zerodds-flatdata-1.0.md
- **Supersedes:** D-2 in zerodds-flatdata-1.0.md (war: "v1.1+, scope-out")

## Kontext

D-2 in der flatdata-Spec hat ursprünglich Iceoryx2 als v1.1+-Feature
eingestuft mit der Begründung "iceoryx2 ist 2026 noch unter
Stabilization". Bei der Phase-2-Decision-Runde wurde aber klar:

- **Eclipse Iceoryx hat Industry-Reach**: ROS2-Iceoryx-Plugin,
  Cyclone-DDS-Iceoryx-Adapter, Apex.AI-Performance-Test. Caller im
  Robotics/Avionics-Ecosystem haben oft schon `ioxd`-Daemon laufen.
- **Iceoryx-Subscriber-Pattern** (separate Discovery, eigene Pub/Sub-
  API) ist eine andere Konsumenten-Klasse als unser DDS-Pfad.
- **POSH-Zertifizierungspfad** (Iceoryx-Stiftung treibt ISO-26262)
  würde wir als Pull-Through bekommen.
- v0.5 → v0.6 API-Breaks sind für uns überschaubar, weil wir nur als
  optional Feature anbinden.

Was Iceoryx-Bridge bietet, was eigener PosixShmTransport **nicht**
kann:
- Bestehende Iceoryx-Apps (ohne Code-Änderung) als Subscriber.
- POSH-Cert-Track als Caller-Vorteil.
- `ioxd`-Daemon-Discovery für Iceoryx-First-Caller.

## Entscheidung

**Iceoryx2-Adapter wird in v1.0 als optional Backend integriert,
gesteuert durch Build-Flag UND Config-Flag.**

- Build-Flag: `--features iceoryx2-bridge` aktiviert die Crate-Dep
  `iceoryx2 = "0.5"` (bzw. neuere stabile Variante).
- Config-Flag: zur Runtime entscheidet die DataWriter/Reader-Config:
  ```rust
  pub enum FlatBackendConfig {
      InMemory { slot_count, slot_capacity },
      Posix { segment_path, slot_count, slot_capacity },
      #[cfg(feature = "iceoryx2-bridge")]
      Iceoryx2 { service_name, max_subscribers, ... },
  }
  ```
- Default-Build hat **kein** iceoryx2 — Workspace-CI bleibt schlank.
- Caller, der iceoryx aktiviert, sieht ein zweites Set Discovery-PIDs
  (`PID_ICEORYX_SERVICE`) parallel zu PID_SHM_LOCATOR.

## Alternativen

1. **Iceoryx als Default** — zwingt alle Caller auf eine fremde Crate,
   API-Stability-Risk. Verworfen.
2. **Iceoryx als v1.1-Feature** (ursprüngliches D-2) — verzögert
   ROS2-Caller-Onboarding. Verworfen.
3. **Iceoryx-Compat-Layer auf eigenem PosixShmTransport** — re-
   implementiert Iceoryx-Wire-Format selbst, hoher Aufwand. Verworfen.
4. **Optional Feature + Config-Flag** (gewählt) — Caller wählt; kein
   Lock-in.

## Konsequenzen

**Positiv**:
- Iceoryx-First-Caller (ROS2-Iceoryx, Apex.AI) können sofort mit
  ZeroDDS-Topics kommunizieren.
- POSH-Cert-Track als optionaler Pull-Through.
- Default-Workspace bleibt ohne externe Deps.

**Negativ**:
- Maintenance-Cost: zweites Backend mit eigener Test-Surface.
- Iceoryx2-API-Breaks sind möglich, müssen wir nachziehen.
- Doku wird komplexer — Caller braucht Entscheidungshilfe "wann
  POSIX vs Iceoryx".

**Folge-Aufgaben**:
- F-Iox: Iceoryx2SlotAdapter implementieren (gegen `SlotBackend`-
  Trait aus ADR-0003).
- Doku: `docs/integration/flatdata-backend-choice.md` als
  Entscheidungshilfe für Caller.
- CI: `--features iceoryx2-bridge`-Build als separater Job.

## Referenzen

- `docs/specs/zerodds-flatdata-1.0.md` D-2 (superseded by this ADR)
- ADR-0003 (Backend-Trait)
- Iceoryx2-Repo: <https://github.com/eclipse-iceoryx/iceoryx2>
- ROS2 Iceoryx-Plugin: <https://github.com/ros2/rmw_iceoryx>
