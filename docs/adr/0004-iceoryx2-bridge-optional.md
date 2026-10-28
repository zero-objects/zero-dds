# 0004 — Iceoryx2 as an optional backend (build + config)

- **Status:** accepted
- **Date:** 2026-05-04
- **Authors:** @sandra
- **Context:** crates/flatdata, docs/specs/zerodds-flatdata-1.0.md
- **Supersedes:** D-2 in zerodds-flatdata-1.0.md (was: "v1.1+, scope-out")

## Context

D-2 in the flatdata spec originally classified Iceoryx2 as a v1.1+
feature, with the reason "iceoryx2 is still under stabilization in
2026". During the phase-2 decision round it became clear:

- **Eclipse Iceoryx has industry reach**: ROS2 Iceoryx plugin,
  Cyclone DDS Iceoryx adapter, Apex.AI performance test. Callers in
  the robotics/avionics ecosystem often already run the `ioxd` daemon.
- **The Iceoryx subscriber pattern** (separate discovery, its own pub/sub
  API) is a different class of consumer than our DDS path.
- **POSH certification path** (the Iceoryx foundation drives ISO-26262)
  we would get as a pull-through.
- v0.5 → v0.6 API breaks are manageable for us, because we only
  bind it as an optional feature.

What the Iceoryx bridge offers that our own PosixShmTransport **cannot**
do:
- Existing Iceoryx apps (without code changes) as subscribers.
- POSH cert track as an advantage for callers.
- `ioxd` daemon discovery for Iceoryx-first callers.

## Decision

**The Iceoryx2 adapter is integrated in v1.0 as an optional backend,
controlled by a build flag AND a config flag.**

- Build flag: `--features iceoryx2-bridge` enables the crate dependency
  `iceoryx2 = "0.5"` (or a newer stable variant).
- Config flag: at runtime the DataWriter/Reader config decides:
  ```rust
  pub enum FlatBackendConfig {
      InMemory { slot_count, slot_capacity },
      Posix { segment_path, slot_count, slot_capacity },
      #[cfg(feature = "iceoryx2-bridge")]
      Iceoryx2 { service_name, max_subscribers, ... },
  }
  ```
- The default build has **no** iceoryx2 — workspace CI stays lean.
- A caller that enables iceoryx sees a second set of discovery PIDs
  (`PID_ICEORYX_SERVICE`) in parallel to PID_SHM_LOCATOR.

## Alternatives

1. **Iceoryx as default** — forces every caller onto a third-party crate,
   API stability risk. Rejected.
2. **Iceoryx as a v1.1 feature** (original D-2) — delays
   onboarding of ROS2 callers. Rejected.
3. **Iceoryx compatibility layer on our own PosixShmTransport** — re-
   implements the Iceoryx wire format ourselves, high effort. Rejected.
4. **Optional feature + config flag** (chosen) — the caller chooses; no
   lock-in.

## Consequences

**Positive**:
- Iceoryx-first callers (ROS2 Iceoryx, Apex.AI) can communicate
  with ZeroDDS topics right away.
- POSH cert track as an optional pull-through.
- The default workspace stays without external dependencies.

**Negative**:
- Maintenance cost: a second backend with its own test surface.
- Iceoryx2 API breaks are possible; we have to follow them.
- Documentation becomes more complex — the caller needs guidance on "when
  POSIX vs Iceoryx".

**Follow-up tasks**:
- F-Iox: implement Iceoryx2SlotAdapter (against the `SlotBackend`
  trait from ADR-0003).
- Documentation: `internal/integration/flatdata-backend-choice.md` as
  decision guidance for callers.
- CI: `--features iceoryx2-bridge` build as a separate job.

## References

- `docs/specs/zerodds-flatdata-1.0.md` D-2 (superseded by this ADR)
- ADR-0003 (backend trait)
- Iceoryx2 repo: <https://github.com/eclipse-iceoryx/iceoryx2>
- ROS2 Iceoryx plugin: <https://github.com/ros2/rmw_iceoryx>
