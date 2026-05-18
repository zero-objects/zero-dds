# `zerodds-ros2-bridge` v1.0 — Open + Partial Items

Aggregat aus `zerodds-ros2-bridge-1.0.md`. Nicht von Hand pflegen — vor jedem
Audit-Lauf löschen und aus dem Hauptfile neu generieren.

## Open

— keine. Alle vormals offenen Items im RC1-Cluster-A/B/C-Closeout
2026-05-07 geschlossen oder per Decision-Record als `n/a (rejected)`
klassifiziert.

## Partial

— keine. Alle vormals partial-Items im RC1-Cluster-A/B/C-Closeout
2026-05-07 nach `done` migriert (Service-Pair + Action-Pattern +
REP-2008 + rcutils-Logging).

## Decision-Records (`n/a (rejected)`)

### §7.1 SROS2-Enclaves → DDS-Security 1.2

**Status:** `n/a (rejected)` — siehe
`docs/adr/0008-ros2-sros2-rejected-rc1.md`. ROS-2-SROS2-Enclave-
Mapping ist alternative Format-Form auf bereits live DDS-Security-
1.2-Substanz (K6 closed); 87% der ROS-2-Production-Roboter laufen ohne
SROS2 (OSRF-2025 Survey). Doppel-Implementation ohne Customer-Pull
verworfen.

### §7.2 ACL via Permissions-XML

**Status:** `n/a (rejected)` — siehe ADR-0008. DDS-Security 1.2
Permissions-Plugin (`crates/security-permissions/`) ist direkt
nutzbar; ROS-2-XML-Format-Bridge liefert nur Übersetzungsschicht ohne
Sicherheits-Mehrwert.
