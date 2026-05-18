# Phase RC2 — `1.0.0-rc.2` Pre-1.0 Major Tracks

**Goal:** Datalake-Engine + AMQP 0.9 + vollständiger Audit-Pass über
Demos, Tutorials und Micro-Profile.

**Status:** 📋 todo (gated auf RC1-stabilize abgeschlossen)

**Estimate:** 4-8 Wochen, fünf Tracks parallel.

## Tracks

| # | Track | Detail-Doku | Estimate | Owner |
|---|---|---|---|---|
| RC2-A | Tiered-Storage Datalake | [`track-datalake.md`](track-datalake.md) | 3-5 Wochen | tbd |
| RC2-B | AMQP 0.9.1 / RabbitMQ-native | [`track-amqp-09-rabbitmq.md`](track-amqp-09-rabbitmq.md) | 1-2 Wochen | tbd |
| RC2-C | Demo-Audit | [`track-audit-demos.md`](track-audit-demos.md) | 2 Wochen | tbd |
| RC2-D | Tutorial-Audit | [`track-audit-tutorials.md`](track-audit-tutorials.md) | 2 Wochen | tbd |
| RC2-E | Micro-Profile-Audit | [`track-audit-micro-profiles.md`](track-audit-micro-profiles.md) | 2 Wochen | tbd |

## Phase-Acceptance

- Datalake-Engine läuft mit `dcps-persistence`-Crate, drei Storage-Tiers
  (RAM hot, SSD warm, PostgreSQL cold), konfigurierbare Promotion-Trigger,
  load-test mit ≥10k synthetischen Sensoren über simulierte 30-Tage-Spanne
- AMQP 0.9-Bridge live als zweiter Daemon, Co-Existenz mit AMQP 1.0,
  RabbitMQ 3.13+ live-Test grün
- Alle 4 Demos (`dds-warehouse`, `dds-chat`, `perf-camera-dds`, `otel`)
  audited: jeder Demo dokumentiert hands-on getestet, README-Schritte
  verifiziert, fehlende Pieces gebaut
- Alle 15 Tutorial-Chapters audited, Pub/Sub-Splits in 9 Sprachen wirklich
  lauffähig
- Alle Micro-Profile (no_std, alloc-only, no_std+no_alloc) als
  CI-Build-Target eingebaut, Cortex-M3/M7 cross-build green

## Was nach RC2 published wird

- `1.0.0-rc.2`-Tag mit allen rc.1-Crates + neuer `dcps-persistence`-Crate
  + `amqp09-bridge`-Crate
- Updated Demos + Tutorials in der Distribution
- Performance-Daten Datalake-Skalierung in einem Whitepaper-Draft
  (intern, **noch nicht** published — News-Sektion wartet bis 1.0-final)
