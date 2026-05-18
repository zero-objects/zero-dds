# Phase RC3 — `1.0.0-rc.3` Breadth

**Goal:** ZeroDDS-Stack erweitert um zusätzliche Sprach-Bindings und
optionale OMG-DDS-Profile (Routing-Service + Persistence-Service).

**Status:** 📋 todo (gated auf RC2 abgeschlossen)

**Estimate:** 8-12 Wochen, drei Tracks parallel.

## Tracks

| # | Track | Detail-Doku | Estimate |
|---|---|---|---|
| RC3-A | Sprach-Bindings Round 2 (Go, Swift, Kotlin, Zig) | [`track-languages-go-swift-kotlin-zig.md`](track-languages-go-swift-kotlin-zig.md) | 6-10 PW |
| RC3-B | OMG DDS Routing-Service | [`track-dds-routing-service.md`](track-dds-routing-service.md) | 3-4 PW |
| RC3-C | OMG DDS Persistence-Service (Spec-conformant) | [`track-dds-persistence-service.md`](track-dds-persistence-service.md) | 2-3 PW |

## Phase-Acceptance

- 4 neue Sprach-Bindings (Go, Swift, Kotlin, Zig) jeweils mit hello-world
  + spec-coverage-doc
- DDS-Routing-Service-Daemon `zerodds-routing-bridged` läuft mit
  domain-bridge zwischen 2 isolierten DDS-Domains
- DDS-Persistence-Service als spec-conformant Wrapper um die in RC2
  gebaute Datalake-Engine (=spec-Compliance, nicht eigener Stack)

## Was nach RC3 published wird

`1.0.0-rc.3`-Tag mit:
- 4 neue Sprach-Crates (`zerodds-go`, `zerodds-swift`, `zerodds-kotlin`,
  `zerodds-zig`) auf crates.io
- Per-Sprache-Distribution: Go-Module, Swift-Package-Manager, Kotlin
  via Gradle, Zig via build.zig.zon
- 2 neue Daemons
- Updated Documentation Trail mit Sprach-Sektion erweitert
